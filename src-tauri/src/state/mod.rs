use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_specta::Event;
use tokio::sync::watch;

use crate::ports::{PortListener, PortProbe};
use crate::process::{ProcessInfo, ProcessProbe};

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
    pub ports: SnapshotSection<Vec<PortListener>>,
    pub processes: SnapshotSection<Vec<ProcessInfo>>,
}

pub trait SnapshotEmitter: Send + Sync {
    fn emit_snapshot(&self, event: &'static str, snapshot: Snapshot) -> Result<(), String>;
}

pub struct SnapshotBuilder<P, Q> {
    port_probe: P,
    process_probe: Q,
}

impl<P, Q> SnapshotBuilder<P, Q>
where
    P: PortProbe,
    Q: ProcessProbe,
{
    pub fn new(port_probe: P, process_probe: Q) -> Self {
        Self {
            port_probe,
            process_probe,
        }
    }

    pub fn poll_and_emit<E: SnapshotEmitter>(&self, emitter: &E) {
        let snapshot = self.poll();
        let _ = emitter.emit_snapshot(SNAPSHOT_EVENT, snapshot);
    }

    pub fn poll(&self) -> Snapshot {
        let ports = isolated_probe("port", || self.port_probe.scan());
        let mut pids: Vec<u32> = ports
            .data
            .iter()
            .map(|listener| listener.process.pid)
            .collect();
        pids.sort_unstable();
        pids.dedup();
        let processes = isolated_probe("process", || self.process_probe.info_for_pids(&pids));

        Snapshot { ports, processes }
    }
}

pub async fn run_poll_loop<P, Q, E>(
    builder: Arc<SnapshotBuilder<P, Q>>,
    emitter: Arc<E>,
    state: PollState,
) where
    P: PortProbe + 'static,
    Q: ProcessProbe + 'static,
    E: SnapshotEmitter + 'static,
{
    let mut mode_rx = state.subscribe();
    loop {
        builder.poll_and_emit(emitter.as_ref());
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
pub fn start(app: AppHandle) {
    use crate::ports::SystemPortProbe;
    use crate::process::SystemProcessProbe;

    let state = PollState::new(ACTIVE_INTERVAL, IDLE_INTERVAL);
    app.manage(state.clone());

    let builder = Arc::new(SnapshotBuilder::new(
        SystemPortProbe,
        SystemProcessProbe::default(),
    ));
    let emitter = Arc::new(TauriSnapshotEmitter { app });
    tauri::async_runtime::spawn(run_poll_loop(builder, emitter, state));
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
