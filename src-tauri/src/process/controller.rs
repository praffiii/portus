use std::thread;
use std::time::{Duration, Instant};

use crate::ports::PortProbe;

use super::{descendants_of, ProcessError, ProcessProbe, ProcessSignal};

pub struct ProcessController<P, Q> {
    process_probe: P,
    port_probe: Q,
    grace_period: Duration,
    poll_interval: Duration,
}

impl<P, Q> ProcessController<P, Q>
where
    P: ProcessProbe,
    Q: PortProbe,
{
    pub fn new(
        process_probe: P,
        port_probe: Q,
        grace_period: Duration,
        poll_interval: Duration,
    ) -> Self {
        Self {
            process_probe,
            port_probe,
            grace_period,
            poll_interval,
        }
    }

    pub fn kill_tree(&self, root_pid: u32, expected_port: Option<u16>) -> Result<(), ProcessError> {
        if self.process_probe.info_for_pids(&[root_pid])?.is_empty() {
            return Err(ProcessError::NotFound(root_pid));
        }

        let snapshots = self.process_probe.all_snapshots()?;
        let mut tree = vec![root_pid];
        tree.extend(descendants_of(root_pid, &snapshots));

        self.signal_all(&tree, ProcessSignal::Terminate)?;
        let remaining = self.wait_for_exit(&tree, self.grace_period)?;
        if !remaining.is_empty() {
            self.signal_all(&remaining, ProcessSignal::Kill)?;
            let remaining = self.wait_for_exit(&remaining, self.grace_period)?;
            if !remaining.is_empty() {
                return Err(ProcessError::StillRunning(remaining));
            }
        }

        if let Some(port) = expected_port {
            self.verify_port_released(port)?;
        }
        Ok(())
    }

    fn signal_all(&self, pids: &[u32], signal: ProcessSignal) -> Result<(), ProcessError> {
        for &pid in pids {
            if !self.process_probe.signal(pid, signal)? {
                return Err(ProcessError::SignalFailed { pid, signal });
            }
        }
        Ok(())
    }

    fn wait_for_exit(&self, pids: &[u32], timeout: Duration) -> Result<Vec<u32>, ProcessError> {
        let deadline = Instant::now() + timeout;
        loop {
            let live: Vec<u32> = self
                .process_probe
                .info_for_pids(pids)?
                .into_iter()
                .map(|process| process.pid)
                .collect();
            if live.is_empty() || Instant::now() >= deadline {
                return Ok(live);
            }
            thread::sleep(self.poll_interval);
        }
    }

    fn verify_port_released(&self, port: u16) -> Result<(), ProcessError> {
        let deadline = Instant::now() + self.grace_period;
        loop {
            let still_listening = self
                .port_probe
                .scan()
                .map_err(|error| ProcessError::PortVerification(error.to_string()))?
                .iter()
                .any(|listener| listener.socket.port() == port);
            if !still_listening {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(ProcessError::PortStillListening(port));
            }
            thread::sleep(self.poll_interval);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    use std::time::Duration;

    use crate::ports::{FakePortProbe, ListenerProcess, PortListener, Protocol};

    use super::*;
    use crate::process::{FakeProcessProbe, ProcessSnapshot};

    const ROOT_PID: u32 = 100;
    const CHILD_PID: u32 = 101;
    const GRANDCHILD_PID: u32 = 102;
    const PORT: u16 = 3000;

    #[test]
    fn kill_tree_returns_not_found_when_root_is_missing() {
        let process_probe = FakeProcessProbe::new(vec![], vec![vec![]]);
        let controller = controller(process_probe, released_port_probe());

        let error = controller.kill_tree(ROOT_PID, Some(PORT)).unwrap_err();

        assert!(matches!(error, ProcessError::NotFound(ROOT_PID)));
    }

    #[test]
    fn kill_tree_returns_signal_failed_when_signal_is_rejected() {
        let process_probe =
            FakeProcessProbe::new(vec![snapshot(ROOT_PID, None)], vec![vec![ROOT_PID]])
                .with_signal_result(ROOT_PID, ProcessSignal::Terminate, false);
        let controller = controller(process_probe, released_port_probe());

        let error = controller.kill_tree(ROOT_PID, None).unwrap_err();

        assert!(matches!(
            error,
            ProcessError::SignalFailed {
                pid: ROOT_PID,
                signal: ProcessSignal::Terminate
            }
        ));
    }

    #[test]
    fn kill_tree_terminates_the_whole_tree_during_grace() {
        let snapshots = vec![
            snapshot(ROOT_PID, None),
            snapshot(CHILD_PID, Some(ROOT_PID)),
            snapshot(GRANDCHILD_PID, Some(CHILD_PID)),
        ];
        let process_probe = FakeProcessProbe::new(
            snapshots,
            vec![vec![ROOT_PID, CHILD_PID, GRANDCHILD_PID], vec![]],
        );
        let inspector = process_probe.clone();
        let controller = controller(process_probe, released_port_probe());

        controller.kill_tree(ROOT_PID, Some(PORT)).unwrap();

        assert_eq!(
            inspector.signals(),
            vec![
                (ROOT_PID, ProcessSignal::Terminate),
                (CHILD_PID, ProcessSignal::Terminate),
                (GRANDCHILD_PID, ProcessSignal::Terminate),
            ]
        );
    }

    #[test]
    fn kill_tree_escalates_surviving_processes_to_kill() {
        let process_probe = FakeProcessProbe::new(
            vec![snapshot(ROOT_PID, None)],
            vec![vec![ROOT_PID], vec![ROOT_PID], vec![]],
        );
        let inspector = process_probe.clone();
        let controller = controller(process_probe, released_port_probe());

        controller.kill_tree(ROOT_PID, None).unwrap();

        assert_eq!(
            inspector.signals(),
            vec![
                (ROOT_PID, ProcessSignal::Terminate),
                (ROOT_PID, ProcessSignal::Kill),
            ]
        );
    }

    #[test]
    fn kill_tree_returns_still_running_after_escalation() {
        let process_probe = FakeProcessProbe::new(
            vec![snapshot(ROOT_PID, None)],
            vec![vec![ROOT_PID], vec![ROOT_PID], vec![ROOT_PID]],
        );
        let controller = controller(process_probe, released_port_probe());

        let error = controller.kill_tree(ROOT_PID, None).unwrap_err();

        assert!(matches!(
            error,
            ProcessError::StillRunning(ref pids) if pids == &[ROOT_PID]
        ));
    }

    #[test]
    fn kill_tree_returns_port_still_listening_after_deadline() {
        let process_probe =
            FakeProcessProbe::new(vec![snapshot(ROOT_PID, None)], vec![vec![ROOT_PID], vec![]]);
        let controller = controller(process_probe, listening_port_probe(PORT));

        let error = controller.kill_tree(ROOT_PID, Some(PORT)).unwrap_err();

        assert!(matches!(error, ProcessError::PortStillListening(PORT)));
    }

    #[test]
    fn kill_tree_without_expected_port_skips_port_verification() {
        let process_probe =
            FakeProcessProbe::new(vec![snapshot(ROOT_PID, None)], vec![vec![ROOT_PID], vec![]]);
        let controller = controller(process_probe, listening_port_probe(PORT));

        controller.kill_tree(ROOT_PID, None).unwrap();
    }

    fn controller(
        process_probe: FakeProcessProbe,
        port_probe: FakePortProbe,
    ) -> ProcessController<FakeProcessProbe, FakePortProbe> {
        ProcessController::new(process_probe, port_probe, Duration::ZERO, Duration::ZERO)
    }

    fn snapshot(pid: u32, parent_pid: Option<u32>) -> ProcessSnapshot {
        ProcessSnapshot { pid, parent_pid }
    }

    fn released_port_probe() -> FakePortProbe {
        FakePortProbe::new(vec![])
    }

    fn listening_port_probe(port: u16) -> FakePortProbe {
        FakePortProbe::new(vec![PortListener {
            protocol: Protocol::Tcp,
            socket: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port),
            process: ListenerProcess {
                pid: ROOT_PID,
                name: "test".to_string(),
                path: "/test".to_string(),
            },
        }])
    }
}
