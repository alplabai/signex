//! The `.gitignore` / `.gitattributes` text Signex manages inside a
//! project, and the IO that keeps it merged into the user's own file.
//!
//! The merge itself is [`crate::managed_block`]; this module owns what goes
//! in the block and the read-merge-atomic-write round trip.

use std::io;
use std::path::Path;

use crate::atomic_io::atomic_write;
use crate::managed_block::{BlockPlacement, ManagedBlockError, merge_managed_block};

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
    #[error("{path} is not a regular file (a link or special file); Signex will not edit it")]
    NotRegularFile { path: String },
    #[error("{path} is read-only; Signex will not replace it")]
    ReadOnly { path: String },
    /// Detected on Unix only: std offers no stable link count on Windows.
    #[error("{path} has several hard links; Signex will not replace it")]
    HardLinked { path: String },
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
    merge_file_with(path, body, BlockPlacement::Top, |text| text.to_owned())
}

/// Like [`merge_file`], with `normalise` applied to the existing text before
/// the block goes in (the returned `existing` stays the file as read), and
/// `placement` deciding where a block not yet in the file goes.
fn merge_file_with(
    path: &Path,
    body: &str,
    placement: BlockPlacement,
    normalise: impl Fn(&str) -> String,
) -> Result<MergedFile, ManagedFileError> {
    let shown = path.display().to_string();
    let Some(existing) = read_regular_utf8_inner(path, &shown)? else {
        let merged = merge_managed_block(None, body, placement).map_err(|source| {
            ManagedFileError::Markers {
                path: shown,
                source,
            }
        })?;
        return Ok(MergedFile {
            existing: None,
            merged,
        });
    };
    let merged =
        merge_managed_block(Some(&normalise(&existing)), body, placement).map_err(|source| {
            ManagedFileError::Markers {
                path: shown,
                source,
            }
        })?;
    Ok(MergedFile {
        existing: Some(existing),
        merged,
    })
}

/// The text of `path` for an edit that keeps it a regular file; `None` when
/// the file is missing. Refuses links, special, read-only and (on Unix only)
/// hard-linked files exactly as the managed merge does.
pub fn read_for_edit(path: &Path) -> Result<Option<String>, ManagedFileError> {
    read_regular_utf8_inner(path, &path.display().to_string())
}

/// Read `path` only when it is an ordinary, writable, singly-linked file.
///
/// A link or special file is refused before it is opened: following a link
/// would pull a file from outside the project into the managed file, and a
/// FIFO would block the read forever. `Ok(None)` means the file is missing.
fn read_regular_utf8_inner(path: &Path, shown: &str) -> Result<Option<String>, ManagedFileError> {
    let meta = match std::fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(ManagedFileError::Read {
                path: shown.to_owned(),
                source,
            });
        }
    };
    if !meta.is_file() {
        return Err(ManagedFileError::NotRegularFile {
            path: shown.to_owned(),
        });
    }
    // The atomic write replaces the file, which would silently widen a
    // read-only file's permissions and detach a hard link from its twin.
    // The link count is read on Unix only; stable std has no equivalent on
    // Windows, so a hard-linked file there is replaced and detached.
    if meta.permissions().readonly() {
        return Err(ManagedFileError::ReadOnly {
            path: shown.to_owned(),
        });
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if meta.nlink() > 1 {
            return Err(ManagedFileError::HardLinked {
                path: shown.to_owned(),
            });
        }
    }
    let bytes = std::fs::read(path).map_err(|source| ManagedFileError::Read {
        path: shown.to_owned(),
        source,
    })?;
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|_| ManagedFileError::NotUtf8 {
            path: shown.to_owned(),
        })
}

/// First line of the `.gitattributes` Signex wrote whole before it became a
/// managed block.
const LEGACY_FIRST_LINE: &str =
    "# Generated by Signex when version control was enabled for this project.";

/// Every line the whole-file generator could emit.
fn legacy_generated_lines() -> Vec<String> {
    let mut lines: Vec<String> = [
        LEGACY_FIRST_LINE,
        "# Re-running \"Enable Version Control\" rewrites this file from scratch \u{2014}",
        "# manual edits will be lost. Hand-edit only if you're not planning to",
        "# re-run the modal.",
        "# Native Signex formats are line-based UTF-8 \u{2014} keep LF endings everywhere",
        "# so cross-platform collaborators don't churn the diff with CRLF flips.",
        "# Binary attachments \u{2014} git shouldn't diff or merge them.",
        "# 3D models opt-in via Git LFS so the working tree doesn't bloat.",
        "assets/3d-models/**\tfilter=lfs diff=lfs merge=lfs -text",
    ]
    .iter()
    .map(|l| (*l).to_owned())
    .collect();
    lines.extend(
        TEXT_EXTENSIONS
            .iter()
            .map(|e| format!("*.{e}\ttext eol=lf")),
    );
    lines.extend(BINARY_EXTENSIONS.iter().map(|e| format!("*.{e}\tbinary")));
    lines
}

/// Drop the whole-file generated content of an old Signex `.gitattributes`
/// (recognised by its first non-blank line) so the managed block replaces it
/// instead of repeating it. Every other line, the user's, stays.
///
/// The legacy file always carried `assets/3d-models/**\tfilter=lfs ...`,
/// and that line is dropped here whatever `use_lfs` is now. Turning Version
/// Control back on with LFS off therefore ends with no LFS rule for the
/// models, which is the user's choice; the managed block adds the LFS rules
/// only when `use_lfs` is set.
fn strip_legacy_gitattributes(text: &str) -> String {
    let first = text.lines().find(|l| !l.trim().is_empty());
    if first.map(str::trim_end) != Some(LEGACY_FIRST_LINE) {
        return text.to_owned();
    }
    let generated = legacy_generated_lines();
    let mut kept = String::new();
    for line in text.split_inclusive('\n') {
        let bare = line.trim_end_matches(['\r', '\n']);
        let drop = generated.iter().any(|g| g == bare) || (bare.is_empty() && kept.is_empty());
        if !drop {
            kept.push_str(line);
        }
    }
    // Blank separators the generator put between sections are not the
    // user's; trim the ones left at the very end.
    while kept.ends_with("\n\n") {
        kept.pop();
    }
    kept
}

/// Merge `body` into the file at `path` and write it atomically; an
/// unchanged result writes nothing.
pub fn ensure_managed_file(path: &Path, body: &str) -> Result<(), ManagedFileError> {
    write_if_changed(path, &merge_file(path, body)?)
}

fn write_if_changed(path: &Path, file: &MergedFile) -> Result<(), ManagedFileError> {
    if !file.is_changed() {
        return Ok(());
    }
    atomic_write(path, file.merged.as_bytes()).map_err(|source| ManagedFileError::Write {
        path: path.display().to_string(),
        source,
    })
}

/// Keep the managed block in `<project_dir>/.gitignore` current, creating
/// the file when it does not exist. The block goes into the file in
/// `project_dir` itself: when that directory is the root of an existing
/// repository, that repository's top-level `.gitignore` gains the block.
pub fn ensure_project_gitignore(project_dir: &Path) -> Result<(), ManagedFileError> {
    ensure_managed_file(&project_dir.join(".gitignore"), GITIGNORE_BLOCK_BODY)
}

/// Read `<project_dir>/.gitattributes` and merge the managed block in
/// without writing; the old whole-file generated content is replaced.
pub fn merge_gitattributes_file(
    project_dir: &Path,
    use_lfs: bool,
) -> Result<MergedFile, ManagedFileError> {
    merge_file_with(
        &project_dir.join(".gitattributes"),
        &gitattributes_block_body(use_lfs),
        BlockPlacement::Top,
        strip_legacy_gitattributes,
    )
}

/// Keep the managed block in `<project_dir>/.gitattributes` current.
pub fn ensure_gitattributes(project_dir: &Path, use_lfs: bool) -> Result<(), ManagedFileError> {
    let file = merge_gitattributes_file(project_dir, use_lfs)?;
    write_if_changed(&project_dir.join(".gitattributes"), &file)
}

#[cfg(test)]
#[path = "project_vcs_files_tests.rs"]
mod project_vcs_files_tests;
