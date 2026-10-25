//! The `.gitignore` / `.gitattributes` text Signex manages inside a
//! project, and the IO that keeps it merged into the user's own file.
//!
//! The merge itself is [`crate::managed_block`]; this module owns what goes
//! in the block and the read-merge-atomic-write round trip.

use std::io;
use std::path::Path;

use crate::atomic_io::atomic_write;
use crate::managed_block::{ManagedBlockError, merge_managed_block};

/// Patterns Signex itself generates, derived from the code that writes them:
///
/// - `*.tmp`: [`crate::atomic_io::atomic_write`] stages every save at
///   `<file>.<pid>-<n>.tmp`; a crashed writer strands it.
/// - `.*.signex-writelock`: the library write-lock sidecar
///   (`.<library>.signex-writelock`) used when a library has no `.git/` yet.
/// - OS clutter: `.DS_Store`, `Thumbs.db`, `desktop.ini`.
///
/// Never listed: the `.snx*` sources, libraries, `assets/` and export
/// outputs; the unit tests pin that.
pub const GITIGNORE_BLOCK_BODY: &str = "\
# Atomic-write temp files left by a crashed save: <file>.<pid>-<n>.tmp
*.tmp
# Library write-lock sidecar (a library without a .git directory)
.*.signex-writelock
# OS clutter
.DS_Store
Thumbs.db
desktop.ini
";

/// Extensions Signex's native text formats use; kept LF so collaborators on
/// other platforms do not churn diffs.
const TEXT_EXTENSIONS: &[&str] = &[
    "snxsch", "snxpcb", "snxprj", "snxmat", "snxnet", "snxbom", "snxout", "snxsym", "snxfpt",
    "snxlib", "snxmod",
];

/// Attachments git must not diff or merge.
const BINARY_EXTENSIONS: &[&str] = &["step", "stp", "wrl", "iges", "png", "jpg", "jpeg", "pdf"];

/// 3D model extensions that go to Git LFS when the user opts in.
const LFS_MODEL_EXTENSIONS: &[&str] = &["step", "stp", "wrl", "iges"];

/// Raster assets (`assets/`) that go to Git LFS with the models. SVG is
/// text and stays a normal file.
const LFS_ASSET_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif"];

const LFS_ATTRS: &str = "filter=lfs diff=lfs merge=lfs -text";

/// Body of the managed `.gitattributes` block; `use_lfs` adds the LFS rules.
pub fn gitattributes_block_body(use_lfs: bool) -> String {
    let mut out = String::new();
    out.push_str("# Native Signex formats are line-based UTF-8: keep LF endings everywhere.\n");
    for ext in TEXT_EXTENSIONS {
        out.push_str(&format!("*.{ext}\ttext eol=lf\n"));
    }
    out.push_str("# Binary attachments: git should not diff or merge them.\n");
    for ext in BINARY_EXTENSIONS {
        out.push_str(&format!("*.{ext}\tbinary\n"));
    }
    if use_lfs {
        out.push_str("# 3D models and raster assets go through Git LFS.\n");
        for ext in LFS_MODEL_EXTENSIONS {
            out.push_str(&format!("*.{ext} {LFS_ATTRS}\n"));
        }
        out.push_str(&format!("assets/3d-models/** {LFS_ATTRS}\n"));
        for ext in LFS_ASSET_EXTENSIONS {
            out.push_str(&format!("assets/*.{ext} {LFS_ATTRS}\n"));
        }
    }
    out
}

/// Why a managed file could not be brought up to date. The file on disk is
/// unchanged in every case.
#[derive(Debug, thiserror::Error)]
pub enum ManagedFileError {
    #[error("read {path}: {source}")]
    Read { path: String, source: io::Error },
    #[error("{path} is not valid UTF-8; Signex will not edit it")]
    NotUtf8 { path: String },
    #[error("{path}: {source}")]
    Markers {
        path: String,
        source: ManagedBlockError,
    },
    #[error("write {path}: {source}")]
    Write { path: String, source: io::Error },
}

/// A file's text before and after the managed block is merged in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergedFile {
    /// The file as read; `None` when it does not exist.
    pub existing: Option<String>,
    /// What the file should hold once the block is in.
    pub merged: String,
}

impl MergedFile {
    /// True when writing `merged` would change the file.
    pub fn is_changed(&self) -> bool {
        self.existing.as_deref() != Some(self.merged.as_str())
    }
}

/// Read `path` and merge `body` in as the managed block, without writing.
/// A missing file is the same as an empty one.
pub fn merge_file(path: &Path, body: &str) -> Result<MergedFile, ManagedFileError> {
    let shown = path.display().to_string();
    let existing = match std::fs::read(path) {
        Ok(bytes) => Some(
            String::from_utf8(bytes).map_err(|_| ManagedFileError::NotUtf8 {
                path: shown.clone(),
            })?,
        ),
        Err(e) if e.kind() == io::ErrorKind::NotFound => None,
        Err(source) => {
            return Err(ManagedFileError::Read {
                path: shown,
                source,
            });
        }
    };
    let merged = merge_managed_block(existing.as_deref(), body).map_err(|source| {
        ManagedFileError::Markers {
            path: shown,
            source,
        }
    })?;
    Ok(MergedFile { existing, merged })
}

/// Merge `body` into the file at `path` and write it atomically; an
/// unchanged result writes nothing.
pub fn ensure_managed_file(path: &Path, body: &str) -> Result<(), ManagedFileError> {
    let file = merge_file(path, body)?;
    if !file.is_changed() {
        return Ok(());
    }
    atomic_write(path, file.merged.as_bytes()).map_err(|source| ManagedFileError::Write {
        path: path.display().to_string(),
        source,
    })
}

/// Keep the managed block in `<project_dir>/.gitignore` current, creating
/// the file when it does not exist.
pub fn ensure_project_gitignore(project_dir: &Path) -> Result<(), ManagedFileError> {
    ensure_managed_file(&project_dir.join(".gitignore"), GITIGNORE_BLOCK_BODY)
}

#[cfg(test)]
#[path = "project_vcs_files_tests.rs"]
mod project_vcs_files_tests;
