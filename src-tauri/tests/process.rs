use portus_lib::process::{
    descendants_of, KillTarget, ProcessProbe, ProcessSnapshot, SystemProcessProbe,
};

#[test]
fn system_probe_resolves_requested_pid() {
    let pid = std::process::id();

    let processes = SystemProcessProbe::default().info_for_pids(&[pid]).unwrap();

    let current = processes
        .iter()
        .find(|process| process.pid == pid)
        .expect("current process should be resolved");
    assert!(current.start_time > 0);
    assert!(!current.name.is_empty());
    assert!(current.executable.is_some());
}

#[test]
fn descendant_builder_returns_the_complete_subtree() {
    let processes = vec![
        snapshot(10, None),
        snapshot(11, Some(10)),
        snapshot(12, Some(11)),
        snapshot(20, None),
    ];

    assert_eq!(descendants_of(10, &processes), vec![11, 12]);
}

#[cfg(target_os = "macos")]
#[test]
fn kill_tree_terminates_parent_and_child_and_releases_port() {
    use std::fs;
    use std::net::{Ipv4Addr, TcpListener};
    use std::time::Duration;

    use portus_lib::ports::SystemPortProbe;

    use portus_lib::process::ProcessController;

    let ready_file = std::env::temp_dir().join(format!(
        "portus-process-tree-{}-{}.txt",
        std::process::id(),
        unique_suffix()
    ));
    let mut parent = spawn_helper("process_tree_parent_helper", &ready_file);
    let (child_pid, port) = wait_for_ready_file(&ready_file);

    let controller = ProcessController::new(
        SystemProcessProbe::default(),
        SystemPortProbe,
        Duration::from_millis(300),
        Duration::from_millis(20),
    );

    let child_info = SystemProcessProbe::default()
        .info_for_pids(&[child_pid])
        .unwrap()
        .into_iter()
        .next()
        .expect("child process should still be running");

    controller
        .kill_tree(KillTarget {
            pid: child_info.pid,
            executable: child_info.executable,
            start_time: child_info.start_time,
            expected_port: Some(port),
        })
        .unwrap();

    wait_for_child_exit(&mut parent);
    wait_for_process_exit(child_pid);
    TcpListener::bind((Ipv4Addr::LOCALHOST, port))
        .expect("port should be reusable after the process tree is killed");

    let _ = fs::remove_file(ready_file);
}

#[test]
#[ignore]
#[cfg(target_os = "macos")]
fn process_tree_parent_helper() {
    let Some(ready_file) = helper_ready_file("PORTUS_PARENT_HELPER") else {
        return;
    };

    let mut child = spawn_helper("process_tree_listener_helper", &ready_file);
    let _ = child.wait();
}

#[test]
#[ignore]
#[cfg(target_os = "macos")]
fn process_tree_listener_helper() {
    use std::fs;
    use std::io::Write;
    use std::net::{Ipv4Addr, TcpListener};
    use std::thread;
    use std::time::Duration;

    let Some(ready_file) = helper_ready_file("PORTUS_LISTENER_HELPER") else {
        return;
    };
    unsafe {
        libc::signal(libc::SIGTERM, libc::SIG_IGN);
    }
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    let mut file = fs::File::create(ready_file).unwrap();
    writeln!(file, "{} {port}", std::process::id()).unwrap();
    file.flush().unwrap();

    loop {
        thread::sleep(Duration::from_secs(1));
    }
}

fn snapshot(pid: u32, parent_pid: Option<u32>) -> ProcessSnapshot {
    ProcessSnapshot { pid, parent_pid }
}

#[cfg(target_os = "macos")]
fn unique_suffix() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
}

#[cfg(target_os = "macos")]
fn spawn_helper(test_name: &str, ready_file: &std::path::Path) -> std::process::Child {
    use std::process::{Command, Stdio};

    Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", test_name, "--nocapture"])
        .env(
            match test_name {
                "process_tree_parent_helper" => "PORTUS_PARENT_HELPER",
                _ => "PORTUS_LISTENER_HELPER",
            },
            ready_file,
        )
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap()
}

#[cfg(target_os = "macos")]
fn helper_ready_file(marker: &str) -> Option<std::path::PathBuf> {
    std::env::var_os(marker).map(std::path::PathBuf::from)
}

#[cfg(target_os = "macos")]
fn wait_for_ready_file(path: &std::path::Path) -> (u32, u16) {
    use std::fs;
    use std::thread;
    use std::time::{Duration, Instant};

    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if let Ok(contents) = fs::read_to_string(path) {
            let mut fields = contents.split_whitespace();
            let pid = fields.next().unwrap().parse().unwrap();
            let port = fields.next().unwrap().parse().unwrap();
            return (pid, port);
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!("helper did not publish its pid and port");
}

#[cfg(target_os = "macos")]
fn wait_for_child_exit(child: &mut std::process::Child) {
    use std::thread;
    use std::time::{Duration, Instant};

    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if child.try_wait().unwrap().is_some() {
            return;
        }
        thread::sleep(Duration::from_millis(20));
    }
    let _ = child.kill();
    panic!("parent process did not exit");
}

#[cfg(target_os = "macos")]
fn wait_for_process_exit(pid: u32) {
    use std::thread;
    use std::time::{Duration, Instant};

    let probe = SystemProcessProbe::default();
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if probe.info_for_pids(&[pid]).unwrap().is_empty() {
            return;
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!("child process {pid} did not exit");
}
