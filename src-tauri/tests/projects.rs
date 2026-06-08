#![cfg(unix)]

use std::path::Path;
use std::time::{Duration, Instant};

use portus_lib::projects::spawn_task;

fn wait_for_exit(p: &mut portus_lib::projects::SpawnedProcess) -> std::process::ExitStatus {
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

    let mut p = spawn_task("/bin/sh", "sleep 30 & exit 0", Path::new("/")).unwrap();
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

#[test]
fn save_as_candidates_includes_the_spawning_command() {
    let mut p = spawn_task("/bin/sh", "sleep 2", Path::new("/")).unwrap();
    let listener_pid = p.pid();

    let candidates = portus_lib::projects::candidates_from_chain_for_test(listener_pid);
    assert!(candidates.iter().any(|c| c.command.contains("sleep")));

    let _ = wait_for_exit(&mut p);
}
