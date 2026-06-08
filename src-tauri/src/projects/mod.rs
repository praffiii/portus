use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;

mod parse;
mod spawn;
mod store;

pub use parse::{
    detect_package_manager, parse_compose_services, parse_package_scripts, tasks_from_folder,
};
pub use spawn::{spawn_task, RingBuffer, SpawnedProcess, RING_CAPACITY};
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
