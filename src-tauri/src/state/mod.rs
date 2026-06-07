use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_specta::Event;
use tokio::sync::watch;

use crate::docker::{DockerBackend, DockerProbe, DockerSnapshot, SystemDockerBackend};
use crate::ports::{normalize, PortProbe, PortRow};
use crate::process::{KillTarget, ProcessController, ProcessError, ProcessInfo, ProcessProbe};

pub const SNAPSHOT_EVENT: &str = "snapshot";
pub const ACTIVE_INTERVAL: Duration = Duration::from_secs(2);
pub const IDLE_INTERVAL: Duration = Duration::from_secs(8);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PollMode {
    Active,
    Idle,
}

#[derive(Clone)]
pub struct PollState {
    active_interval: Duration,
    idle_interval: Duration,
    mode_tx: watch::Sender<PollMode>,
}

impl PollState {
    pub fn new(active_interval: Duration, idle_interval: Duration) -> Self {
        let (mode_tx, _) = watch::channel(PollMode::Idle);
        Self {
            active_interval,
            idle_interval,
            mode_tx,
        }
    }

    pub fn mode(&self) -> PollMode {
        *self.mode_tx.borrow()
    }

    pub fn current_interval(&self) -> Duration {
        match self.mode() {
            PollMode::Active => self.active_interval,
            PollMode::Idle => self.idle_interval,
        }
    }

    pub fn set_active(&self) {
        self.mode_tx.send_replace(PollMode::Active);
    }

    pub fn set_idle(&self) {
        self.mode_tx.send_replace(PollMode::Idle);
    }

    fn subscribe(&self) -> watch::Receiver<PollMode> {
        self.mode_tx.subscribe()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Type)]
pub struct SnapshotSection<T> {
    pub data: T,
    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Type, Event)]
#[tauri_specta(event_name = "snapshot")]
pub struct Snapshot {
    pub ports: SnapshotSection<Vec<PortRow>>,
    pub processes: SnapshotSection<Vec<ProcessInfo>>,
    pub docker: SnapshotSection<DockerSnapshot>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum KillProcessError {
    NeedsElevatedPrivileges { pid: u32 },
    ProcessChanged { pid: u32 },
    PortChanged { pid: u32, port: u16 },
    StillRunning { pids: Vec<u32> },
    PortStillListening { port: u16 },
    Failed { message: String },
}

pub trait SnapshotEmitter: Send + Sync {
    fn emit_snapshot(&self, event: &'static str, snapshot: Snapshot) -> Result<(), String>;
}

pub struct SnapshotBuilder<P, Q, R> {
    port_probe: P,
    process_probe: Q,
    docker_probe: DockerProbe<R>,
}

impl<P, Q, R> SnapshotBuilder<P, Q, R>
where
    P: PortProbe,
    Q: ProcessProbe,
    R: DockerBackend,
{
    pub fn new(port_probe: P, process_probe: Q, docker_backend: R) -> Self {
        Self {
            port_probe,
            process_probe,
            docker_probe: DockerProbe::new(docker_backend),
        }
    }

    pub async fn poll_and_emit<E: SnapshotEmitter>(&self, emitter: &E) {
        let snapshot = self.poll().await;
        let _ = emitter.emit_snapshot(SNAPSHOT_EVENT, snapshot);
    }

    pub async fn poll(&self) -> Snapshot {
        let docker = match self.docker_probe.snapshot().await {
            Ok(data) => SnapshotSection { data, error: None },
            Err(error) => SnapshotSection {
                data: DockerSnapshot::default(),
                error: Some(error.to_string()),
            },
        };
        let published_ports = published_container_ports(&docker.data);
        let ports = isolated_probe("port", || {
            self.port_probe.scan().map(|listeners| {
                normalize(listeners)
                    .into_iter()
                    .filter(|row| !is_docker_proxy_row(row, &published_ports))
                    .collect::<Vec<_>>()
            })
        });
        let mut pids: Vec<u32> = ports
            .data
            .iter()
            .flat_map(|row| row.owners.iter().map(|owner| owner.pid))
            .collect();
        pids.sort_unstable();
        pids.dedup();
        let processes = isolated_probe("process", || self.process_probe.info_for_pids(&pids));

        Snapshot {
            ports,
            processes,
            docker,
        }
    }
}

pub async fn run_poll_loop<P, Q, R, E>(
    builder: Arc<SnapshotBuilder<P, Q, R>>,
    emitter: Arc<E>,
    state: PollState,
) where
    P: PortProbe + 'static,
    Q: ProcessProbe + 'static,
    R: DockerBackend + 'static,
    E: SnapshotEmitter + 'static,
{
    let mut mode_rx = state.subscribe();
    loop {
        builder.poll_and_emit(emitter.as_ref()).await;
        tokio::select! {
            _ = tokio::time::sleep(state.current_interval()) => {}
            changed = mode_rx.changed() => {
                if changed.is_err() {
                    return;
                }
            }
        }
    }
}

#[tauri::command]
#[specta::specta]
pub fn set_active(state: State<'_, PollState>) {
    state.set_active();
}

#[tauri::command]
#[specta::specta]
pub fn set_idle(state: State<'_, PollState>) {
    state.set_idle();
}

#[cfg(target_os = "macos")]
#[tauri::command]
#[specta::specta]
pub fn kill_process_tree(target: KillTarget) -> Result<(), KillProcessError> {
    use crate::ports::SystemPortProbe;
    use crate::process::SystemProcessProbe;

    ProcessController::new(
        SystemProcessProbe::default(),
        SystemPortProbe,
        Duration::from_millis(300),
        Duration::from_millis(20),
    )
    .kill_tree(target)
    .map_err(KillProcessError::from)
}

impl From<ProcessError> for KillProcessError {
    fn from(error: ProcessError) -> Self {
        match error {
            ProcessError::PermissionDenied { pid } => Self::NeedsElevatedPrivileges { pid },
            ProcessError::NotFound(pid) | ProcessError::IdentityMismatch { pid } => {
                Self::ProcessChanged { pid }
            }
            ProcessError::PortOwnerMismatch { pid, port } => Self::PortChanged { pid, port },
            ProcessError::StillRunning(pids) => Self::StillRunning { pids },
            ProcessError::PortStillListening(port) => Self::PortStillListening { port },
            ProcessError::Inspect(message) | ProcessError::PortVerification(message) => {
                Self::Failed { message }
            }
            ProcessError::SignalFailed { pid, signal } => Self::Failed {
                message: format!("failed to send {signal:?} to process {pid}"),
            },
        }
    }
}

#[cfg(target_os = "macos")]
pub fn start(app: AppHandle) {
    use crate::ports::SystemPortProbe;
    use crate::process::SystemProcessProbe;

    let state = PollState::new(ACTIVE_INTERVAL, IDLE_INTERVAL);
    app.manage(state.clone());

    let builder = Arc::new(SnapshotBuilder::new(
        SystemPortProbe,
        SystemProcessProbe::default(),
        SystemDockerBackend::default(),
    ));
    let emitter = Arc::new(TauriSnapshotEmitter { app });
    tauri::async_runtime::spawn(run_poll_loop(builder, emitter, state));
}

fn published_container_ports(snapshot: &DockerSnapshot) -> Vec<u16> {
    let mut ports: Vec<u16> = snapshot
        .containers
        .iter()
        .flat_map(|container| published_ports_from_status(&container.status))
        .collect();
    ports.sort_unstable();
    ports.dedup();
    ports
}

fn published_ports_from_status(status: &str) -> impl Iterator<Item = u16> + '_ {
    status
        .split(',')
        .filter_map(|segment| segment.trim().split_once("->"))
        .filter_map(|(host, _container)| host.rsplit(':').next())
        .filter_map(|port| port.parse::<u16>().ok())
}

fn is_docker_proxy_row(row: &PortRow, published_ports: &[u16]) -> bool {
    published_ports.binary_search(&row.port).is_ok()
        && row.owners.iter().any(|owner| {
            let name = owner.name.to_lowercase();
            let path = owner.path.to_lowercase();
            name.contains("docker") || path.contains("docker.app") || path.contains("/docker/")
        })
}

struct TauriSnapshotEmitter {
    app: AppHandle,
}

impl SnapshotEmitter for TauriSnapshotEmitter {
    fn emit_snapshot(&self, event: &'static str, snapshot: Snapshot) -> Result<(), String> {
        self.app
            .emit(event, snapshot)
            .map_err(|error| error.to_string())
    }
}

fn isolated_probe<T, E, F>(name: &str, probe: F) -> SnapshotSection<T>
where
    T: Default,
    E: std::fmt::Display,
    F: FnOnce() -> Result<T, E>,
{
    match catch_unwind(AssertUnwindSafe(probe)) {
        Ok(Ok(data)) => SnapshotSection { data, error: None },
        Ok(Err(error)) => SnapshotSection {
            data: T::default(),
            error: Some(error.to_string()),
        },
        Err(_) => SnapshotSection {
            data: T::default(),
            error: Some(format!("{name} probe panicked")),
        },
    }
}
