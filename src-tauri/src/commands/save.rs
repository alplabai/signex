use std::path::Path;
use crate::engine::{parser::SchematicSheet, writer};

#[tauri::command]
pub async fn save_schematic(
    project_dir: String,
    filename: String,
    data: SchematicSheet,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let dir = Path::new(&project_dir);
        let path = dir.join(&filename);

        let content = writer::write_schematic(&data);

        std::fs::write(&path, content)
            .map_err(|e| format!("Failed to write {}: {}", filename, e))
    })
    .await
    .map_err(|e| format!("Task failed: {}", e))?
}
