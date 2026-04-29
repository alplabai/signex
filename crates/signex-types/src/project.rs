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
    #[error("unsupported project file extension: .{0} (Signex Community only opens .snxprj; convert legacy projects with the import companion tool first)")]
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
/// Only native `.snxprj` project files are supported. Foreign-format
/// projects must be converted to `.snxprj` via the import companion
/// tool before opening.
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

    // Walk the root schematic to populate the full sheet tree so the
    // Projects panel mirrors hierarchical designs. Each parsed sheet
    // contributes its own row; any nested child sheets are appended in
    // depth-first order. We swallow read/parse errors per sheet so a
    // single corrupt file doesn't hide the rest of the project.
    let sheets = match &schematic_root {
        Some(root_name) => {
            let mut entries: Vec<SheetEntry> = Vec::new();
            let mut visited: std::collections::HashSet<String> = std::collections::HashSet::new();
            collect_sheet_entries(
                dir,
                root_name,
                &project_name,
                &mut entries,
                &mut visited,
            );
            if entries.is_empty() {
                vec![SheetEntry {
                    name: project_name.clone(),
                    filename: root_name.clone(),
                    symbols_count: 0,
                    wires_count: 0,
                    labels_count: 0,
                }]
            } else {
                entries
            }
        }
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

/// Recursively walk a schematic and any hierarchical child sheets,
/// pushing one [`SheetEntry`] per discovered sheet (rooted at
/// `project_dir`). `visited` short-circuits cycles by relative
/// filename. Errors reading or parsing a sheet are swallowed so the
/// project tree stays usable when an individual file is corrupt.
fn collect_sheet_entries(
    project_dir: &Path,
    relative_filename: &str,
    display_name: &str,
    entries: &mut Vec<SheetEntry>,
    visited: &mut std::collections::HashSet<String>,
) {
    if !visited.insert(relative_filename.to_string()) {
        return;
    }

    let abs = project_dir.join(relative_filename);
    let Ok(text) = std::fs::read_to_string(&abs) else {
        entries.push(SheetEntry {
            name: display_name.to_string(),
            filename: relative_filename.to_string(),
            symbols_count: 0,
            wires_count: 0,
            labels_count: 0,
        });
        return;
    };

    let Ok(parsed) = crate::format::SnxSchematic::parse(&text) else {
        entries.push(SheetEntry {
            name: display_name.to_string(),
            filename: relative_filename.to_string(),
            symbols_count: 0,
            wires_count: 0,
            labels_count: 0,
        });
        return;
    };

    let sheet = &parsed.sheet;
    entries.push(SheetEntry {
        name: display_name.to_string(),
        filename: relative_filename.to_string(),
        symbols_count: sheet.symbols.len(),
        wires_count: sheet.wires.len(),
        labels_count: sheet.labels.len(),
    });

    let parent_dir = abs.parent().unwrap_or(project_dir);
    for child in &sheet.child_sheets {
        let child_filename_trimmed = child.filename.trim();
        if child_filename_trimmed.is_empty() {
            continue;
        }

        let child_abs = if Path::new(child_filename_trimmed).is_absolute() {
            std::path::PathBuf::from(child_filename_trimmed)
        } else {
            parent_dir.join(child_filename_trimmed)
        };

        let child_relative = child_abs
            .strip_prefix(project_dir)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| child_abs.to_string_lossy().to_string());

        let child_display = if child.name.trim().is_empty() {
            child_abs
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| child_relative.clone())
        } else {
            child.name.clone()
        };

        collect_sheet_entries(
            project_dir,
            &child_relative,
            &child_display,
            entries,
            visited,
        );
    }
}
