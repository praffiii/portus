use super::{PortListener, PortProbe, PortProbeError};

pub struct FakePortProbe {
    listeners: Vec<PortListener>,
}

impl FakePortProbe {
    pub fn new(listeners: Vec<PortListener>) -> Self {
        Self { listeners }
    }
}

impl PortProbe for FakePortProbe {
    fn scan(&self) -> Result<Vec<PortListener>, PortProbeError> {
        Ok(self.listeners.clone())
    }
}
