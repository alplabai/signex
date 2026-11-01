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

impl Signex {
    /// Refuse a Save As of a document of extension `ext` onto a project file
    /// (`.snxprj`, whether or not that project is loaded) or onto anything
    /// without `ext`. A project file is recognised by its extension alone, so
    /// no path comparison (and no second case-fold rule) is involved.
    /// Nothing has been written when this errs, and the document stays dirty.
    pub(super) fn refuse_save_as_target(&self, target: &Path, ext: &str) -> Result<()> {
        if has_extension(target, "snxprj") {
            anyhow::bail!(
                "that file is a project file; nothing was written: {}",
                file_label(target)
            );
        }
        if !has_extension(target, ext) {
            // A dotted name ("v1.2") has an extension the app does not know.
            anyhow::bail!(
                "a Save As target must be a .{ext} file (add .{ext} to the name); nothing \
                 was written: {}",
                file_label(target)
            );
        }
        Ok(())
    }
}
