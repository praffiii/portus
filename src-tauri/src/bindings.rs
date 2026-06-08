use std::path::{Path, PathBuf};

use specta_typescript::Typescript;
use tauri_specta::{collect_commands, collect_events, Builder};

use crate::docker::{DockerContainer, DockerSnapshot};
use crate::logs::ansi::{LogBatch, LogLine};
use crate::process::KillTarget;
use crate::state::{self, KillProcessError, Snapshot};

const TYPESCRIPT_BINDINGS_PATH: &str = "../src/lib/bindings.ts";

pub fn builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new()
        .commands(collect_commands![
            state::set_active,
            state::set_idle,
            state::kill_process_tree,
            crate::projects::commands::load_projects,
            crate::projects::commands::suggest_tasks,
            crate::projects::commands::save_project,
            crate::projects::commands::remove_project,
            crate::projects::commands::start_task,
            crate::projects::commands::stop_task,
            crate::projects::commands::subscribe_logs,
            crate::projects::commands::unsubscribe_logs,
            crate::projects::commands::save_as_candidates
        ])
        .events(collect_events![Snapshot])
        .typ::<DockerContainer>()
        .typ::<DockerSnapshot>()
        .typ::<KillProcessError>()
        .typ::<KillTarget>()
        .typ::<LogBatch>()
        .typ::<LogLine>()
        .typ::<crate::projects::Project>()
        .typ::<crate::projects::Task>()
        .typ::<crate::projects::ManagedStatus>()
        .typ::<crate::projects::SaveAsCandidate>()
        .dangerously_cast_bigints_to_number()
}

pub fn typescript_bindings_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(TYPESCRIPT_BINDINGS_PATH)
}

pub fn export_to(path: impl AsRef<Path>) -> Result<(), specta_typescript::Error> {
    builder().export(Typescript::default(), path)
}

#[cfg(debug_assertions)]
pub fn export() -> Result<(), specta_typescript::Error> {
    export_to(typescript_bindings_path())
}

#[cfg(test)]
mod tests {
    #[test]
    fn exports_commands_and_snapshot_event_without_writing_source_tree() {
        let dir = tempfile::tempdir().expect("create temporary bindings directory");
        let output = dir.path().join("bindings.ts");

        super::export_to(&output).expect("export bindings");

        let bindings = std::fs::read_to_string(output).expect("read bindings");
        assert!(bindings.contains("setActive"));
        assert!(bindings.contains("setIdle"));
        assert!(bindings.contains("killProcessTree"));
        assert!(bindings.contains("loadProjects"));
        assert!(bindings.contains("startTask"));
        assert!(bindings.contains("subscribeLogs"));
        assert!(bindings.contains("snapshot"));
        assert!(bindings.contains("export type LogBatch"));
        assert!(bindings.contains("export type Snapshot"));
    }
}
