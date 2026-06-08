pub mod bindings;
pub mod docker;
pub mod ports;
pub mod process;
pub mod projects;
pub mod state;
pub mod ui;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let bindings = bindings::builder();
    #[cfg(debug_assertions)]
    if let Err(error) = bindings::export() {
        eprintln!("warning: failed to export TypeScript bindings: {error}");
    }

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_positioner::init())
        .invoke_handler(bindings.invoke_handler());

    #[cfg(target_os = "macos")]
    let builder = builder.setup(move |app| {
        bindings.mount_events(app);
        state::start(app.handle().clone());
        ui::setup(app)?;
        Ok(())
    });

    builder
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit = event {
                if let Some(registry) =
                    app.try_state::<std::sync::Arc<std::sync::Mutex<crate::projects::ProjectRegistry>>>()
                {
                    crate::projects::kill_all_managed(&registry);
                }
            }
        });
}
