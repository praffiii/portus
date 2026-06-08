#![cfg(unix)]

use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use portus_lib::logs::ansi::LogBatch;
use portus_lib::projects::spawn_task;
use tauri::ipc::{Channel, InvokeResponseBody};

fn wait_for_exit(
    p: &mut portus_lib::projects::SpawnedProcess,
) -> portus_lib::projects::ProcessExitStatus {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = p.try_status().unwrap() {
            return status;
        }
        assert!(Instant::now() < deadline, "process did not exit in time");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn spawn_captures_stdout_into_ring_buffer() {
    let mut p = spawn_task("/bin/sh", "echo hello-from-task", Path::new("/")).unwrap();
    wait_for_exit(&mut p);
    std::thread::sleep(Duration::from_millis(50));
    assert!(p
        .recent_output()
        .iter()
        .any(|l| l.contains("hello-from-task")));
}

#[test]
fn pty_spawn_makes_stdout_a_tty() {
    let mut p = spawn_task(
        "/bin/sh",
        "python3 -c 'import os, sys; print(os.isatty(sys.stdout.fileno()))'",
        Path::new("/"),
    )
    .unwrap();
    wait_for_exit(&mut p);

    assert!(p.recent_output().iter().any(|l| l.contains("True")));
}

#[test]
fn env_file_values_override_inherited_environment() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(".env"), "PORTUS_L3_VALUE=from-file\n").unwrap();
    std::env::set_var("PORTUS_L3_VALUE", "from-parent");

    let mut p = spawn_task("/bin/sh", "printf '%s\\n' \"$PORTUS_L3_VALUE\"", dir.path()).unwrap();
    wait_for_exit(&mut p);

    std::env::remove_var("PORTUS_L3_VALUE");
    assert!(p.recent_output().iter().any(|l| l.contains("from-file")));
}

#[test]
fn malformed_env_file_keeps_valid_values_and_reports_notice() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(".env"),
        "PORTUS_L3_VALID=kept\nnot valid dotenv\n",
    )
    .unwrap();

    let mut p = spawn_task("/bin/sh", "printf '%s\\n' \"$PORTUS_L3_VALID\"", dir.path()).unwrap();
    wait_for_exit(&mut p);
    let output = p.recent_output();

    assert!(output.iter().any(|l| l.contains("kept")));
    assert!(output.iter().any(|l| l.contains(".env")));
}

#[test]
fn subscribe_logs_streams_sanitized_batches_until_unsubscribed() {
    let mut p = spawn_task(
        "/bin/sh",
        "printf '\\033[31mred\\033[0m\\n'; sleep 1; printf 'after\\n'",
        Path::new("/"),
    )
    .unwrap();
    let (tx, rx) = mpsc::channel();
    p.subscribe_logs(Channel::new(move |body| {
        tx.send(body).unwrap();
        Ok(())
    }));

    let first = recv_log_batch_matching(&rx, |batch| {
        batch
            .lines
            .iter()
            .any(|line| line.html.contains("ansi-fg-red"))
    });
    assert!(first
        .lines
        .iter()
        .any(|line| line.html.contains("ansi-fg-red")));
    assert!(!first
        .lines
        .iter()
        .any(|line| line.html.contains("\x1b[31m")));

    p.unsubscribe_logs();
    std::thread::sleep(Duration::from_millis(1200));
    assert!(rx.try_recv().is_err());
    let _ = wait_for_exit(&mut p);
}

#[test]
fn send_input_reaches_a_process_reading_from_the_pty() {
    let mut p = spawn_task("/bin/sh", "read answer; echo got-$answer", Path::new("/")).unwrap();
    p.write_input(b"yes\n").unwrap();

    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if p.recent_output()
            .iter()
            .any(|line| line.contains("got-yes"))
        {
            break;
        }
        assert!(Instant::now() < deadline, "input echo did not appear");
        std::thread::sleep(Duration::from_millis(20));
    }

    let _ = wait_for_exit(&mut p);
}

#[test]
fn send_input_after_exit_is_a_noop_status() {
    let mut p = spawn_task("/bin/sh", "true", Path::new("/")).unwrap();
    let _ = wait_for_exit(&mut p);

    let result = p.send_input_if_running(b"ignored\n").unwrap();

    assert_eq!(result, portus_lib::projects::InputStatus::Ignored);
}

#[test]
fn spawn_reports_clean_exit() {
    let mut p = spawn_task("/bin/sh", "true", Path::new("/")).unwrap();
    let status = wait_for_exit(&mut p);
    assert_eq!(status.code(), Some(0));
}

#[test]
fn spawn_reports_nonzero_exit() {
    let mut p = spawn_task("/bin/sh", "exit 7", Path::new("/")).unwrap();
    let status = wait_for_exit(&mut p);
    assert_eq!(status.code(), Some(7));
}

#[test]
fn spawn_pgid_equals_child_pid() {
    let mut p = spawn_task("/bin/sh", "sleep 1", Path::new("/")).unwrap();
    assert_eq!(p.pgid(), p.pid());
    wait_for_exit(&mut p);
}

#[test]
fn kill_on_quit_kills_a_reparented_child() {
    use portus_lib::projects::kill_group;

    let mut p = spawn_task(
        "/bin/sh",
        "trap '' HUP; sleep 30 & printf 'child alive\\n'; exit 0",
        Path::new("/"),
    )
    .unwrap();
    let pgid = p.pgid();
    let _ = wait_for_exit(&mut p);

    assert_eq!(unsafe { libc::killpg(pgid as libc::pid_t, 0) }, 0);

    kill_group(pgid, libc::SIGKILL);
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if unsafe { libc::killpg(pgid as libc::pid_t, 0) } != 0 {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "re-parented child should die with the group"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn recv_log_batch_matching(
    rx: &mpsc::Receiver<InvokeResponseBody>,
    matches: impl Fn(&LogBatch) -> bool,
) -> LogBatch {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(InvokeResponseBody::Json(json)) => {
                let batch: LogBatch = serde_json::from_str(&json).unwrap();
                if matches(&batch) {
                    return batch;
                }
            }
            Ok(InvokeResponseBody::Raw(_)) => {}
            Err(mpsc::RecvTimeoutError::Timeout) => {
                assert!(Instant::now() < deadline, "log batch was not received");
            }
            Err(error) => panic!("log channel closed before a batch arrived: {error}"),
        }
    }
}

#[test]
fn save_as_candidates_includes_the_spawning_command() {
    let mut p = spawn_task("/bin/sh", "sleep 2", Path::new("/")).unwrap();
    let listener_pid = p.pid();

    let candidates = portus_lib::projects::candidates_from_chain_for_test(listener_pid);
    assert!(candidates.iter().any(|c| c.command.contains("sleep")));

    let _ = wait_for_exit(&mut p);
}
