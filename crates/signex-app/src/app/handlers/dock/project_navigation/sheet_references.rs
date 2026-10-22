//! Keeping hierarchical sheet references in step with a renamed `.snxsch`.
//!
//! Renaming a child sheet's file leaves every parent's `ChildSheet.filename`
//! pointing at a path that no longer exists. The fix goes through the engine
//! (`Command::RenameChildSheetFile`) so it is undoable and dirty-tracked; the
//! parents are only marked dirty, never written — the user's explicit save
//! (or Save All on close) persists them.

use std::path::{Path, PathBuf};

use super::*;
use crate::app::project_sheets::{assemble_project_sheets, project_root_sheet_path};
use crate::app::state::scope::path_key;

/// One parent-sheet reference to re-point: `parent` holds a child entry whose
/// `filename` is currently `from` and must become `to`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct ReferenceRewrite {
    pub(super) parent: PathBuf,
    pub(super) from: String,
    pub(super) to: String,
}

/// What a rename has to do to the rest of the project, planned before the
/// file moves.
#[derive(Debug, Default)]
pub(super) struct ReferencePlan {
    pub(super) rewrites: Vec<ReferenceRewrite>,
    /// Project sheets that could not be read or parsed, so whether they
    /// reference the renamed file is unknown. Reported, never skipped silently.
    pub(super) unchecked: Vec<String>,
}

/// `filename` with its last path component replaced by `new_name`, keeping any
/// directory prefix the parent used to reach the child.
fn with_new_leaf(filename: &str, new_name: &str) -> String {
    match filename.rfind(['/', '\\']) {
        Some(idx) => format!("{}{new_name}", &filename[..=idx]),
        None => new_name.to_string(),
    }
}

/// What applying a [`ReferencePlan`] could not finish.
#[derive(Debug, Default)]
pub(super) struct RewriteOutcome {
    /// References that were found but could not be re-pointed.
    pub(super) failed: Vec<String>,
    /// Sheets that could not be read, so their references are unknown.
    pub(super) unchecked: Vec<String>,
}

impl RewriteOutcome {
    pub(super) fn is_clean(&self) -> bool {
        self.failed.is_empty() && self.unchecked.is_empty()
    }
}

/// Detail text for the "Rename Incomplete" card: the file moved, but some
/// parent references were not re-pointed (`failed`, known stale) or could
/// not be checked at all (`unchecked`, unknown).
pub(super) fn rename_incomplete_detail(failed: &[String], unchecked: &[String]) -> String {
    let mut parts = Vec::new();
    if !failed.is_empty() {
        parts.push(format!(
            "The file was renamed, but {} sheet reference(s) could not be updated \
             and still point at the old name:\n  {}",
            failed.len(),
            failed.join("\n  ")
        ));
    }
    if !unchecked.is_empty() {
        parts.push(format!(
            "The file was renamed, but {} sheet(s) could not be read, so it is unknown \
             whether they reference the old name:\n  {}",
            unchecked.len(),
            unchecked.join("\n  ")
        ));
    }
    parts.join("\n\n")
}

impl Signex {
    /// Every parent reference, across the owning project's sheets, that
    /// resolves to `old_path`. Must run BEFORE the file is renamed on disk so
    /// the hierarchy walk still reaches the sheet's own subtree.
    pub(super) fn plan_reference_rewrites(
        &self,
        project_idx: usize,
        old_path: &Path,
        new_name: &str,
    ) -> ReferencePlan {
        let Some(project) = self.document_state.projects.get(project_idx) else {
            return ReferencePlan::default();
        };
        let dir = project.dir();
        let pages: Vec<PathBuf> = project
            .data
            .sheets
            .iter()
            .map(|s| dir.join(&s.filename))
            .collect();
        let root = project_root_sheet_path(project).unwrap_or_else(|| old_path.to_path_buf());
        let set = assemble_project_sheets(&self.document_state, &pages, &root);
        let old_key = path_key(old_path);

        let mut plan = Vec::new();
        for (parent, sheet) in &set.sheets {
            if path_key(parent) == old_key {
                continue;
            }
            let parent_dir = parent.parent().unwrap_or_else(|| Path::new(""));
            for child in &sheet.child_sheets {
                if path_key(&parent_dir.join(&child.filename)) != old_key {
                    continue;
                }
                let rewrite = ReferenceRewrite {
                    parent: parent.clone(),
                    from: child.filename.clone(),
                    to: with_new_leaf(&child.filename, new_name),
                };
                plan.push(rewrite);
            }
        }
        // Sort first so the dedupe is a single pass and the result order
        // does not depend on the hierarchy walk.
        plan.sort();
        plan.dedup();
        let mut unchecked: Vec<String> = set
            .unreadable
            .iter()
            .map(|(path, why)| {
                format!(
                    "{} — could not be read, so its references to this sheet were not checked: {why}",
                    path.display()
                )
            })
            .collect();
        unchecked.sort();
        ReferencePlan {
            rewrites: plan,
            unchecked,
        }
    }

    /// Apply the planned rewrites through the engine, marking each touched
    /// parent dirty. A parent that is not open is loaded into an engine (no
    /// tab) so Save All can persist it. Returns the parents that could not be
    /// updated and those that could not be checked; each is also logged.
    pub(super) fn apply_reference_rewrites(&mut self, plan: &ReferencePlan) -> RewriteOutcome {
        let mut failures = Vec::new();
        for rewrite in &plan.rewrites {
            if let Err(why) = self.apply_one_rewrite(rewrite) {
                failures.push(format!("{} — {why}", rewrite.parent.display()));
            }
        }
        // The modal card is dismissible; the Messages panel keeps every
        // dangling or unverified reference visible after it is gone.
        for line in failures.iter().chain(&plan.unchecked) {
            crate::diagnostics::log_error(
                "rename: sheet reference not updated",
                &anyhow::anyhow!("{line}"),
            );
        }
        RewriteOutcome {
            failed: failures,
            unchecked: plan.unchecked.clone(),
        }
    }

    fn apply_one_rewrite(&mut self, rewrite: &ReferenceRewrite) -> Result<(), String> {
        let parent = &rewrite.parent;
        if !self.document_state.engines.contains_key(parent) {
            let text =
                std::fs::read_to_string(parent).map_err(|e| format!("could not be read: {e}"))?;
            let sheet = signex_types::format::SnxSchematic::parse(&text)
                .map_err(|e| format!("could not be parsed: {e}"))?
                .sheet;
            let engine = signex_engine::Engine::new_with_path(sheet, Some(parent.clone()))
                .map_err(|e| format!("could not be loaded: {e}"))?;
            self.document_state.engines.insert(parent.clone(), engine);
        }
        let Some(engine) = self.document_state.engines.get_mut(parent) else {
            return Err("no engine".to_string());
        };
        let result = engine
            .execute(signex_engine::Command::RenameChildSheetFile {
                from: rewrite.from.clone(),
                to: rewrite.to.clone(),
            })
            .map_err(|e| format!("reference update failed: {e}"))?;
        if result.changed {
            self.document_state.dirty_paths.insert(parent.clone());
            for tab in self
                .document_state
                .tabs
                .iter_mut()
                .filter(|t| t.path == *parent)
            {
                tab.dirty = true;
            }
            if self.document_state.active_path.as_ref() == Some(parent) {
                self.sync_canvas_from_visible_schematic(
                    crate::schematic_runtime::RenderInvalidation::FULL,
                );
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{rename_incomplete_detail, with_new_leaf};

    #[test]
    fn rename_incomplete_detail_is_clean_text() {
        let detail = rename_incomplete_detail(
            &["a.snxsch — x".to_string(), "b.snxsch — y".to_string()],
            &[],
        );
        assert_eq!(
            detail,
            "The file was renamed, but 2 sheet reference(s) could not be updated \
             and still point at the old name:\n  a.snxsch — x\n  b.snxsch — y"
        );
        let first_line = detail.lines().next().expect("first line");
        assert!(
            !first_line.contains("  "),
            "no runs of spaces: {first_line}"
        );
    }

    #[test]
    fn unchecked_sheets_get_their_own_wording() {
        let detail = rename_incomplete_detail(&[], &["c.snxsch — bad".to_string()]);
        assert_eq!(
            detail,
            "The file was renamed, but 1 sheet(s) could not be read, so it is unknown \
             whether they reference the old name:\n  c.snxsch — bad"
        );
        assert!(!detail.contains("still point at the old name"));
    }

    #[test]
    fn leaf_replacement_keeps_directory_prefix() {
        assert_eq!(with_new_leaf("a.snxsch", "b.snxsch"), "b.snxsch");
        assert_eq!(with_new_leaf("sub/a.snxsch", "b.snxsch"), "sub/b.snxsch");
        assert_eq!(with_new_leaf("sub\\a.snxsch", "b.snxsch"), "sub\\b.snxsch");
    }
}
