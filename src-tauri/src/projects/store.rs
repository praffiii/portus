use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;
use uuid::Uuid;

use super::{Project, Task};

pub const CURRENT_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
pub struct ProjectStoreData {
    pub version: u32,
    pub projects: Vec<Project>,
}

impl Default for ProjectStoreData {
    fn default() -> Self {
        Self {
            version: CURRENT_VERSION,
            projects: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub struct LoadOutcome {
    pub data: ProjectStoreData,
    pub notice: Option<String>,
}

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("failed to write projects file: {0}")]
    Write(String),
    #[error("failed to serialize projects: {0}")]
    Serialize(String),
}

pub struct ProjectStore {
    path: PathBuf,
}

impl ProjectStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn load(&self) -> LoadOutcome {
        let raw = match fs::read_to_string(&self.path) {
            Ok(raw) => raw,
            Err(_) => {
                return LoadOutcome {
                    data: ProjectStoreData::default(),
                    notice: None,
                };
            }
        };

        match serde_json::from_str::<ProjectStoreData>(&raw) {
            Ok(data) => LoadOutcome { data, notice: None },
            Err(_) => {
                let notice = self.back_up_corrupt();
                LoadOutcome {
                    data: ProjectStoreData::default(),
                    notice: Some(notice),
                }
            }
        }
    }

    pub fn save(&self, data: &ProjectStoreData) -> Result<(), StoreError> {
        let json = serde_json::to_string_pretty(data)
            .map_err(|error| StoreError::Serialize(error.to_string()))?;

        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|error| StoreError::Write(error.to_string()))?;
        }

        let tmp = self.path.with_extension("json.tmp");
        fs::write(&tmp, json.as_bytes()).map_err(|error| StoreError::Write(error.to_string()))?;
        fs::rename(&tmp, &self.path).map_err(|error| StoreError::Write(error.to_string()))?;
        Ok(())
    }

    fn back_up_corrupt(&self) -> String {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let backup = self.path.with_file_name(format!(
            "{}.corrupt-{stamp}",
            self.path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "projects.json".to_string())
        ));
        let _ = fs::rename(&self.path, &backup);
        format!(
            "Couldn't read saved projects; backed up to {}",
            backup.display()
        )
    }
}

/// Canonicalize a folder for stable project identity. Best-effort: a deleted or
/// not-yet-created folder falls back to the input path unchanged.
pub fn canonicalize_folder(folder: &str) -> String {
    fs::canonicalize(folder)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| folder.to_string())
}

/// Insert or replace a project, deduped by canonical folder id.
pub fn upsert(data: &mut ProjectStoreData, mut project: Project) {
    project.id = canonicalize_folder(&project.folder);
    project.folder = project.id.clone();
    if let Some(existing) = data.projects.iter_mut().find(|p| p.id == project.id) {
        *existing = project;
    } else {
        data.projects.push(project);
    }
}

pub fn append_task_to_folder(data: &mut ProjectStoreData, folder: &str, command: String) {
    let canonical = canonicalize_folder(folder);
    let task = Task {
        id: Uuid::new_v4().to_string(),
        name: command.clone(),
        command,
    };

    if let Some(existing) = data.projects.iter_mut().find(|p| p.id == canonical) {
        existing.tasks.push(task);
        return;
    }

    data.projects.push(Project {
        id: canonical.clone(),
        name: project_name_from_folder(&canonical),
        folder: canonical,
        tasks: vec![task],
    });
}

fn project_name_from_folder(folder: &str) -> String {
    PathBuf::from(folder)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or(folder)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ProjectStoreData {
        ProjectStoreData {
            version: 1,
            projects: vec![Project {
                id: "/Users/dev/web".to_string(),
                name: "web".to_string(),
                folder: "/Users/dev/web".to_string(),
                tasks: vec![Task {
                    id: "dev".to_string(),
                    name: "dev".to_string(),
                    command: "pnpm dev".to_string(),
                }],
            }],
        }
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let store = ProjectStore::new(dir.path().join("projects.json"));

        store.save(&sample()).unwrap();
        let outcome = store.load();

        assert_eq!(outcome.data, sample());
        assert!(outcome.notice.is_none());
    }

    #[test]
    fn missing_file_loads_empty() {
        let dir = tempfile::tempdir().unwrap();
        let store = ProjectStore::new(dir.path().join("projects.json"));

        let outcome = store.load();

        assert!(outcome.data.projects.is_empty());
        assert_eq!(outcome.data.version, CURRENT_VERSION);
        assert!(outcome.notice.is_none());
    }

    #[test]
    fn corrupt_file_is_backed_up_not_clobbered() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("projects.json");
        std::fs::write(&path, b"{ this is not json").unwrap();
        let store = ProjectStore::new(path.clone());

        let outcome = store.load();

        assert!(outcome.data.projects.is_empty());
        assert!(outcome.notice.is_some());
        let backups: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().contains(".corrupt-"))
            .collect();
        assert_eq!(
            backups.len(),
            1,
            "corrupt file should be preserved as a backup"
        );
    }

    #[test]
    fn save_is_atomic_leaving_no_tmp_file() {
        let dir = tempfile::tempdir().unwrap();
        let store = ProjectStore::new(dir.path().join("projects.json"));

        store.save(&sample()).unwrap();

        let leftovers: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(
            leftovers.is_empty(),
            "temp file must be renamed, not left behind"
        );
    }

    #[test]
    fn upsert_dedupes_by_folder_id() {
        let mut data = ProjectStoreData::default();
        let mut p = sample().projects.pop().unwrap();
        p.id = String::new();

        upsert(&mut data, p.clone());
        upsert(&mut data, p);

        assert_eq!(data.projects.len(), 1);
    }

    #[test]
    fn append_task_to_folder_preserves_existing_tasks() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("web");
        std::fs::create_dir(&folder).unwrap();
        let canonical = canonicalize_folder(&folder.to_string_lossy());
        let mut data = ProjectStoreData {
            version: 1,
            projects: vec![Project {
                id: canonical.clone(),
                name: "custom name".to_string(),
                folder: canonical.clone(),
                tasks: vec![Task {
                    id: "dev".to_string(),
                    name: "dev".to_string(),
                    command: "pnpm dev".to_string(),
                }],
            }],
        };

        append_task_to_folder(
            &mut data,
            &folder.to_string_lossy(),
            "pnpm test".to_string(),
        );

        assert_eq!(data.projects.len(), 1);
        assert_eq!(data.projects[0].id, canonical);
        assert_eq!(data.projects[0].name, "custom name");
        assert_eq!(data.projects[0].tasks.len(), 2);
        assert_eq!(data.projects[0].tasks[0].command, "pnpm dev");
        assert_eq!(data.projects[0].tasks[1].command, "pnpm test");
    }

    #[test]
    fn append_task_to_folder_creates_project_for_new_folder() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("api");
        std::fs::create_dir(&folder).unwrap();
        let mut data = ProjectStoreData::default();

        append_task_to_folder(
            &mut data,
            &folder.to_string_lossy(),
            "cargo run".to_string(),
        );

        assert_eq!(data.projects.len(), 1);
        assert_eq!(data.projects[0].name, "api");
        assert_eq!(data.projects[0].tasks.len(), 1);
        assert_eq!(data.projects[0].tasks[0].command, "cargo run");
    }
}
