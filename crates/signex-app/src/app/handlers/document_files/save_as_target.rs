//! Which files a Save As may write to (#1823). The OS dialog filters by
//! extension on some platforms only, and a typed name or a confirmed
//! "Replace?" reaches the app unchecked, so the app refuses a target that
//! would turn a project or library file into a document.

use std::path::Path;

use anyhow::Result;

use super::super::super::*;
use super::save::file_label;

/// Whether `path` carries extension `ext`, ignoring case.
fn has_extension(path: &Path, ext: &str) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case(ext))
}

/// Whether both paths name one file, by spelling (case-folded) or on disk.
fn same_file(a: &Path, b: &Path) -> bool {
    if a.to_string_lossy()
        .eq_ignore_ascii_case(&b.to_string_lossy())
    {
        return true;
    }
    matches!((a.canonicalize(), b.canonicalize()), (Ok(x), Ok(y)) if x == y)
}

impl Signex {
    /// Refuse a Save As of a document of extension `ext` onto `target` when
    /// `target` is a loaded project's `.snxprj` or does not carry `ext`.
    /// Nothing has been written when this errs, and the document stays dirty.
    pub(super) fn refuse_save_as_target(&self, target: &Path, ext: &str) -> Result<()> {
        if self
            .document_state
            .projects
            .iter()
            .any(|project| same_file(&project.path, target))
        {
            anyhow::bail!(
                "that file is a project file; nothing was written: {}",
                file_label(target)
            );
        }
        if !has_extension(target, ext) {
            anyhow::bail!(
                "a Save As target must be a .{ext} file; nothing was written: {}",
                file_label(target)
            );
        }
        Ok(())
    }
}
