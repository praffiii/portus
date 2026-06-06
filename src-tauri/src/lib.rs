pub mod docker;
pub mod ports;
pub mod process;
pub mod state;
pub mod ui;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_positioner::init())
        .invoke_handler(tauri::generate_handler![state::set_active, state::set_idle]);

    #[cfg(target_os = "macos")]
    let builder = builder.setup(|app| {
        state::start(app.handle().clone());
        ui::setup(app)?;
        Ok(())
    });

    builder
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
