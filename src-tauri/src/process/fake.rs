use std::collections::{HashSet, VecDeque};
use std::sync::{Arc, Mutex};

use super::{ProcessError, ProcessInfo, ProcessProbe, ProcessSignal, ProcessSnapshot};

#[derive(Clone)]
pub struct FakeProcessProbe {
    state: Arc<Mutex<FakeProcessState>>,
}

struct FakeProcessState {
    snapshots: Vec<ProcessSnapshot>,
    alive_polls: VecDeque<HashSet<u32>>,
    last_alive: HashSet<u32>,
    signal_results: Vec<(u32, ProcessSignal, bool)>,
    signals: Vec<(u32, ProcessSignal)>,
}

impl FakeProcessProbe {
    pub fn new(snapshots: Vec<ProcessSnapshot>, alive_polls: Vec<Vec<u32>>) -> Self {
        let alive_polls: VecDeque<HashSet<u32>> = alive_polls
            .into_iter()
            .map(|pids| pids.into_iter().collect())
            .collect();
        let last_alive = alive_polls.front().cloned().unwrap_or_default();

        Self {
            state: Arc::new(Mutex::new(FakeProcessState {
                snapshots,
                alive_polls,
                last_alive,
                signal_results: Vec::new(),
                signals: Vec::new(),
            })),
        }
    }

    pub fn with_signal_result(self, pid: u32, signal: ProcessSignal, result: bool) -> Self {
        self.state
            .lock()
            .expect("fake process probe lock poisoned")
            .signal_results
            .push((pid, signal, result));
        self
    }

    pub fn signals(&self) -> Vec<(u32, ProcessSignal)> {
        self.state
            .lock()
            .expect("fake process probe lock poisoned")
            .signals
            .clone()
    }
}

impl ProcessProbe for FakeProcessProbe {
    fn info_for_pids(&self, pids: &[u32]) -> Result<Vec<ProcessInfo>, ProcessError> {
        let mut state = self.state.lock().expect("fake process probe lock poisoned");
        if let Some(alive) = state.alive_polls.pop_front() {
            state.last_alive = alive;
        }

        Ok(pids
            .iter()
            .copied()
            .filter(|pid| state.last_alive.contains(pid))
            .map(process_info)
            .collect())
    }

    fn all_snapshots(&self) -> Result<Vec<ProcessSnapshot>, ProcessError> {
        Ok(self
            .state
            .lock()
            .expect("fake process probe lock poisoned")
            .snapshots
            .clone())
    }

    fn signal(&self, pid: u32, signal: ProcessSignal) -> Result<bool, ProcessError> {
        let mut state = self.state.lock().expect("fake process probe lock poisoned");
        state.signals.push((pid, signal));

        Ok(state
            .signal_results
            .iter()
            .rev()
            .find_map(|&(configured_pid, configured_signal, result)| {
                (configured_pid == pid && configured_signal == signal).then_some(result)
            })
            .unwrap_or(true))
    }
}

fn process_info(pid: u32) -> ProcessInfo {
    ProcessInfo {
        pid,
        parent_pid: None,
        name: format!("fake-{pid}"),
        command: Vec::new(),
        executable: None,
        cwd: None,
        start_time: 1,
        cpu_usage: 0.0,
        memory_bytes: 0,
    }
}
