use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use tauri::State;

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
