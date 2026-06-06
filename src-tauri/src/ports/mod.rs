use std::net::SocketAddr;

use serde::Serialize;
use thiserror::Error;

mod fake;

#[cfg(target_os = "macos")]
mod macos;

pub use fake::FakePortProbe;

#[cfg(target_os = "macos")]
pub use macos::SystemPortProbe;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum Protocol {
    Tcp,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ListenerProcess {
    pub pid: u32,
    pub name: String,
    pub path: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PortListener {
    pub protocol: Protocol,
    pub socket: SocketAddr,
    pub process: ListenerProcess,
}

#[derive(Debug, Error)]
pub enum PortProbeError {
    #[error("failed to scan listening ports: {0}")]
    Scan(String),
}

pub trait PortProbe: Send + Sync {
    fn scan(&self) -> Result<Vec<PortListener>, PortProbeError>;
}
