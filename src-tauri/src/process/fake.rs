use std::collections::{HashMap, HashSet, VecDeque};
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
    signal_errors: Vec<(u32, ProcessSignal, ProcessError)>,
    signals: Vec<(u32, ProcessSignal)>,
    process_infos: HashMap<u32, ProcessInfo>,
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
                signal_errors: Vec::new(),
                signals: Vec::new(),
                process_infos: HashMap::new(),
            })),
        }
    }

    pub fn with_signal_error(self, pid: u32, signal: ProcessSignal, error: ProcessError) -> Self {
        self.state
            .lock()
            .expect("fake process probe lock poisoned")
            .signal_errors
            .push((pid, signal, error));
        self
    }

    pub fn with_process_info(self, pid: u32, info: ProcessInfo) -> Self {
        self.state
            .lock()
            .expect("fake process probe lock poisoned")
            .process_infos
            .insert(pid, info);
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
            .map(|pid| {
                state
                    .process_infos
                    .get(&pid)
                    .cloned()
                    .unwrap_or_else(|| process_info(pid))
            })
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

    fn signal(&self, pid: u32, signal: ProcessSignal) -> Result<(), ProcessError> {
        let mut state = self.state.lock().expect("fake process probe lock poisoned");
        state.signals.push((pid, signal));

        if let Some(error) = state.signal_errors.iter().rev().find_map(
            |(configured_pid, configured_signal, error)| {
                (*configured_pid == pid && *configured_signal == signal).then_some(error.clone())
            },
        ) {
            Err(error)
        } else {
            Ok(())
        }
    }
}

fn process_info(pid: u32) -> ProcessInfo {
    ProcessInfo {
        pid,
        parent_pid: None,
        name: format!("fake-{pid}"),
        command: Vec::new(),
        executable: Some("/fake".to_string()),
        cwd: None,
        start_time: 1,
        cpu_usage: 0.0,
        memory_bytes: 0,
    }
}
