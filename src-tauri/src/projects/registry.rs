use std::time::{Duration, Instant};

use serde::Serialize;
use specta::Type;

use super::lifecycle::{classify, ExitState, Lifecycle};
use super::spawn::SpawnedProcess;

const TERMINAL_STATUS_RETENTION: Duration = Duration::from_secs(60);

#[derive(Clone, Debug, PartialEq, Serialize, Type)]
pub struct ManagedStatus {
    pub project_id: String,
    pub task_id: String,
    pub pid: u32,
    pub lifecycle: Lifecycle,
    pub recent_output: Vec<String>,
}

struct ManagedProcess {
    project_id: String,
    task_id: String,
    pgid: u32,
    started_at: Instant,
    process: Option<SpawnedProcess>,
    terminal_since: Option<Instant>,
    terminal_lifecycle: Option<Lifecycle>,
    terminal_output: Vec<String>,
}

pub struct ProjectRegistry {
    grace: Duration,
    managed: Vec<ManagedProcess>,
}

impl ProjectRegistry {
    pub fn new(grace: Duration) -> Self {
        Self {
            grace,
            managed: Vec::new(),
        }
    }

    pub fn insert(&mut self, project_id: String, task_id: String, process: SpawnedProcess) {
        self.purge_terminal(&project_id, &task_id);
        self.managed.push(ManagedProcess {
            project_id,
            task_id,
            pgid: process.pgid(),
            started_at: Instant::now(),
            process: Some(process),
            terminal_since: None,
            terminal_lifecycle: None,
            terminal_output: Vec::new(),
        });
    }

    /// Drop any retained terminal entry for this task so a restart isn't
    /// shadowed by the previous run's frozen status (the frontend's first-match
    /// lookup would otherwise show the stale crashed/exited row for up to the
    /// retention window).
    fn purge_terminal(&mut self, project_id: &str, task_id: &str) {
        self.managed.retain(|m| {
            !(m.project_id == project_id
                && m.task_id == task_id
                && m.terminal_lifecycle.is_some())
        });
    }

    pub fn has_active_task(&self, project_id: &str, task_id: &str) -> bool {
        self.managed.iter().any(|m| {
            m.project_id == project_id
                && m.task_id == task_id
                && !matches!(
                    m.terminal_lifecycle,
                    Some(Lifecycle::Exited | Lifecycle::Crashed)
                )
        })
    }

    pub fn pgids(&self) -> Vec<u32> {
        self.managed
            .iter()
            .filter(|m| m.terminal_lifecycle.is_none())
            .map(|m| m.pgid)
            .collect()
    }

    pub fn drain(&mut self) -> Vec<(u32, Option<SpawnedProcess>)> {
        self.managed
            .drain(..)
            .map(|m| (m.pgid, m.process))
            .collect()
    }

    pub fn reconcile(
        &mut self,
        mut exit_of: impl FnMut(u32) -> ExitState,
        mut pgid_of: impl FnMut(u32) -> Option<u32>,
        listeners: &[u32],
    ) -> Vec<ManagedStatus> {
        let grace = self.grace;
        let mut statuses = Vec::with_capacity(self.managed.len());

        for managed in &mut self.managed {
            let exit = exit_of(managed.pgid);
            let holds_port = listeners
                .iter()
                .any(|&pid| pgid_of(pid) == Some(managed.pgid));
            let lifecycle = reconcile_lifecycle(managed, grace, exit, holds_port);
            statuses.push(status_for(managed, lifecycle));
        }

        self.managed.retain(|m| {
            m.terminal_since
                .map(|since| since.elapsed() < TERMINAL_STATUS_RETENTION)
                .unwrap_or(true)
        });
        statuses
    }

    pub fn reconcile_owned(
        &mut self,
        mut fallback_exit_of: impl FnMut(u32) -> ExitState,
        mut pgid_of: impl FnMut(u32) -> Option<u32>,
        listeners: &[u32],
    ) -> Vec<ManagedStatus> {
        let grace = self.grace;
        let mut statuses = Vec::with_capacity(self.managed.len());

        for managed in &mut self.managed {
            let exit = match managed.process.as_mut() {
                Some(process) => match process.try_status() {
                    Ok(Some(status)) => status
                        .code()
                        .map(ExitState::Exited)
                        .unwrap_or(ExitState::Signaled),
                    Ok(None) => ExitState::Alive,
                    Err(_) => fallback_exit_of(managed.pgid),
                },
                None => fallback_exit_of(managed.pgid),
            };
            let holds_port = listeners
                .iter()
                .any(|&pid| pgid_of(pid) == Some(managed.pgid));
            let lifecycle = reconcile_lifecycle(managed, grace, exit, holds_port);
            statuses.push(status_for(managed, lifecycle));
        }

        self.managed.retain(|m| {
            m.terminal_since
                .map(|since| since.elapsed() < TERMINAL_STATUS_RETENTION)
                .unwrap_or(true)
        });
        statuses
    }

    #[cfg(test)]
    fn insert_for_test(&mut self, project_id: &str, task_id: &str, pgid: u32, age: Duration) {
        // Mirror the real `insert`: a fresh start purges any retained terminal
        // entry for the same task.
        self.purge_terminal(project_id, task_id);
        self.managed.push(ManagedProcess {
            project_id: project_id.to_string(),
            task_id: task_id.to_string(),
            pgid,
            started_at: Instant::now() - age,
            process: None,
            terminal_since: None,
            terminal_lifecycle: None,
            terminal_output: Vec::new(),
        });
    }
}

fn reconcile_lifecycle(
    managed: &mut ManagedProcess,
    grace: Duration,
    exit: ExitState,
    holds_port: bool,
) -> Lifecycle {
    if let Some(lifecycle) = managed.terminal_lifecycle {
        return lifecycle;
    }

    let lifecycle = classify(managed.started_at.elapsed(), grace, exit, holds_port);
    if matches!(lifecycle, Lifecycle::Exited | Lifecycle::Crashed) {
        managed.terminal_since = Some(Instant::now());
        managed.terminal_lifecycle = Some(lifecycle);
        managed.terminal_output = managed
            .process
            .as_ref()
            .map(SpawnedProcess::recent_output)
            .unwrap_or_default();
        managed.process = None;
    }
    lifecycle
}

fn status_for(managed: &ManagedProcess, lifecycle: Lifecycle) -> ManagedStatus {
    ManagedStatus {
        project_id: managed.project_id.clone(),
        task_id: managed.task_id.clone(),
        pid: managed.pgid,
        lifecycle,
        recent_output: managed
            .process
            .as_ref()
            .map(SpawnedProcess::recent_output)
            .unwrap_or_else(|| managed.terminal_output.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn reconcile_marks_running_when_a_listener_pid_is_in_the_group() {
        let mut registry = ProjectRegistry::new(Duration::from_secs(10));
        registry.insert_for_test("p1", "dev", 4242, Duration::from_secs(1));

        let statuses = registry.reconcile(
            |_pid| ExitState::Alive,
            |listener_pid| {
                if listener_pid == 5000 {
                    Some(4242)
                } else {
                    None
                }
            },
            &[5000],
        );

        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].lifecycle, Lifecycle::Running);
        assert_eq!(statuses[0].pid, 4242);
    }

    #[test]
    fn reconcile_drops_groups_that_have_exited_cleanly() {
        let mut registry = ProjectRegistry::new(Duration::from_secs(10));
        registry.insert_for_test("p1", "dev", 4242, Duration::from_secs(1));

        let statuses = registry.reconcile(|_pid| ExitState::Exited(0), |_pid| None, &[]);

        assert_eq!(statuses[0].lifecycle, Lifecycle::Exited);
        assert!(registry.pgids().is_empty());
    }

    #[test]
    fn reconcile_keeps_finished_groups_visible_for_later_polls() {
        let mut registry = ProjectRegistry::new(Duration::from_secs(10));
        registry.insert_for_test("p1", "dev", 4242, Duration::from_secs(1));

        let first = registry.reconcile(|_pid| ExitState::Exited(1), |_pid| None, &[]);
        let second = registry.reconcile(|_pid| ExitState::Alive, |_pid| None, &[]);

        assert_eq!(first[0].lifecycle, Lifecycle::Crashed);
        assert_eq!(second[0].lifecycle, Lifecycle::Crashed);
    }

    #[test]
    fn active_task_check_ignores_terminal_statuses() {
        let mut registry = ProjectRegistry::new(Duration::from_secs(10));
        registry.insert_for_test("p1", "dev", 4242, Duration::from_secs(1));

        assert!(registry.has_active_task("p1", "dev"));
        registry.reconcile(|_pid| ExitState::Exited(0), |_pid| None, &[]);
        assert!(!registry.has_active_task("p1", "dev"));
    }

    #[test]
    fn restart_purges_the_retained_terminal_entry() {
        let mut registry = ProjectRegistry::new(Duration::from_secs(10));
        registry.insert_for_test("p1", "dev", 4242, Duration::from_secs(1));
        // First run crashes and is retained as a terminal status.
        registry.reconcile(|_pid| ExitState::Exited(1), |_pid| None, &[]);

        // Restart: a fresh start for the same task replaces the terminal row
        // rather than leaving a stale duplicate that shadows the live one.
        registry.insert_for_test("p1", "dev", 5555, Duration::from_secs(0));
        let statuses = registry.reconcile(|_pid| ExitState::Alive, |_pid| None, &[]);

        let dev: Vec<_> = statuses
            .iter()
            .filter(|s| s.project_id == "p1" && s.task_id == "dev")
            .collect();
        assert_eq!(dev.len(), 1, "restart must not leave a duplicate row");
        assert_eq!(dev[0].pid, 5555);
        assert_eq!(dev[0].lifecycle, Lifecycle::Starting);
    }
}
