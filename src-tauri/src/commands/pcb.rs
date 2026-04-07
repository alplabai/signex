use std::path::Path;

use crate::engine::pcb_parser;

#[tauri::command]
pub async fn get_pcb(
    project_dir: String,
    filename: String,
) -> Result<pcb_parser::PcbBoard, String> {
    // Validate filename to prevent path traversal
    for comp in Path::new(&filename).components() {
        match comp {
            std::path::Component::ParentDir => {
                return Err("Invalid filename: path traversal not allowed".to_string());
            }
            std::path::Component::RootDir | std::path::Component::Prefix(_) => {
                return Err("Invalid filename: absolute paths not allowed".to_string());
            }
            _ => {}
        }
    }

    tokio::task::spawn_blocking(move || {
        let dir = Path::new(&project_dir);
        let pcb_path = dir.join(&filename);

        // Validate resolved path is within project dir
        let canonical_dir = dir
            .canonicalize()
            .map_err(|e| format!("Invalid project dir: {}", e))?;
        if pcb_path.exists() {
            let canonical_pcb = pcb_path
                .canonicalize()
                .map_err(|e| format!("Invalid path: {}", e))?;
            if !canonical_pcb.starts_with(&canonical_dir) {
                return Err("Path escapes project directory".to_string());
            }
        } else {
            return Err(format!("PCB file not found: {}", filename));
        }

        let content = std::fs::read_to_string(&pcb_path)
            .map_err(|e| format!("Failed to read {}: {}", filename, e))?;

        pcb_parser::parse_pcb(&content)
    })
    .await
    .map_err(|e| format!("Task failed: {}", e))?
}
