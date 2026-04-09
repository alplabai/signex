mod commands;
mod engine;

use commands::{export, library, pcb, project, save, schematic, signal, simulation};

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
            library::list_library_symbols,
            library::get_symbol,
            library::save_symbol,
            library::get_footprint,
            library::save_footprint,
            export::generate_bom,
            export::generate_bom_configured,
            export::export_netlist,
            export::export_netlist_xml,
            pcb::get_pcb,
            signal::set_api_key,
            signal::set_signex_backend,
            signal::has_api_key,
            signal::get_api_mode,
            signal::signal_chat,
            signal::signal_chat_stream,
            signal::signal_review,
            signal::signal_fix_erc,
            simulation::detect_solvers,
            simulation::get_spice_netlist,
            simulation::run_spice_simulation,
            simulation::calculate_impedance,
            simulation::get_default_stackup,
        ])
        .run(tauri::generate_context!())
        .unwrap_or_else(|e| {
            eprintln!("Signex failed to start: {}", e);
            std::process::exit(1);
        });
}
