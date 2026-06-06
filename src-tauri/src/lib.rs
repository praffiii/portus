pub mod docker;
pub mod ports;
pub mod process;
pub mod state;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![state::set_active, state::set_idle]);

    #[cfg(target_os = "macos")]
    let builder = builder.setup(|app| {
        state::start(app.handle().clone());
        Ok(())
    });

    builder
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
