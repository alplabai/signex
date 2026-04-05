mod commands;
mod engine;

use commands::project;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![
            project::open_project,
            project::get_app_info,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Alp EDA");
}
