mod commands;
mod engine;

use commands::{export, library, project, save, schematic};

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
            library::list_libraries,
            library::search_symbols,
            library::get_symbol,
            export::generate_bom,
            export::export_netlist,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Signex");
}
