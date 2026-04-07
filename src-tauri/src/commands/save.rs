use crate::engine::{parser::SchematicSheet, writer};
use std::path::Path;

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
        let canonical_dir = dir
            .canonicalize()
            .map_err(|e| format!("Invalid project dir: {}", e))?;
        let canonical_path = dir.join(&filename).parent().unwrap_or(dir).to_path_buf();
        let canonical_path = if canonical_path.as_os_str().is_empty() {
            canonical_dir.clone()
        } else {
            canonical_path
                .canonicalize()
                .map_err(|e| format!("Invalid path: {}", e))?
        };
        if !canonical_path.starts_with(&canonical_dir) {
            return Err("Path escapes project directory".to_string());
        }

        let content = writer::write_schematic(&data);

        // Atomic write: write to temp file then rename (prevents corruption on crash)
        let tmp_path = path.with_extension("kicad_sch.tmp");
        std::fs::write(&tmp_path, &content)
            .map_err(|e| format!("Failed to write temp file: {}", e))?;
        std::fs::rename(&tmp_path, &path).map_err(|e| {
            // Clean up temp file on rename failure
            let _ = std::fs::remove_file(&tmp_path);
            format!("Failed to save {}: {}", filename, e)
        })
    })
    .await
    .map_err(|e| format!("Task failed: {}", e))?
}
