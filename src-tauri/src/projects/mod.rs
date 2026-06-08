use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;

pub(crate) mod commands;
mod env;
mod kill;
mod lifecycle;
mod parse;
mod registry;
mod spawn;
mod store;

pub use lifecycle::{classify, ExitState, Lifecycle};
pub use parse::{
    detect_package_manager, parse_compose_services, parse_package_scripts, tasks_from_folder,
};
pub use registry::{ManagedStatus, ProjectRegistry};
pub use spawn::{spawn_task, ProcessExitStatus, RingBuffer, SpawnedProcess, RING_CAPACITY};
pub use store::{
    canonicalize_folder, upsert, LoadOutcome, ProjectStore, ProjectStoreData, StoreError,
    CURRENT_VERSION,
};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
pub struct Task {
    pub id: String,
    pub name: String,
    pub command: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub folder: String,
    pub tasks: Vec<Task>,
}

#[derive(Debug, Error)]
pub enum ProjectError {
    #[error(transparent)]
    Store(#[from] StoreError),
}
pub use commands::{
    candidates_from_chain_for_test, load_projects, remove_project, save_project, start_task,
    stop_task, subscribe_logs, suggest_tasks, unsubscribe_logs, ProjectsState, SaveAsCandidate,
};
pub use kill::{kill_all_managed, kill_group, QUIT_GRACE};
