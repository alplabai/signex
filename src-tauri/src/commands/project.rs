use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::engine::parser;

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub name: String,
    pub path: String,
    pub dir: String,
    pub format: String,
    pub schematic_root: Option<String>,
    pub pcb_file: Option<String>,
    pub sheets: Vec<SheetInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SheetInfo {
    pub name: String,
    pub filename: String,
    pub symbols_count: usize,
    pub wires_count: usize,
    pub labels_count: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AppInfo {
    pub name: String,
    pub version: String,
}

#[tauri::command]
pub fn get_app_info() -> AppInfo {
    AppInfo {
        name: "Signex".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    }
}

#[tauri::command]
pub async fn pick_and_open_project() -> Result<Option<ProjectInfo>, String> {
    tokio::task::spawn_blocking(|| {
        let file = rfd::FileDialog::new()
            .set_title("Open Project")
            .add_filter("Signex Project", &["alpproj"])
            .add_filter("KiCad Project (Import)", &["kicad_pro"])
            .add_filter("All Files", &["*"])
            .pick_file();

        match file {
            Some(path) => {
                let path_str = path.to_string_lossy().to_string();
                do_open_project(&path_str).map(Some)
            }
            None => Ok(None),
        }
    })
    .await
    .map_err(|e| format!("Task failed: {}", e))?
}

#[tauri::command]
pub async fn open_project(path: String) -> Result<ProjectInfo, String> {
    tokio::task::spawn_blocking(move || do_open_project(&path))
        .await
        .map_err(|e| format!("Task failed: {}", e))?
}

fn do_open_project(path: &str) -> Result<ProjectInfo, String> {
    let project_path = Path::new(path);
    if !project_path.exists() {
        let name = project_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        return Err(format!("Project file not found: {}", name));
    }

    let ext = project_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    match ext {
        "kicad_pro" => open_kicad_project(project_path, path),
        "alpproj" => open_alp_project(project_path, path),
        _ => Err(format!("Unsupported project format: .{}", ext)),
    }
}

fn open_kicad_project(project_path: &Path, original_path: &str) -> Result<ProjectInfo, String> {
    let data = parser::parse_project(project_path)?;

    let sheets = data
        .sheets
        .iter()
        .map(|s| SheetInfo {
            name: s.name.clone(),
            filename: s.filename.clone(),
            symbols_count: s.symbols_count,
            wires_count: s.wires_count,
            labels_count: s.labels_count,
        })
        .collect();

    Ok(ProjectInfo {
        name: data.name,
        path: original_path.to_string(),
        dir: data.dir,
        format: "kicad".to_string(),
        schematic_root: data.schematic_root,
        pcb_file: data.pcb_file,
        sheets,
    })
}

fn open_alp_project(project_path: &Path, original_path: &str) -> Result<ProjectInfo, String> {
    let dir = project_path.parent().unwrap_or(Path::new("."));
    let name = project_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Untitled")
        .to_string();

    Ok(ProjectInfo {
        name,
        path: original_path.to_string(),
        dir: dir.to_string_lossy().to_string(),
        format: "alp".to_string(),
        schematic_root: None,
        pcb_file: None,
        sheets: vec![],
    })
}
