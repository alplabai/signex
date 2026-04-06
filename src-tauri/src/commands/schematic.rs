use std::path::Path;

use crate::engine::parser;

#[tauri::command]
pub async fn get_schematic(
    project_dir: String,
    filename: String,
) -> Result<parser::SchematicSheet, String> {
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
        let sch_path = dir.join(&filename);

        if !sch_path.exists() {
            return Err(format!("Schematic not found: {}", filename));
        }

        let content = std::fs::read_to_string(&sch_path)
            .map_err(|e| format!("Failed to read {}: {}", filename, e))?;

        parser::parse_schematic(&content)
    })
    .await
    .map_err(|e| format!("Task failed: {}", e))?
}
