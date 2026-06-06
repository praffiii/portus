use super::{ListenerProcess, PortListener, PortProbe, PortProbeError, Protocol};

pub struct SystemPortProbe;

impl PortProbe for SystemPortProbe {
    fn scan(&self) -> Result<Vec<PortListener>, PortProbeError> {
        let listeners =
            listeners::get_all().map_err(|error| PortProbeError::Scan(error.to_string()))?;

        Ok(listeners
            .into_iter()
            .filter(|listener| listener.protocol == listeners::Protocol::TCP)
            .map(|listener| PortListener {
                protocol: Protocol::Tcp,
                socket: listener.socket,
                process: ListenerProcess {
                    pid: listener.process.pid,
                    name: listener.process.name,
                    path: listener.process.path,
                },
            })
            .collect())
    }
}
