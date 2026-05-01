use std::path::Path;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
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
// Library entry (per-project library mount)
// ---------------------------------------------------------------------------

/// How a [`LibraryEntry`] resolves on disk. Project-local libraries live
/// under the project directory and use a relative path; shared / global
/// libraries live elsewhere on the user's machine and use an absolute
/// path. Drives the auto-mount path resolution at project-open time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LibraryEntryKind {
    /// `*.snxlib/` directory inside the project directory. `path` is
    /// relative to `ProjectData::dir`.
    ProjectLocal,
    /// Shared library on the user's machine — `path` is absolute.
    Shared,
    /// Global / system library — `path` is absolute.
    Global,
}

/// One library reference recorded in `.snxprj`. The project loader
/// iterates this list at open-time and mounts each library via the
/// matching adapter (currently `LocalGitAdapter` for all three kinds).
///
/// See `docs/internal/docs/v0.9-library-plan.md` for the data-model
/// rationale.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LibraryEntry {
    /// On-disk location. Relative to the project dir for
    /// [`LibraryEntryKind::ProjectLocal`]; absolute for the others.
    pub path: PathBuf,
    pub kind: LibraryEntryKind,
    /// `library_id` from `library.toml`, populated after first open
    /// so `.snxprj` lookups can follow the library when it moves.
    /// `None` for entries that have never been opened yet.
    #[serde(default)]
    pub library_id: Option<Uuid>,
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
    /// Component libraries referenced by this project. Project-open
    /// auto-mounts every entry; the project tree renders each as a
    /// `Libraries ▸ <name>.snxlib` node. `#[serde(default)]` so old
    /// `.snxprj` files (and `.standard_pro` files which know nothing
    /// about Signex libraries) load with an empty list.
    #[serde(default)]
    pub libraries: Vec<LibraryEntry>,
}

impl ProjectData {
    /// Resolve a [`LibraryEntry`]'s `path` to an absolute path. Project-
    /// local entries are joined against `dir`; shared/global entries
    /// are returned as-is. Used by both the auto-mount loop and the
    /// project-tree renderer.
    pub fn resolve_library_path(&self, entry: &LibraryEntry) -> PathBuf {
        match entry.kind {
            LibraryEntryKind::ProjectLocal => PathBuf::from(&self.dir).join(&entry.path),
            LibraryEntryKind::Shared | LibraryEntryKind::Global => entry.path.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `.snxprj` files written before the `libraries` field landed
    /// must round-trip cleanly with an empty list. Backwards-compat
    /// is the load-bearing constraint here — we cannot break
    /// existing project files.
    #[test]
    fn project_data_loads_without_libraries_field() {
        let json = r#"{
            "name": "test",
            "dir": "/tmp/test",
            "schematic_root": null,
            "pcb_file": null
        }"#;
        let parsed: ProjectData = serde_json::from_str(json).expect("parse");
        assert_eq!(parsed.name, "test");
        assert!(parsed.libraries.is_empty());
    }

    #[test]
    fn library_entry_round_trips_through_serde() {
        let entry = LibraryEntry {
            path: PathBuf::from("MyLib.snxlib"),
            kind: LibraryEntryKind::ProjectLocal,
            library_id: Some(Uuid::nil()),
        };
        let json = serde_json::to_string(&entry).expect("serialize");
        let back: LibraryEntry = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(entry, back);
    }

    #[test]
    fn library_entry_loads_without_library_id_field() {
        // Older snapshots may omit `library_id` when the library has
        // never been mounted. `#[serde(default)]` on the field must
        // keep that path working.
        let json = r#"{
            "path": "lib/Power.snxlib",
            "kind": "shared"
        }"#;
        let parsed: LibraryEntry = serde_json::from_str(json).expect("parse");
        assert_eq!(parsed.path, PathBuf::from("lib/Power.snxlib"));
        assert_eq!(parsed.kind, LibraryEntryKind::Shared);
        assert!(parsed.library_id.is_none());
    }

    #[test]
    fn resolve_library_path_joins_project_local() {
        let project = ProjectData {
            name: "p".into(),
            dir: "/projects/foo".into(),
            schematic_root: None,
            pcb_file: None,
            sheets: vec![],
            variant_definitions: vec![],
            active_variant: None,
            libraries: vec![],
        };
        let local = LibraryEntry {
            path: PathBuf::from("foo-lib.snxlib"),
            kind: LibraryEntryKind::ProjectLocal,
            library_id: None,
        };
        assert_eq!(
            project.resolve_library_path(&local),
            PathBuf::from("/projects/foo").join("foo-lib.snxlib")
        );
    }

    #[test]
    fn resolve_library_path_keeps_absolute_for_shared() {
        let project = ProjectData {
            name: "p".into(),
            dir: "/projects/foo".into(),
            schematic_root: None,
            pcb_file: None,
            sheets: vec![],
            variant_definitions: vec![],
            active_variant: None,
            libraries: vec![],
        };
        let shared = LibraryEntry {
            path: PathBuf::from("/var/signex/Power.snxlib"),
            kind: LibraryEntryKind::Shared,
            library_id: None,
        };
        assert_eq!(
            project.resolve_library_path(&shared),
            PathBuf::from("/var/signex/Power.snxlib")
        );
    }
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
    #[error("unsupported project file extension: .{0} (Signex Community only opens .snxprj; convert Standard projects with the signex-standard-import companion)")]
    UnsupportedExtension(String),
}

/// Parse a `.snxprj` project file. Two paths:
///
/// 1. **JSON content** — newer projects ship a serialized [`ProjectData`]
///    inside the file. We deserialize it directly and patch `dir` to
///    match the file's actual location (so projects keep working when
///    copied to a different folder).
/// 2. **Empty / legacy marker** — older `.snxprj` files were empty
///    markers; the parser inferred everything from the directory
///    (probe for `<name>.snxsch` / `<name>.snxpcb`). We keep that
///    fallback so existing projects keep loading.
///
/// Standard project files (`.standard_pro`) are not supported in Signex
/// Community. Users running Standard projects use the optional
/// `signex-standard-import` GPL-3.0 companion tool to convert their files
/// first.
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

    // JSON-backed projects: deserialize ProjectData directly. We don't
    // care if the persisted `dir` is stale (projects move); patch it to
    // the parent of the path the user actually opened.
    let bytes = std::fs::read(path).map_err(|source| ProjectError::Io {
        path: path.display().to_string(),
        source,
    })?;
    let trimmed = std::str::from_utf8(&bytes)
        .map(|s| s.trim_start())
        .unwrap_or("");
    if trimmed.starts_with('{') {
        if let Ok(mut data) = serde_json::from_slice::<ProjectData>(&bytes) {
            data.dir = dir.to_string_lossy().to_string();
            // Project name in the file is informational; the on-disk
            // filename is authoritative so renaming the .snxprj outside
            // the app doesn't desync the displayed name.
            data.name = project_name;
            return Ok(data);
        }
        // Falls through to directory probe if JSON parse fails; the
        // user's project is still recoverable from disk layout.
    }

    // Legacy/empty marker — directory-driven probe (the original
    // pre-JSON behaviour).
    let snx_sch_name = format!("{}.snxsch", project_name);
    let schematic_root = if dir.join(&snx_sch_name).exists() {
        Some(snx_sch_name)
    } else {
        None
    };

    let snx_pcb_name = format!("{}.snxpcb", project_name);
    let pcb_file = if dir.join(&snx_pcb_name).exists() {
        Some(snx_pcb_name)
    } else {
        None
    };

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
        libraries: Vec::new(),
    })
}

/// Serialize `data` to `path` as pretty JSON. Companion of
/// [`parse_project`] for the JSON-backed branch — newly-added sheets,
/// PCB, and libraries persist through this writer.
pub fn write_project(path: &Path, data: &ProjectData) -> Result<(), ProjectError> {
    let json = serde_json::to_vec_pretty(data).map_err(|err| ProjectError::Io {
        path: path.display().to_string(),
        source: std::io::Error::other(err.to_string()),
    })?;
    std::fs::write(path, json).map_err(|source| ProjectError::Io {
        path: path.display().to_string(),
        source,
    })
}
