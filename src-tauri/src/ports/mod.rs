use std::net::SocketAddr;

use serde::Serialize;
use specta::Type;
use thiserror::Error;

mod fake;

#[cfg(target_os = "macos")]
mod macos;
mod normalize;

pub use fake::FakePortProbe;
pub use normalize::normalize;

#[cfg(target_os = "macos")]
pub use macos::SystemPortProbe;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Type)]
pub enum Protocol {
    Tcp,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Type)]
pub struct ListenerProcess {
    pub pid: u32,
    pub name: String,
    pub path: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Type)]
pub struct PortListener {
    pub protocol: Protocol,
    pub socket: SocketAddr,
    pub process: ListenerProcess,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Type)]
pub enum BindScope {
    AllInterfaces,
    Loopback,
    Specific,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Type)]
pub enum AddressFamily {
    V4,
    V6,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Type)]
pub struct PortOwner {
    pub pid: u32,
    pub name: String,
    pub path: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Type)]
pub struct PortRow {
    pub protocol: Protocol,
    pub port: u16,
    pub scope: BindScope,
    pub specific_addr: Option<String>,
    pub families: Vec<AddressFamily>,
    pub owners: Vec<PortOwner>,
    pub key: String,
}

#[derive(Debug, Error)]
pub enum PortProbeError {
    #[error("failed to scan listening ports: {0}")]
    Scan(String),
}

pub trait PortProbe: Send + Sync {
    fn scan(&self) -> Result<Vec<PortListener>, PortProbeError>;
}
