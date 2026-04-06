use std::path::Path;
use crate::engine::{parser::SchematicSheet, writer};

/// Validate that a filename doesn't escape the project directory via traversal
fn validate_filename(filename: &str) -> Result<(), String> {
    let p = Path::new(filename);
    for comp in p.components() {
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
    Ok(())
}

#[tauri::command]
pub async fn save_schematic(
    project_dir: String,
    filename: String,
    data: SchematicSheet,
) -> Result<(), String> {
    validate_filename(&filename)?;

    tokio::task::spawn_blocking(move || {
        let dir = Path::new(&project_dir);
        let path = dir.join(&filename);

        // Double-check resolved path is within project dir
        let canonical_dir = dir.canonicalize()
            .map_err(|e| format!("Invalid project dir: {}", e))?;
        let canonical_path = path.parent()
            .unwrap_or(dir)
            .canonicalize()
            .map_err(|e| format!("Invalid path: {}", e))?;
        if !canonical_path.starts_with(&canonical_dir) {
            return Err("Path escapes project directory".to_string());
        }

        let content = writer::write_schematic(&data);

        std::fs::write(&path, content)
            .map_err(|e| format!("Failed to write {}: {}", filename, e))
    })
    .await
    .map_err(|e| format!("Task failed: {}", e))?
}
