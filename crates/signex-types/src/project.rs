use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Document type
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentType {
    Schematic,
    Pcb,
    Library,
    OutputJob,
}

// ---------------------------------------------------------------------------
// Document handle
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub id: Uuid,
    pub name: String,
    pub doc_type: DocumentType,
    pub path: String,
    #[serde(default)]
    pub dirty: bool,
}

// ---------------------------------------------------------------------------
// Sheet entry (summary row for the project tree)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SheetEntry {
    pub name: String,
    pub filename: String,
    #[serde(default)]
    pub symbols_count: usize,
    #[serde(default)]
    pub wires_count: usize,
    #[serde(default)]
    pub labels_count: usize,
}

// ---------------------------------------------------------------------------
// Project data
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectData {
    pub name: String,
    pub dir: String,
    pub schematic_root: Option<String>,
    pub pcb_file: Option<String>,
    #[serde(default)]
    pub sheets: Vec<SheetEntry>,
    /// Schematic-level variant definitions.
    #[serde(default)]
    pub variant_definitions: Vec<String>,
    /// Currently selected variant if known from project context.
    #[serde(default)]
    pub active_variant: Option<String>,
}

// ---------------------------------------------------------------------------
// Project-file parser
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum ProjectError {
    #[error("project file not found: {0}")]
    NotFound(String),
    #[error("io error reading {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("unsupported project file extension: .{0} (Signex Community only opens .snxprj; convert KiCad projects with the signex-kicad-import companion)")]
    UnsupportedExtension(String),
}

/// Parse a `.snxprj` project file and discover the root schematic + companion PCB.
///
/// This is a directory-walk parser: it reads the project filename to find the
/// project name, then probes the same directory for `<name>.snxsch` and
/// `<name>.snxpcb`. The full sheet tree (used by the project tree to show
/// nested schematics) is populated by walking the root schematic at runtime
/// — this lightweight parser only sees the filenames present on disk.
///
/// KiCad project files (`.kicad_pro`) are not supported in Signex Community.
/// Users running KiCad projects use the optional `signex-kicad-import`
/// GPL-3.0 companion tool to convert their files first.
pub fn parse_project(path: &Path) -> Result<ProjectData, ProjectError> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    if !matches!(ext.as_str(), "snxprj") {
        return Err(ProjectError::UnsupportedExtension(ext));
    }

    let dir = path.parent().unwrap_or(Path::new("."));
    let project_name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Untitled")
        .to_string();

    // Schematic root: prefer .snxsch in the project directory.
    let snx_sch_name = format!("{}.snxsch", project_name);
    let schematic_root = if dir.join(&snx_sch_name).exists() {
        Some(snx_sch_name)
    } else {
        None
    };

    // Companion PCB.
    let snx_pcb_name = format!("{}.snxpcb", project_name);
    let pcb_file = if dir.join(&snx_pcb_name).exists() {
        Some(snx_pcb_name)
    } else {
        None
    };

    // The detailed sheet tree (counts, names, child sheets) is populated
    // lazily by the engine when a schematic is opened. The project parser
    // returns the root entry only.
    let sheets = match &schematic_root {
        Some(root_name) => vec![SheetEntry {
            name: project_name.clone(),
            filename: root_name.clone(),
            symbols_count: 0,
            wires_count: 0,
            labels_count: 0,
        }],
        None => Vec::new(),
    };

    Ok(ProjectData {
        name: project_name,
        dir: dir.to_string_lossy().to_string(),
        schematic_root,
        pcb_file,
        sheets,
        variant_definitions: Vec::new(),
        active_variant: None,
    })
}
