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

        // Validate resolved path is within project dir
        let canonical_dir = dir
            .canonicalize()
            .map_err(|e| format!("Invalid project dir: {}", e))?;
        if sch_path.exists() {
            let canonical_sch = sch_path
                .canonicalize()
                .map_err(|e| format!("Invalid path: {}", e))?;
            if !canonical_sch.starts_with(&canonical_dir) {
                return Err("Path escapes project directory".to_string());
            }
        } else {
            return Err(format!("Schematic not found: {}", filename));
        }

        const MAX_SCH_BYTES: u64 = 100 * 1024 * 1024;
        let metadata = std::fs::metadata(&sch_path).map_err(|e| format!("Cannot stat file: {}", e))?;
        if metadata.len() > MAX_SCH_BYTES {
            return Err(format!("Schematic file too large ({} MiB, limit 100 MiB)", metadata.len() / 1_048_576));
        }

        let content = std::fs::read_to_string(&sch_path)
            .map_err(|e| format!("Failed to read {}: {}", filename, e))?;

        parser::parse_schematic(&content)
    })
    .await
    .map_err(|e| format!("Task failed: {}", e))?
}
