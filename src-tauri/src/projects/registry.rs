use std::time::{Duration, Instant};

use serde::Serialize;
use specta::Type;

use super::lifecycle::{classify, ExitState, Lifecycle};
use super::spawn::SpawnedProcess;

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
        self.managed.push(ManagedProcess {
            project_id,
            task_id,
            pgid: process.pgid(),
            started_at: Instant::now(),
            process: Some(process),
        });
    }

    pub fn pgids(&self) -> Vec<u32> {
        self.managed.iter().map(|m| m.pgid).collect()
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
        let mut finished_pgids = Vec::new();

        for managed in &mut self.managed {
            let exit = exit_of(managed.pgid);
            let holds_port = listeners
                .iter()
                .any(|&pid| pgid_of(pid) == Some(managed.pgid));
            let lifecycle = classify(managed.started_at.elapsed(), grace, exit, holds_port);
            if matches!(lifecycle, Lifecycle::Exited | Lifecycle::Crashed) {
                finished_pgids.push(managed.pgid);
            }
            statuses.push(status_for(managed, lifecycle));
        }

        self.managed.retain(|m| !finished_pgids.contains(&m.pgid));
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
        let mut finished_pgids = Vec::new();

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
            let lifecycle = classify(managed.started_at.elapsed(), grace, exit, holds_port);
            if matches!(lifecycle, Lifecycle::Exited | Lifecycle::Crashed) {
                finished_pgids.push(managed.pgid);
            }
            statuses.push(status_for(managed, lifecycle));
        }

        self.managed.retain(|m| !finished_pgids.contains(&m.pgid));
        statuses
    }

    #[cfg(test)]
    fn insert_for_test(&mut self, project_id: &str, task_id: &str, pgid: u32, age: Duration) {
        self.managed.push(ManagedProcess {
            project_id: project_id.to_string(),
            task_id: task_id.to_string(),
            pgid,
            started_at: Instant::now() - age,
            process: None,
        });
    }
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
            .unwrap_or_default(),
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
}
