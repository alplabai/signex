mod commands;
mod engine;

use commands::{project, save, schematic};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        // dialog handled via rfd crate directly (tauri dialog plugin freezes WebView)
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![
            project::open_project,
            project::pick_and_open_project,
            project::get_app_info,
            schematic::get_schematic,
            save::save_schematic,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Alp EDA");
}
