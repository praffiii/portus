use std::time::Duration;

use serde::Serialize;
use specta::Type;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum Lifecycle {
    Starting,
    Running,
    RunningNoPort,
    Exited,
    Crashed,
}

/// Exit signal derived from `SpawnedProcess::try_status`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExitState {
    Alive,
    Exited(i32),
    /// Terminated by a signal (no exit code), for example our own kill-on-quit.
    Signaled,
}

/// Pure classifier. `elapsed` is time since spawn; `grace` is the no-port grace
/// window; `holds_port` is whether any listening port is owned by this group.
pub fn classify(
    elapsed: Duration,
    grace: Duration,
    exit: ExitState,
    holds_port: bool,
) -> Lifecycle {
    match exit {
        ExitState::Exited(0) | ExitState::Signaled => Lifecycle::Exited,
        ExitState::Exited(_) => Lifecycle::Crashed,
        ExitState::Alive => {
            if holds_port {
                Lifecycle::Running
            } else if elapsed >= grace {
                Lifecycle::RunningNoPort
            } else {
                Lifecycle::Starting
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    const GRACE: Duration = Duration::from_secs(10);

    #[test]
    fn alive_before_grace_without_port_is_starting() {
        let l = classify(Duration::from_secs(2), GRACE, ExitState::Alive, false);
        assert_eq!(l, Lifecycle::Starting);
    }

    #[test]
    fn alive_with_port_is_running() {
        let l = classify(Duration::from_secs(1), GRACE, ExitState::Alive, true);
        assert_eq!(l, Lifecycle::Running);
    }

    #[test]
    fn alive_after_grace_without_port_is_running_no_port() {
        let l = classify(Duration::from_secs(11), GRACE, ExitState::Alive, false);
        assert_eq!(l, Lifecycle::RunningNoPort);
    }

    #[test]
    fn clean_exit_is_exited() {
        let l = classify(Duration::from_secs(1), GRACE, ExitState::Exited(0), false);
        assert_eq!(l, Lifecycle::Exited);
    }

    #[test]
    fn nonzero_exit_is_crashed() {
        let l = classify(Duration::from_secs(1), GRACE, ExitState::Exited(7), false);
        assert_eq!(l, Lifecycle::Crashed);
    }

    #[test]
    fn signal_termination_is_exited_not_crashed() {
        let l = classify(Duration::from_secs(1), GRACE, ExitState::Signaled, false);
        assert_eq!(l, Lifecycle::Exited);
    }
}
