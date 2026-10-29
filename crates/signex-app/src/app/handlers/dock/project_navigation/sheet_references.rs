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
use crate::app::state::library_entry::with_new_leaf;
use crate::app::state::scope::path_key_with;

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
    /// Parents of a rewrite that are read-only (written by a newer Signex),
    /// so the engine would refuse the re-point. The rename must not start
    /// when this is non-empty: the file would move and the reference stay.
    pub(super) read_only_parents: Vec<PathBuf>,
}

/// The refusal text for a rename whose parents cannot be re-pointed.
pub(super) fn read_only_parents_detail(parents: &[PathBuf]) -> String {
    let names: Vec<String> = parents
        .iter()
        .map(|p| {
            p.file_name().map_or_else(
                || p.display().to_string(),
                |n| n.to_string_lossy().into_owned(),
            )
        })
        .collect();
    format!(
        "Not renamed: {} reference this sheet but are read-only (written by a newer Signex), so the reference cannot be updated. Edit them in that version first.",
        names.join(", ")
    )
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
        let set = assemble_project_sheets(&self.document_state, &pages, &root, project.case_fold);
        let fold = project.case_fold;
        let old_key = path_key_with(old_path, fold);

        let mut plan = Vec::new();
        let mut read_only_parents = Vec::new();
        for (parent, sheet) in &set.sheets {
            if path_key_with(parent, fold) == old_key {
                continue;
            }
            let parent_dir = parent.parent().unwrap_or_else(|| Path::new(""));
            for child in &sheet.child_sheets {
                if path_key_with(&parent_dir.join(&child.filename), fold) != old_key {
                    continue;
                }
                let rewrite = ReferenceRewrite {
                    parent: parent.clone(),
                    from: child.filename.clone(),
                    to: with_new_leaf(&child.filename, new_name),
                };
                plan.push(rewrite);
                if sheet.is_read_only() && !read_only_parents.contains(parent) {
                    read_only_parents.push(parent.clone());
                }
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
        read_only_parents.sort();
        ReferencePlan {
            rewrites: plan,
            unchecked,
            read_only_parents,
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
        let loaded_here = !self.document_state.engines.contains_key(parent);
        if loaded_here {
            let text =
                std::fs::read_to_string(parent).map_err(|e| format!("could not be read: {e}"))?;
            let sheet = signex_types::format::SnxSchematic::parse(&text)
                .map_err(|e| format!("could not be parsed: {e}"))?
                .sheet;
            let engine = signex_engine::Engine::new_with_path(sheet, Some(parent.clone()))
                .map_err(|e| format!("could not be loaded: {e}"))?;
            self.document_state.insert_engine(parent.clone(), engine);
            self.adopt_net_colors_of(parent);
            self.clear_net_color_steps(parent);
        }
        let Some(engine) = self.document_state.engines.get_mut(parent) else {
            return Err("no engine".to_string());
        };
        let result = engine
            .execute(signex_engine::Command::RenameChildSheetFile {
                from: rewrite.from.clone(),
                to: rewrite.to.clone(),
            })
            .map_err(|e| format!("reference update failed: {e}"));
        // An engine loaded only to be edited, left clean and tabless, would
        // own the path for the rest of the session and shadow a later open.
        if loaded_here && !matches!(&result, Ok(done) if done.changed) {
            self.forget_net_colors_of(parent);
            self.document_state.remove_engine(parent);
        }
        let result = result?;
        if result.changed {
            self.document_state.mark_document_dirty(parent);
            if self.document_state.active_path.as_ref() == Some(parent) {
                self.sync_canvas_from_visible_schematic(
                    crate::schematic_runtime::RenderInvalidation::CHILD_SHEETS,
                );
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{ReferenceRewrite, rename_incomplete_detail};
    use crate::app::Signex;
    use crate::app::state::library_entry::with_new_leaf;

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

    #[test]
    fn a_parent_loaded_for_an_unchanged_rewrite_is_not_left_in_engines() {
        let dir = tempfile::tempdir().expect("tempdir");
        let parent = dir.path().join("Parent.snxsch");
        let text = signex_types::format::SnxSchematic::new(Default::default())
            .write_string()
            .expect("serialise");
        std::fs::write(&parent, text).expect("write parent");
        let (mut app, _boot) = Signex::new();

        let outcome = app.apply_one_rewrite(&ReferenceRewrite {
            parent: parent.clone(),
            from: "Missing.snxsch".to_string(),
            to: "Other.snxsch".to_string(),
        });

        assert!(outcome.is_ok());
        assert!(
            !app.document_state.engines.contains_key(&parent),
            "a clean tabless engine would shadow every later open of the path"
        );
    }

    #[test]
    fn renaming_a_child_leaves_a_read_only_parent_clean() {
        use crate::app::handlers::menu::export::tests as fx;
        use crate::app::{Message, RenameDialogState, RenameMsg};
        let dir = tempfile::tempdir().expect("tempdir");
        let parent = dir.path().join("Parent.snxsch");
        let child = dir.path().join("child.snxsch");
        let text = signex_types::format::SnxSchematic::new(Default::default())
            .write_string()
            .expect("serialise");
        std::fs::write(&child, text).expect("write child");
        let mut app = fx::app_workspace(
            &dir.path().to_string_lossy(),
            &["Parent.snxsch", "child.snxsch"],
        );
        let mut sheet = fx::sheet_with_net("R_TOP", "TOP_NET", &["child.snxsch"]);
        sheet.unknown.newer_minor_source = Some("snxsch/1.7".into());
        fx::open_with(&mut app.document_state, &parent, sheet);

        app.ui_state.rename_dialog = Some(RenameDialogState {
            project_id: None,
            target_path: child.clone(),
            tree_path: vec![0],
            buffer: "renamed.snxsch".to_string(),
            error: None,
            is_project_rename: false,
        });
        let _task = app.update(Message::Rename(RenameMsg::Submit));

        assert!(child.exists(), "the file must not move");
        assert!(!dir.path().join("renamed.snxsch").exists());
        let notice = app
            .document_state
            .error_notice
            .as_ref()
            .expect("error card");
        assert!(
            format!("{notice:?}").contains("Parent.snxsch"),
            "{notice:?}"
        );
        assert!(!app.document_state.is_document_dirty(&parent));
        let engine = &app.document_state.engines[&parent];
        assert_eq!(engine.document().child_sheets[0].filename, "child.snxsch");
    }

    /// The engine's own refusal still reaches the failure list when a
    /// rewrite is applied directly.
    #[test]
    fn a_read_only_parent_rewrite_is_reported_not_applied() {
        use crate::app::handlers::menu::export::tests as fx;
        let parent = std::path::PathBuf::from("/w/ro-parent/Parent.snxsch");
        let mut sheet = fx::sheet_with_net("R_TOP", "TOP_NET", &["child.snxsch"]);
        sheet.unknown.newer_minor_source = Some("snxsch/1.7".into());
        let (mut app, _boot) = Signex::new();
        fx::open_with(&mut app.document_state, &parent, sheet);
        let outcome = app.apply_reference_rewrites(&super::ReferencePlan {
            rewrites: vec![ReferenceRewrite {
                parent: parent.clone(),
                from: "child.snxsch".to_string(),
                to: "renamed.snxsch".to_string(),
            }],
            unchecked: Vec::new(),
            read_only_parents: Vec::new(),
        });
        assert_eq!(outcome.failed.len(), 1, "{outcome:?}");
        assert!(!app.document_state.is_document_dirty(&parent));
    }
}
