use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use specta::Type;
use tauri::State;

use crate::process::{ProcessProbe, SystemProcessProbe};

use super::parse::tasks_from_folder;
use super::registry::ProjectRegistry;
use super::spawn::spawn_task;
use super::store::{upsert, ProjectStore, ProjectStoreData};
use super::{Project, Task};

pub struct ProjectsState {
    pub store: ProjectStore,
    pub data: Mutex<ProjectStoreData>,
}

impl ProjectsState {
    pub fn load(store: ProjectStore) -> (Self, Option<String>) {
        let outcome = store.load();
        (
            Self {
                store,
                data: Mutex::new(outcome.data),
            },
            outcome.notice,
        )
    }
}

fn login_shell() -> String {
    std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string())
}

#[derive(Clone, Debug, PartialEq, Serialize, Type)]
pub struct SaveAsCandidate {
    pub pid: u32,
    pub command: String,
    pub is_shell: bool,
}

const SHELLS: &[&str] = &["sh", "zsh", "bash", "-zsh", "-bash", "fish", "-sh", "login"];

fn is_shell_command(cmd: &[String]) -> bool {
    cmd.first()
        .map(|first| {
            let base = first.rsplit('/').next().unwrap_or(first);
            SHELLS.contains(&base)
        })
        .unwrap_or(true)
}

pub fn candidates_from_chain(
    listener_pid: u32,
    procs: &[(u32, Option<u32>, Vec<String>)],
) -> Vec<SaveAsCandidate> {
    let by_pid: std::collections::HashMap<u32, &(u32, Option<u32>, Vec<String>)> =
        procs.iter().map(|p| (p.0, p)).collect();

    let mut out = Vec::new();
    let mut current = Some(listener_pid);
    while let Some(pid) = current {
        let Some(entry) = by_pid.get(&pid) else {
            break;
        };
        if out.len() > 1 && is_shell_command(&entry.2) {
            break;
        }
        out.push(SaveAsCandidate {
            pid: entry.0,
            command: entry.2.join(" "),
            is_shell: is_shell_command(&entry.2),
        });
        current = entry.1;
    }
    out
}

#[cfg(test)]
fn default_pick(candidates: &[SaveAsCandidate]) -> SaveAsCandidate {
    candidates
        .iter()
        .rev()
        .find(|c| !c.is_shell)
        .or_else(|| candidates.first())
        .cloned()
        .unwrap_or(SaveAsCandidate {
            pid: 0,
            command: String::new(),
            is_shell: false,
        })
}

#[tauri::command]
#[specta::specta]
pub fn load_projects(state: State<'_, ProjectsState>) -> Vec<Project> {
    state
        .data
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .projects
        .clone()
}

#[tauri::command]
#[specta::specta]
pub fn suggest_tasks(folder: String) -> Vec<Task> {
    tasks_from_folder(Path::new(&folder))
}

#[tauri::command]
#[specta::specta]
pub fn save_project(
    state: State<'_, ProjectsState>,
    project: Project,
) -> Result<Vec<Project>, String> {
    let mut data = state.data.lock().unwrap_or_else(|e| e.into_inner());
    upsert(&mut data, project);
    state.store.save(&data).map_err(|e| e.to_string())?;
    Ok(data.projects.clone())
}

#[tauri::command]
#[specta::specta]
pub fn remove_project(state: State<'_, ProjectsState>, id: String) -> Result<Vec<Project>, String> {
    let mut data = state.data.lock().unwrap_or_else(|e| e.into_inner());
    data.projects.retain(|p| p.id != id);
    state.store.save(&data).map_err(|e| e.to_string())?;
    Ok(data.projects.clone())
}

#[tauri::command]
#[specta::specta]
pub fn start_task(
    state: State<'_, ProjectsState>,
    registry: State<'_, Arc<Mutex<ProjectRegistry>>>,
    project_id: String,
    task_id: String,
) -> Result<(), String> {
    let (folder, command) = {
        let data = state.data.lock().unwrap_or_else(|e| e.into_inner());
        let project = data
            .projects
            .iter()
            .find(|p| p.id == project_id)
            .ok_or("project not found")?;
        let task = project
            .tasks
            .iter()
            .find(|t| t.id == task_id)
            .ok_or("task not found")?;
        (project.folder.clone(), task.command.clone())
    };

    let process =
        spawn_task(&login_shell(), &command, &PathBuf::from(folder)).map_err(|e| e.to_string())?;
    registry
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(project_id, task_id, process);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn stop_task(pgid: u32) -> Result<(), String> {
    #[cfg(unix)]
    super::kill::kill_group(pgid, libc::SIGTERM);
    #[cfg(not(unix))]
    super::kill::kill_group(pgid, 15);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn save_as_candidates(listener_pid: u32) -> Result<Vec<SaveAsCandidate>, String> {
    save_as_candidates_from_probe(listener_pid, SystemProcessProbe::default())
}

fn save_as_candidates_from_probe(
    listener_pid: u32,
    probe: impl ProcessProbe,
) -> Result<Vec<SaveAsCandidate>, String> {
    let snapshots = probe.all_snapshots().map_err(|e| e.to_string())?;
    let pids: Vec<u32> = snapshots.iter().map(|s| s.pid).collect();
    let infos = probe.info_for_pids(&pids).map_err(|e| e.to_string())?;
    let command_by_pid: std::collections::HashMap<u32, Vec<String>> =
        infos.into_iter().map(|i| (i.pid, i.command)).collect();

    let procs: Vec<(u32, Option<u32>, Vec<String>)> = snapshots
        .into_iter()
        .map(|s| {
            (
                s.pid,
                s.parent_pid,
                command_by_pid.get(&s.pid).cloned().unwrap_or_default(),
            )
        })
        .collect();

    Ok(candidates_from_chain(listener_pid, &procs))
}

pub fn candidates_from_chain_for_test(pid: u32) -> Vec<SaveAsCandidate> {
    save_as_candidates(pid).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node_under_pnpm() -> Vec<(u32, Option<u32>, Vec<String>)> {
        vec![
            (100, Some(1), vec!["-zsh".to_string()]),
            (200, Some(100), vec!["pnpm".to_string(), "dev".to_string()]),
            (
                300,
                Some(200),
                vec!["node".to_string(), "server.js".to_string()],
            ),
        ]
    }

    #[test]
    fn walk_prefers_the_non_shell_ancestor_over_the_listener() {
        let candidates = candidates_from_chain(300, &node_under_pnpm());

        assert_eq!(candidates[0].command, "node server.js");
        assert!(candidates.iter().any(|c| c.command == "pnpm dev"));
        assert_eq!(default_pick(&candidates).command, "pnpm dev");
    }
}
