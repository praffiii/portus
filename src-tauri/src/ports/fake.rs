use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use super::{PortListener, PortProbe, PortProbeError};

#[derive(Clone)]
pub struct FakePortProbe {
    state: Arc<Mutex<FakePortState>>,
}

struct FakePortState {
    scans: VecDeque<Vec<PortListener>>,
    last_scan: Vec<PortListener>,
}

impl FakePortProbe {
    pub fn new(listeners: Vec<PortListener>) -> Self {
        Self::with_scans(vec![listeners])
    }

    pub fn with_scans(scans: Vec<Vec<PortListener>>) -> Self {
        let scans: VecDeque<Vec<PortListener>> = scans.into();
        let last_scan = scans.front().cloned().unwrap_or_default();

        Self {
            state: Arc::new(Mutex::new(FakePortState { scans, last_scan })),
        }
    }
}

impl PortProbe for FakePortProbe {
    fn scan(&self) -> Result<Vec<PortListener>, PortProbeError> {
        let mut state = self.state.lock().expect("fake port probe lock poisoned");
        if let Some(listeners) = state.scans.pop_front() {
            state.last_scan = listeners;
        }
        Ok(state.last_scan.clone())
    }
}
