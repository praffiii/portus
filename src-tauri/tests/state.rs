use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use portus_lib::docker::{DockerBackendError, FakeDockerBackend, RawContainer};
use portus_lib::ports::{
    AddressFamily, BindScope, FakePortProbe, ListenerProcess, PortListener, PortOwner, PortProbe,
    PortProbeError, PortRow, Protocol,
};
use portus_lib::process::{
    FakeProcessProbe, ProcessError, ProcessInfo, ProcessProbe, ProcessSignal, ProcessSnapshot,
};
use portus_lib::state::{
    run_poll_loop, KillProcessError, PollMode, PollState, SnapshotBuilder, SnapshotEmitter,
    SNAPSHOT_EVENT,
};
use serde_json::json;

#[test]
fn cadence_switches_between_idle_and_active() {
    let state = PollState::new(Duration::from_secs(2), Duration::from_secs(8));

    assert_eq!(state.mode(), PollMode::Idle);
    assert_eq!(state.current_interval(), Duration::from_secs(8));

    state.set_active();
    assert_eq!(state.mode(), PollMode::Active);
    assert_eq!(state.current_interval(), Duration::from_secs(2));

    state.set_idle();
    assert_eq!(state.mode(), PollMode::Idle);
    assert_eq!(state.current_interval(), Duration::from_secs(8));
}

#[test]
fn permission_denied_process_error_maps_to_elevated_privileges_action_state() {
    let error = KillProcessError::from(ProcessError::PermissionDenied { pid: 42 });

    assert_eq!(error, KillProcessError::NeedsElevatedPrivileges { pid: 42 });
}

#[tokio::test]
async fn active_mode_interrupts_the_idle_wait() {
    let state = PollState::new(Duration::from_secs(2), Duration::from_secs(30));
    let emitter = Arc::new(RecordingEmitter::default());
    let builder = Arc::new(SnapshotBuilder::new(
        FakePortProbe::new(vec![]),
        FakeProcessProbe::new(vec![], vec![vec![], vec![]]),
        no_docker(),
    ));
    let task = tokio::spawn(run_poll_loop(builder, emitter.clone(), state.clone()));

    emitter.wait_for_count(1).await;
    state.set_active();
    tokio::time::timeout(Duration::from_millis(200), emitter.wait_for_count(2))
        .await
        .expect("active mode should interrupt the idle wait");

    task.abort();
}

#[tokio::test]
async fn poll_emits_ports_and_processes_in_one_snapshot() {
    let ports = vec![listener(3000, 42)];
    let processes = FakeProcessProbe::new(
        vec![ProcessSnapshot {
            pid: 42,
            parent_pid: None,
        }],
        vec![vec![42]],
    );
    let emitter = RecordingEmitter::default();
    let builder = SnapshotBuilder::new(FakePortProbe::new(ports.clone()), processes, no_docker());

    builder.poll_and_emit(&emitter).await;

    let snapshots = emitter.snapshots();
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].ports.data, vec![port_row(3000, 42)]);
    assert_eq!(snapshots[0].processes.data.len(), 1);
    assert_eq!(snapshots[0].processes.data[0].pid, 42);
    assert_eq!(emitter.events(), vec![SNAPSHOT_EVENT]);
}

#[tokio::test]
async fn docker_proxy_listener_is_reconciled_into_the_container_row() {
    let docker_proxy = PortListener {
        protocol: Protocol::Tcp,
        socket: "0.0.0.0:8080".parse().unwrap(),
        process: ListenerProcess {
            pid: 101,
            name: "com.docker.backend".to_string(),
            path: "/Applications/Docker.app/Contents/MacOS/com.docker.backend".to_string(),
        },
    };
    let node = listener(5173, 42);
    let docker = FakeDockerBackend::new(vec![Ok(vec![RawContainer {
        id: "abc123".to_string(),
        names: vec!["/web".to_string()],
        image: "nginx:latest".to_string(),
        state: "running".to_string(),
        status: "Up 3 seconds, 0.0.0.0:8080->80/tcp".to_string(),
    }])]);
    let processes = FakeProcessProbe::new(
        vec![ProcessSnapshot {
            pid: 42,
            parent_pid: None,
        }],
        vec![vec![42]],
    );
    let builder = SnapshotBuilder::new(
        FakePortProbe::new(vec![docker_proxy, node]),
        processes,
        docker,
    );

    let snapshot = builder.poll().await;

    assert_eq!(snapshot.docker.data.containers.len(), 1);
    assert_eq!(snapshot.docker.data.containers[0].names, vec!["/web"]);
    assert_eq!(snapshot.ports.data, vec![port_row(5173, 42)]);
    assert_eq!(snapshot.processes.data.len(), 1);
}

#[test]
fn snapshot_serializes_process_fields_as_human_readable_strings() {
    let snapshot = portus_lib::state::Snapshot {
        ports: portus_lib::state::SnapshotSection {
            data: vec![],
            error: None,
        },
        processes: portus_lib::state::SnapshotSection {
            data: vec![ProcessInfo {
                pid: 42,
                parent_pid: None,
                name: "node".to_string(),
                command: vec!["node".to_string(), "server.js".to_string()],
                executable: Some("/usr/local/bin/node".to_string()),
                cwd: Some("/Users/dev/project".to_string()),
                start_time: 1,
                cpu_usage: 0.0,
                memory_bytes: 0,
            }],
            error: None,
        },
        docker: portus_lib::state::SnapshotSection {
            data: Default::default(),
            error: None,
        },
    };

    let value = serde_json::to_value(snapshot).unwrap();

    assert_eq!(value["processes"]["data"][0]["name"], json!("node"));
    assert_eq!(
        value["processes"]["data"][0]["command"],
        json!(["node", "server.js"])
    );
    assert_eq!(
        value["processes"]["data"][0]["executable"],
        json!("/usr/local/bin/node")
    );
}

#[tokio::test]
async fn duplicate_listener_pids_are_resolved_once() {
    let requested = Arc::new(Mutex::new(Vec::new()));
    let process_probe = RecordingProcessProbe {
        requested: requested.clone(),
    };
    let builder = SnapshotBuilder::new(
        FakePortProbe::new(vec![listener(3000, 42), listener(3001, 42)]),
        process_probe,
        no_docker(),
    );

    builder.poll().await;

    assert_eq!(*requested.lock().unwrap(), vec![42]);
}

#[tokio::test]
async fn panicking_process_probe_does_not_blank_ports_or_skip_emit() {
    let ports = vec![listener(3000, 42)];
    let emitter = RecordingEmitter::default();
    let builder = SnapshotBuilder::new(
        FakePortProbe::new(ports.clone()),
        PanickingProcessProbe,
        no_docker(),
    );

    builder.poll_and_emit(&emitter).await;
    let snapshots = emitter.snapshots();
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].ports.data, vec![port_row(3000, 42)]);
    assert!(snapshots[0].ports.error.is_none());
    assert!(snapshots[0].processes.data.is_empty());
    assert_eq!(
        snapshots[0].processes.error.as_deref(),
        Some("process probe panicked")
    );
}

#[tokio::test]
async fn panicking_port_probe_still_emits_a_snapshot() {
    let emitter = RecordingEmitter::default();
    let builder = SnapshotBuilder::new(
        PanickingPortProbe,
        FakeProcessProbe::new(vec![], vec![vec![]]),
        no_docker(),
    );

    builder.poll_and_emit(&emitter).await;

    let snapshots = emitter.snapshots();
    assert_eq!(snapshots.len(), 1);
    assert!(snapshots[0].ports.data.is_empty());
    assert_eq!(
        snapshots[0].ports.error.as_deref(),
        Some("port probe panicked")
    );
    assert!(snapshots[0].processes.error.is_none());
}

#[derive(Clone, Default)]
struct RecordingEmitter {
    events: Arc<Mutex<Vec<&'static str>>>,
    snapshots: Arc<Mutex<Vec<portus_lib::state::Snapshot>>>,
    count: Arc<AtomicUsize>,
    emitted: Arc<tokio::sync::Notify>,
}

impl RecordingEmitter {
    fn events(&self) -> Vec<&'static str> {
        self.events.lock().unwrap().clone()
    }

    fn snapshots(&self) -> Vec<portus_lib::state::Snapshot> {
        self.snapshots.lock().unwrap().clone()
    }

    async fn wait_for_count(&self, expected: usize) {
        while self.count.load(Ordering::SeqCst) < expected {
            self.emitted.notified().await;
        }
    }
}

impl SnapshotEmitter for RecordingEmitter {
    fn emit_snapshot(
        &self,
        event: &'static str,
        snapshot: portus_lib::state::Snapshot,
    ) -> Result<(), String> {
        self.events.lock().unwrap().push(event);
        self.snapshots.lock().unwrap().push(snapshot);
        self.count.fetch_add(1, Ordering::SeqCst);
        self.emitted.notify_one();
        Ok(())
    }
}

struct PanickingPortProbe;

impl PortProbe for PanickingPortProbe {
    fn scan(&self) -> Result<Vec<PortListener>, PortProbeError> {
        panic!("port probe failure");
    }
}

struct PanickingProcessProbe;

impl ProcessProbe for PanickingProcessProbe {
    fn info_for_pids(&self, _pids: &[u32]) -> Result<Vec<ProcessInfo>, ProcessError> {
        panic!("process probe failure");
    }

    fn all_snapshots(&self) -> Result<Vec<ProcessSnapshot>, ProcessError> {
        unreachable!()
    }

    fn signal(&self, _pid: u32, _signal: ProcessSignal) -> Result<(), ProcessError> {
        unreachable!()
    }
}

struct RecordingProcessProbe {
    requested: Arc<Mutex<Vec<u32>>>,
}

impl ProcessProbe for RecordingProcessProbe {
    fn info_for_pids(&self, pids: &[u32]) -> Result<Vec<ProcessInfo>, ProcessError> {
        self.requested.lock().unwrap().extend_from_slice(pids);
        Ok(vec![])
    }

    fn all_snapshots(&self) -> Result<Vec<ProcessSnapshot>, ProcessError> {
        unreachable!()
    }

    fn signal(&self, _pid: u32, _signal: ProcessSignal) -> Result<(), ProcessError> {
        unreachable!()
    }
}

fn listener(port: u16, pid: u32) -> PortListener {
    PortListener {
        protocol: Protocol::Tcp,
        socket: format!("127.0.0.1:{port}").parse().unwrap(),
        process: ListenerProcess {
            pid,
            name: "node".to_string(),
            path: "/usr/local/bin/node".to_string(),
        },
    }
}

fn port_row(port: u16, pid: u32) -> PortRow {
    PortRow {
        protocol: Protocol::Tcp,
        port,
        scope: BindScope::Loopback,
        specific_addr: None,
        families: vec![AddressFamily::V4],
        owners: vec![PortOwner {
            pid,
            name: "node".to_string(),
            path: "/usr/local/bin/node".to_string(),
        }],
        key: format!("tcp|loopback|{port}|{pid}"),
    }
}

fn no_docker() -> FakeDockerBackend {
    FakeDockerBackend::new(vec![Err(DockerBackendError::NotDetected)])
}
