use std::time::{Duration, Instant};

use serde::Serialize;
use specta::Type;
use tauri::ipc::Channel;
use uuid::Uuid;

use super::lifecycle::{classify, line_looks_like_prompt, ExitState, Lifecycle};
use super::spawn::{InputStatus, SpawnedProcess};
use crate::logs::ansi::{sanitize_chunk, LogBatch, LogLine};

const TERMINAL_STATUS_RETENTION: Duration = Duration::from_secs(60);

#[derive(Clone, Debug, PartialEq, Serialize, Type)]
pub struct ManagedStatus {
    pub run_id: String,
    pub origin: ManagedOrigin,
    pub launch_spec: LaunchSpec,
    pub pid: u32,
    pub lifecycle: Lifecycle,
    pub recent_output: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Type)]
pub struct LaunchSpec {
    pub command: String,
    pub cwd: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ManagedOrigin {
    Project { project_id: String, task_id: String },
    QuickRun,
}

struct ManagedProcess {
    run_id: String,
    origin: ManagedOrigin,
    launch_spec: LaunchSpec,
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

    pub fn insert(
        &mut self,
        project_id: String,
        task_id: String,
        launch_spec: LaunchSpec,
        process: SpawnedProcess,
    ) -> String {
        self.purge_terminal(&project_id, &task_id);
        let run_id = new_run_id();
        self.managed.push(ManagedProcess {
            run_id: run_id.clone(),
            origin: ManagedOrigin::Project {
                project_id,
                task_id,
            },
            launch_spec,
            pgid: process.pgid(),
            started_at: Instant::now(),
            process: Some(process),
            terminal_since: None,
            terminal_lifecycle: None,
            terminal_output: Vec::new(),
        });
        run_id
    }

    pub fn insert_quick_run(&mut self, launch_spec: LaunchSpec, process: SpawnedProcess) -> String {
        let run_id = new_run_id();
        self.managed.push(ManagedProcess {
            run_id: run_id.clone(),
            origin: ManagedOrigin::QuickRun,
            launch_spec,
            pgid: process.pgid(),
            started_at: Instant::now(),
            process: Some(process),
            terminal_since: None,
            terminal_lifecycle: None,
            terminal_output: Vec::new(),
        });
        run_id
    }

    /// Drop any retained terminal entry for this task so a restart isn't
    /// shadowed by the previous run's frozen status (the frontend's first-match
    /// lookup would otherwise show the stale crashed/exited row for up to the
    /// retention window).
    fn purge_terminal(&mut self, project_id: &str, task_id: &str) {
        self.managed.retain(|m| {
            !(matches!(
                &m.origin,
                ManagedOrigin::Project {
                    project_id: managed_project_id,
                    task_id: managed_task_id,
                } if managed_project_id == project_id && managed_task_id == task_id
            ) && m.terminal_lifecycle.is_some())
        });
    }

    pub fn has_active_run(&self, run_id: &str) -> bool {
        self.active_run(run_id).is_some()
    }

    pub fn active_project_run_id(&self, project_id: &str, task_id: &str) -> Option<String> {
        self.managed
            .iter()
            .find(|m| {
                matches!(
                    &m.origin,
                    ManagedOrigin::Project {
                        project_id: managed_project_id,
                        task_id: managed_task_id,
                    } if managed_project_id == project_id && managed_task_id == task_id
                ) && !is_terminal(m)
            })
            .map(|m| m.run_id.clone())
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

    pub fn subscribe_logs(&mut self, run_id: &str, channel: Channel<LogBatch>) -> bool {
        let Some(managed) = self.managed.iter_mut().find(|m| m.run_id == run_id) else {
            return false;
        };
        if let Some(process) = managed.process.as_ref() {
            process.subscribe_logs(channel);
            return true;
        }
        if is_terminal(managed) {
            let batch = LogBatch {
                lines: managed
                    .terminal_output
                    .iter()
                    .map(|line| LogLine {
                        html: sanitize_chunk(line.as_bytes()),
                    })
                    .collect(),
            };
            let _ = channel.send(batch);
            return true;
        }
        false
    }

    pub fn unsubscribe_logs(&mut self, run_id: &str) -> bool {
        let Some(managed) = self.active_run_mut(run_id) else {
            return false;
        };
        let Some(process) = managed.process.as_ref() else {
            return false;
        };
        process.unsubscribe_logs();
        true
    }

    pub fn send_input(&mut self, run_id: &str, data: &[u8]) -> std::io::Result<InputStatus> {
        let Some(managed) = self.active_run_mut(run_id) else {
            return Ok(InputStatus::Ignored);
        };
        let Some(process) = managed.process.as_mut() else {
            return Ok(InputStatus::Ignored);
        };
        process.send_input_if_running(data)
    }

    pub fn pgid_for_run(&self, run_id: &str) -> Option<u32> {
        self.active_run(run_id).map(|managed| managed.pgid)
    }

    pub fn launch_spec_for_run(&self, run_id: &str) -> Option<LaunchSpec> {
        self.managed
            .iter()
            .find(|managed| managed.run_id == run_id)
            .map(|managed| managed.launch_spec.clone())
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
    fn insert_for_test(
        &mut self,
        project_id: &str,
        task_id: &str,
        pgid: u32,
        age: Duration,
    ) -> String {
        // Mirror the real `insert`: a fresh start purges any retained terminal
        // entry for the same task.
        self.purge_terminal(project_id, task_id);
        let run_id = new_run_id();
        self.managed.push(ManagedProcess {
            run_id: run_id.clone(),
            origin: ManagedOrigin::Project {
                project_id: project_id.to_string(),
                task_id: task_id.to_string(),
            },
            launch_spec: LaunchSpec {
                command: "pnpm dev".to_string(),
                cwd: "/tmp/portus-test".to_string(),
            },
            pgid,
            started_at: Instant::now() - age,
            process: None,
            terminal_since: None,
            terminal_lifecycle: None,
            terminal_output: Vec::new(),
        });
        run_id
    }

    #[cfg(test)]
    pub(crate) fn insert_quick_run_for_test(
        &mut self,
        launch_spec: LaunchSpec,
        pgid: u32,
    ) -> String {
        let run_id = new_run_id();
        self.managed.push(ManagedProcess {
            run_id: run_id.clone(),
            origin: ManagedOrigin::QuickRun,
            launch_spec,
            pgid,
            started_at: Instant::now(),
            process: None,
            terminal_since: None,
            terminal_lifecycle: None,
            terminal_output: Vec::new(),
        });
        run_id
    }

    #[cfg(test)]
    fn set_terminal_output_for_test(&mut self, run_id: &str, lines: Vec<String>) {
        if let Some(managed) = self.managed.iter_mut().find(|m| m.run_id == run_id) {
            managed.terminal_output = lines;
        }
    }

    fn active_run(&self, run_id: &str) -> Option<&ManagedProcess> {
        self.managed
            .iter()
            .find(|managed| managed.run_id == run_id && !is_terminal(managed))
    }

    fn active_run_mut(&mut self, run_id: &str) -> Option<&mut ManagedProcess> {
        self.managed
            .iter_mut()
            .find(|managed| managed.run_id == run_id && !is_terminal(managed))
    }
}

fn new_run_id() -> String {
    Uuid::new_v4().to_string()
}

fn is_terminal(managed: &ManagedProcess) -> bool {
    matches!(
        managed.terminal_lifecycle,
        Some(Lifecycle::Exited | Lifecycle::Crashed)
    )
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

    let (idle, last_line_is_prompt) = managed
        .process
        .as_ref()
        .map(|process| {
            (
                process.output_idle(),
                process
                    .last_non_empty_output()
                    .is_some_and(|line| line_looks_like_prompt(&line)),
            )
        })
        .unwrap_or((Duration::ZERO, false));
    let lifecycle = classify(
        managed.started_at.elapsed(),
        grace,
        exit,
        holds_port,
        idle,
        last_line_is_prompt,
    );
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
        run_id: managed.run_id.clone(),
        origin: managed.origin.clone(),
        launch_spec: managed.launch_spec.clone(),
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
    use std::sync::mpsc;
    use std::time::Duration;
    use tauri::ipc::{Channel, InvokeResponseBody};

    #[test]
    fn subscribe_logs_replays_sanitized_terminal_output() {
        let mut registry = ProjectRegistry::new(Duration::from_secs(10));
        let run_id = registry.insert_for_test("p1", "dev", 4242, Duration::from_secs(1));
        registry.reconcile(|_pid| ExitState::Exited(0), |_pid| None, &[]);
        registry.set_terminal_output_for_test(
            &run_id,
            vec![
                "plain line".to_string(),
                "\u{1b}[32m\u{1b}[1m✓\u{1b}[22m\u{1b}[39m Ready in 179ms".to_string(),
            ],
        );

        let (tx, rx) = mpsc::channel();
        assert!(registry.subscribe_logs(
            &run_id,
            Channel::new(move |body| {
                tx.send(body).unwrap();
                Ok(())
            }),
        ));

        let payload = rx.recv_timeout(Duration::from_secs(1)).unwrap();
        let InvokeResponseBody::Json(json) = payload else {
            panic!("expected JSON log batch");
        };
        let batch: LogBatch = serde_json::from_str(&json).unwrap();
        assert_eq!(batch.lines.len(), 2);
        assert!(batch.lines[0].html.contains("plain line"));
        assert!(batch.lines[1].html.contains("ansi-fg-green"));
        assert!(batch.lines[1].html.contains("Ready in 179ms"));
        assert!(!batch.lines.iter().any(|line| line.html.contains("\u{1b}[")));
    }

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
        let run_id = registry.insert_for_test("p1", "dev", 4242, Duration::from_secs(1));

        assert!(registry.has_active_run(&run_id));
        registry.reconcile(|_pid| ExitState::Exited(0), |_pid| None, &[]);
        assert!(!registry.has_active_run(&run_id));
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
            .filter(|s| {
                matches!(
                    &s.origin,
                    ManagedOrigin::Project {
                        project_id,
                        task_id,
                    } if project_id == "p1" && task_id == "dev"
                )
            })
            .collect();
        assert_eq!(dev.len(), 1, "restart must not leave a duplicate row");
        assert_eq!(dev[0].pid, 5555);
        assert_eq!(dev[0].lifecycle, Lifecycle::Starting);
    }

    #[test]
    fn duplicate_task_entries_have_distinct_run_ids_and_lookup_routes_by_run_id() {
        let mut registry = ProjectRegistry::new(Duration::from_secs(10));
        let first_run_id = registry.insert_for_test("p1", "dev", 4242, Duration::from_secs(1));
        let second_run_id = registry.insert_for_test("p1", "dev", 5555, Duration::from_secs(1));

        assert_ne!(first_run_id, second_run_id);

        let statuses = registry.reconcile(|_pid| ExitState::Alive, |_pid| None, &[]);
        let first = statuses
            .iter()
            .find(|status| status.run_id == first_run_id)
            .unwrap();
        let second = statuses
            .iter()
            .find(|status| status.run_id == second_run_id)
            .unwrap();

        assert_eq!(first.pid, 4242);
        assert_eq!(second.pid, 5555);
        assert_eq!(registry.pgid_for_run(&first_run_id), Some(4242));
        assert_eq!(registry.pgid_for_run(&second_run_id), Some(5555));
        assert_eq!(
            first.origin,
            ManagedOrigin::Project {
                project_id: "p1".to_string(),
                task_id: "dev".to_string()
            }
        );
        assert_eq!(
            second.origin,
            ManagedOrigin::Project {
                project_id: "p1".to_string(),
                task_id: "dev".to_string()
            }
        );
    }

    #[test]
    fn terminal_status_preserves_launch_command_and_cwd() {
        let mut registry = ProjectRegistry::new(Duration::from_secs(10));
        registry.insert_for_test("p1", "dev", 4242, Duration::from_secs(1));

        let statuses = registry.reconcile(|_pid| ExitState::Exited(0), |_pid| None, &[]);

        assert_eq!(statuses[0].lifecycle, Lifecycle::Exited);
        assert_eq!(statuses[0].launch_spec.command, "pnpm dev");
        assert_eq!(statuses[0].launch_spec.cwd, "/tmp/portus-test");
    }

    #[test]
    fn quick_run_insert_sets_origin_launch_spec_and_distinct_run_id() {
        let mut registry = ProjectRegistry::new(Duration::from_secs(10));
        let launch_spec = LaunchSpec {
            command: "pnpm dev".to_string(),
            cwd: "/Users/test/project".to_string(),
        };

        let run_id = registry.insert_quick_run_for_test(launch_spec.clone(), 4242);
        let statuses = registry.reconcile(|_pid| ExitState::Alive, |_pid| None, &[]);

        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].run_id, run_id);
        assert_eq!(statuses[0].launch_spec, launch_spec);
        assert_eq!(statuses[0].origin, ManagedOrigin::QuickRun);
    }

    #[test]
    fn duplicate_quick_runs_route_independently_by_run_id() {
        let mut registry = ProjectRegistry::new(Duration::from_secs(10));
        let first_run_id = registry.insert_quick_run_for_test(
            LaunchSpec {
                command: "pnpm dev".to_string(),
                cwd: "/tmp/portus-a".to_string(),
            },
            4242,
        );
        let second_run_id = registry.insert_quick_run_for_test(
            LaunchSpec {
                command: "pnpm dev".to_string(),
                cwd: "/tmp/portus-a".to_string(),
            },
            5555,
        );

        assert_ne!(first_run_id, second_run_id);
        assert_eq!(registry.pgid_for_run(&first_run_id), Some(4242));
        assert_eq!(registry.pgid_for_run(&second_run_id), Some(5555));

        let statuses = registry.reconcile(|_pid| ExitState::Alive, |_pid| None, &[]);
        let quick_runs: Vec<_> = statuses
            .iter()
            .filter(|status| matches!(status.origin, ManagedOrigin::QuickRun))
            .collect();
        assert_eq!(quick_runs.len(), 2);
    }

    #[test]
    fn launch_spec_for_run_finds_terminal_quick_runs() {
        let mut registry = ProjectRegistry::new(Duration::from_secs(10));
        let launch_spec = LaunchSpec {
            command: "pnpm dev".to_string(),
            cwd: "/tmp/portus-a".to_string(),
        };
        let run_id = registry.insert_quick_run_for_test(launch_spec.clone(), 4242);

        registry.reconcile(|_pid| ExitState::Exited(0), |_pid| None, &[]);

        assert_eq!(registry.launch_spec_for_run(&run_id), Some(launch_spec));
    }

    #[test]
    fn quick_run_pgids_are_included_for_kill_on_quit() {
        let mut registry = ProjectRegistry::new(Duration::from_secs(10));
        registry.insert_quick_run_for_test(
            LaunchSpec {
                command: "pnpm dev".to_string(),
                cwd: "/tmp/portus-a".to_string(),
            },
            4242,
        );

        assert_eq!(registry.pgids(), vec![4242]);
    }
}
