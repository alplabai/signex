use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub name: String,
    pub path: String,
    pub schematics: Vec<String>,
    pub pcb: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AppInfo {
    pub name: String,
    pub version: String,
}

#[tauri::command]
pub fn get_app_info() -> AppInfo {
    AppInfo {
        name: "Alp EDA".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    }
}

#[tauri::command]
pub fn open_project(path: String) -> Result<ProjectInfo, String> {
    // Phase 0: stub — will integrate KiCad parser in Week 2
    let project_path = std::path::Path::new(&path);
    if !project_path.exists() {
        let name = project_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        return Err(format!("Project file not found: {}", name));
    }

    let name = project_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Untitled")
        .to_string();

    Ok(ProjectInfo {
        name,
        path,
        schematics: vec![],
        pcb: None,
    })
}
