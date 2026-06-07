use serde::Serialize;
use specta::Type;
use thiserror::Error;

mod controller;
mod descendants;
mod fake;
mod system;

pub use controller::{KillTarget, ProcessController};
pub use descendants::descendants_of;
pub use fake::FakeProcessProbe;
pub use system::SystemProcessProbe;

#[derive(Clone, Debug, PartialEq, Serialize, Type)]
pub struct ProcessInfo {
    pub pid: u32,
    pub parent_pid: Option<u32>,
    pub name: String,
    pub command: Vec<String>,
    pub executable: Option<String>,
    pub cwd: Option<String>,
    pub start_time: u64,
    pub cpu_usage: f32,
    pub memory_bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProcessSnapshot {
    pub pid: u32,
    pub parent_pid: Option<u32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcessSignal {
    Terminate,
    Kill,
}

#[derive(Debug, Error)]
pub enum ProcessError {
    #[error("failed to inspect processes: {0}")]
    Inspect(String),
    #[error("process {0} is no longer running")]
    NotFound(u32),
    #[error("process {pid} no longer matches the selected row")]
    IdentityMismatch { pid: u32 },
    #[error("process {pid} no longer owns port {port}")]
    PortOwnerMismatch { pid: u32, port: u16 },
    #[error("failed to send {signal:?} to process {pid}")]
    SignalFailed { pid: u32, signal: ProcessSignal },
    #[error("processes are still running after signal escalation: {0:?}")]
    StillRunning(Vec<u32>),
    #[error("port {0} is still listening after the process tree was killed")]
    PortStillListening(u16),
    #[error("failed to verify port release: {0}")]
    PortVerification(String),
}

pub trait ProcessProbe: Send + Sync {
    fn info_for_pids(&self, pids: &[u32]) -> Result<Vec<ProcessInfo>, ProcessError>;

    fn all_snapshots(&self) -> Result<Vec<ProcessSnapshot>, ProcessError>;

    fn signal(&self, pid: u32, signal: ProcessSignal) -> Result<bool, ProcessError>;
}
