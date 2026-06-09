use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use specta::Type;
use tauri::ipc::Channel;
use tauri::State;

use crate::logs::ansi::LogBatch;
use crate::process::{ProcessProbe, SystemProcessProbe};

use super::parse::tasks_from_folder;
use super::registry::{LaunchSpec, ProjectRegistry};
use super::spawn::spawn_task;
use super::store::{upsert, ProjectStore, ProjectStoreData};
use super::{InputStatus, Project, Task};

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
    std::env::var("SHELL")
        .ok()
        .filter(|shell| !shell.trim().is_empty())
        .or_else(login_shell_from_passwd)
        .unwrap_or_else(|| "/bin/zsh".to_string())
}

#[derive(Debug, PartialEq)]
struct ValidQuickRunRequest {
    command: String,
    cwd: PathBuf,
}

fn validate_quick_run_request(
    command: &str,
    cwd: &str,
    home_dir: impl FnOnce() -> Option<PathBuf>,
    is_readable_dir: impl FnOnce(&Path) -> bool,
) -> Result<ValidQuickRunRequest, String> {
    let command = command.trim().to_string();
    if command.is_empty() {
        return Err("quick-run command cannot be blank".to_string());
    }

    let cwd = expand_tilde(cwd, home_dir)?;
    if !cwd.exists() {
        return Err(format!("folder does not exist: {}", cwd.display()));
    }
    if !cwd.is_dir() {
        return Err(format!("folder is not a directory: {}", cwd.display()));
    }
    if !is_readable_dir(&cwd) {
        return Err(format!("folder is not readable: {}", cwd.display()));
    }

    Ok(ValidQuickRunRequest { command, cwd })
}

fn expand_tilde(cwd: &str, home_dir: impl FnOnce() -> Option<PathBuf>) -> Result<PathBuf, String> {
    if cwd == "~" {
        return home_dir().ok_or("could not resolve home directory".to_string());
    }
    if let Some(rest) = cwd.strip_prefix("~/") {
        let home = home_dir().ok_or("could not resolve home directory".to_string())?;
        return Ok(home.join(rest));
    }
    Ok(PathBuf::from(cwd))
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
}

fn is_readable_dir(path: &Path) -> bool {
    std::fs::read_dir(path).is_ok()
}

#[cfg(unix)]
fn login_shell_from_passwd() -> Option<String> {
    unsafe {
        let passwd = libc::getpwuid(libc::getuid());
        if passwd.is_null() {
            return None;
        }
        std::ffi::CStr::from_ptr((*passwd).pw_shell)
            .to_str()
            .ok()
            .filter(|shell| !shell.trim().is_empty())
            .map(ToOwned::to_owned)
    }
}

#[cfg(not(unix))]
fn login_shell_from_passwd() -> Option<String> {
    None
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
    let mut visited = std::collections::HashSet::new();
    while let Some(pid) = current {
        if !visited.insert(pid) || visited.len() > 32 {
            break;
        }
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

    let mut registry = registry.lock().unwrap_or_else(|e| e.into_inner());
    if registry
        .active_project_run_id(&project_id, &task_id)
        .is_some()
    {
        return Err("task is already running".to_string());
    }
    let process =
        spawn_task(&login_shell(), &command, &PathBuf::from(&folder)).map_err(|e| e.to_string())?;
    registry.insert(
        project_id,
        task_id,
        LaunchSpec {
            command,
            cwd: folder,
        },
        process,
    );
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn start_quick_run(
    registry: State<'_, Arc<Mutex<ProjectRegistry>>>,
    command: String,
    cwd: String,
) -> Result<String, String> {
    let request = validate_quick_run_request(&command, &cwd, home_dir, is_readable_dir)?;

    let mut registry = registry.lock().unwrap_or_else(|e| e.into_inner());
    let process =
        spawn_task(&login_shell(), &request.command, &request.cwd).map_err(|e| e.to_string())?;
    let run_id = registry.insert_quick_run(
        LaunchSpec {
            command: request.command,
            cwd: request.cwd.to_string_lossy().to_string(),
        },
        process,
    );
    Ok(run_id)
}

#[tauri::command]
#[specta::specta]
pub fn stop_task(
    registry: State<'_, Arc<Mutex<ProjectRegistry>>>,
    run_id: String,
) -> Result<(), String> {
    let pgid = registry
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .pgid_for_run(&run_id)
        .ok_or("task is not running")?;
    std::thread::spawn(move || {
        #[cfg(unix)]
        {
            super::kill::kill_group(pgid, libc::SIGTERM);
            std::thread::sleep(Duration::from_millis(500));
            super::kill::kill_group(pgid, libc::SIGKILL);
        }
        #[cfg(not(unix))]
        {
            super::kill::kill_group(pgid, 15);
        }
    });
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn subscribe_logs(
    registry: State<'_, Arc<Mutex<ProjectRegistry>>>,
    run_id: String,
    channel: Channel<LogBatch>,
) -> Result<(), String> {
    let mut registry = registry.lock().unwrap_or_else(|e| e.into_inner());
    if registry.subscribe_logs(&run_id, channel) {
        Ok(())
    } else {
        Err("task is not running".to_string())
    }
}

#[tauri::command]
#[specta::specta]
pub fn unsubscribe_logs(
    registry: State<'_, Arc<Mutex<ProjectRegistry>>>,
    run_id: String,
) -> Result<(), String> {
    let mut registry = registry.lock().unwrap_or_else(|e| e.into_inner());
    registry.unsubscribe_logs(&run_id);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn send_input(
    registry: State<'_, Arc<Mutex<ProjectRegistry>>>,
    run_id: String,
    data: String,
) -> Result<InputStatus, String> {
    let mut registry = registry.lock().unwrap_or_else(|e| e.into_inner());
    registry
        .send_input(&run_id, data.as_bytes())
        .map_err(|e| e.to_string())
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

    #[test]
    fn walk_stops_on_parent_cycles() {
        let procs = vec![
            (
                100,
                Some(200),
                vec!["node".to_string(), "server.js".to_string()],
            ),
            (200, Some(100), vec!["pnpm".to_string(), "dev".to_string()]),
        ];

        let candidates = candidates_from_chain(100, &procs);

        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].pid, 100);
        assert_eq!(candidates[1].pid, 200);
    }

    #[test]
    fn quick_run_folder_expands_tilde_and_validates_directory() {
        let home = tempfile::tempdir().expect("home dir");
        let project = home.path().join("repo");
        std::fs::create_dir(&project).expect("create project dir");

        let resolved = validate_quick_run_request(
            "pnpm dev",
            "~/repo",
            || Some(home.path().to_path_buf()),
            |_| true,
        )
        .expect("valid request");

        assert_eq!(resolved.command, "pnpm dev");
        assert_eq!(resolved.cwd, project);
    }

    #[test]
    fn quick_run_folder_rejects_missing_paths() {
        let home = tempfile::tempdir().expect("home dir");

        let error = validate_quick_run_request(
            "pnpm dev",
            "~/missing",
            || Some(home.path().to_path_buf()),
            |_| true,
        )
        .expect_err("missing path should be rejected");

        assert!(error.contains("does not exist"));
    }

    #[test]
    fn quick_run_folder_rejects_non_directories() {
        let home = tempfile::tempdir().expect("home dir");
        let file = home.path().join("package.json");
        std::fs::write(&file, "{}").expect("write file");

        let error = validate_quick_run_request(
            "pnpm dev",
            "~/package.json",
            || Some(home.path().to_path_buf()),
            |_| true,
        )
        .expect_err("file path should be rejected");

        assert!(error.contains("not a directory"));
    }

    #[test]
    fn quick_run_folder_rejects_unreadable_directories() {
        let home = tempfile::tempdir().expect("home dir");
        let project = home.path().join("repo");
        std::fs::create_dir(&project).expect("create project dir");

        let error = validate_quick_run_request(
            "pnpm dev",
            "~/repo",
            || Some(home.path().to_path_buf()),
            |_| false,
        )
        .expect_err("unreadable dir should be rejected");

        assert!(error.contains("not readable"));
    }

    #[test]
    fn quick_run_rejects_blank_commands() {
        let home = tempfile::tempdir().expect("home dir");

        let error = validate_quick_run_request(
            "   \n\t",
            "~",
            || Some(home.path().to_path_buf()),
            |_| true,
        )
        .expect_err("blank command should be rejected");

        assert!(error.contains("command cannot be blank"));
    }
}
