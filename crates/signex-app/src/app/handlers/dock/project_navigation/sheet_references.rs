//! Keeping hierarchical sheet references in step with a renamed `.snxsch`.
//!
//! Renaming a child sheet's file leaves every parent's `ChildSheet.filename`
//! pointing at a path that no longer exists. The fix goes through the engine
//! (`Command::RenameChildSheetFile`) so it is undoable and dirty-tracked; the
//! saved parents are re-pointed on disk by the same job that moved the file
//! (`parent_reference_disk`, #1849); an open parent follows in memory, and is
//! dirtied only when it already held other unsaved edits or the disk write was
//! skipped.

use std::path::{Path, PathBuf};

use super::*;
use crate::app::parent_reference_disk::{ParentDisk, ParentDiskOutcomes};
use crate::app::project_sheets::{assemble_project_sheets, project_root_sheet_path};
use crate::app::state::library_entry::with_new_leaf;
use crate::app::state::scope::path_key_with;

pub(super) use crate::app::path_ops::{ParentSheets, ReferencePlan, ReferenceRewrite};

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

/// What a Save As left unchanged among the other sheets.
#[derive(Debug, Default)]
pub(crate) struct SaveAsParentsLeft {
    /// Parents whose reference to the old file was not re-pointed.
    pub(crate) not_rewritten: Vec<String>,
    /// Sheets that could not be read, so their references are unknown.
    pub(crate) unchecked: Vec<String>,
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
    pub(crate) fn plan_reference_rewrites(
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
    pub(super) fn apply_reference_rewrites(
        &mut self,
        plan: &ReferencePlan,
        parents: ParentSheets,
        disk: &ParentDiskOutcomes,
    ) -> RewriteOutcome {
        self.apply_reference_rewrites_for(plan, parents, disk, "rename")
    }

    /// [`Self::apply_reference_rewrites`] logging failures under `context`
    /// (the operation that planned them).
    fn apply_reference_rewrites_for(
        &mut self,
        plan: &ReferencePlan,
        mut parents: ParentSheets,
        disk: &ParentDiskOutcomes,
        context: &str,
    ) -> RewriteOutcome {
        self.record_written_parent_baselines(disk);
        let mut failures = Vec::new();
        for rewrite in &plan.rewrites {
            let on_disk = disk
                .get(&rewrite.parent)
                .is_some_and(ParentDisk::is_written);
            if let Some(ParentDisk::Skipped(why) | ParentDisk::Unchecked(why)) =
                disk.get(&rewrite.parent)
            {
                crate::diagnostics::log_warning(format!(
                    "{context}: {} {why}, so its saved reference was not updated; save it to keep the change.",
                    rewrite.parent.display()
                ));
            }
            if let Err(why) = self.apply_one_rewrite(rewrite, &mut parents, on_disk) {
                // The disk holds the new reference but memory does not: the
                // two differ, so the parent has something to save.
                if on_disk && self.document_state.engines.contains_key(&rewrite.parent) {
                    self.document_state.mark_document_dirty(&rewrite.parent);
                }
                failures.push(format!("{} — {why}", rewrite.parent.display()));
            }
        }
        self.settle_unplanned_parents(plan, disk, context);
        // The modal card is dismissible; the Messages panel keeps every
        // dangling or unverified reference visible after it is gone.
        for line in failures.iter().chain(&plan.unchecked) {
            crate::diagnostics::log_error(
                &format!("{context}: sheet reference not updated"),
                &anyhow::anyhow!("{line}"),
            );
        }
        RewriteOutcome {
            failed: failures,
            unchecked: plan.unchecked.clone(),
        }
    }

    /// Is an open of `path` still reading it? Compared case-folded whatever the
    /// volume does: a spelling that differs only in case can only make this
    /// refuse more, never miss an open the volume would treat as the same file.
    pub(crate) fn open_in_flight(&self, path: &Path) -> bool {
        let key = path_key_with(path, crate::app::state::scope::CaseFold::from_probe(true));
        self.document_state.pending_opens.paths().any(|open| {
            path_key_with(open, crate::app::state::scope::CaseFold::from_probe(true)) == key
        })
    }

    /// Parents the disk job scanned that the plan did not name (a reference
    /// that exists only in the saved file). One that was rewritten differs from
    /// an open engine that still holds the old state, so it gets the external
    /// dirty mark (undo back to the save point must not read clean); one that
    /// could not be rewritten is reported.
    fn settle_unplanned_parents(
        &mut self,
        plan: &ReferencePlan,
        disk: &ParentDiskOutcomes,
        context: &str,
    ) {
        let mut outcomes: Vec<_> = disk.iter().collect();
        outcomes.sort_by_key(|(parent, _)| *parent);
        for (parent, outcome) in outcomes {
            if plan.rewrites.iter().any(|r| &r.parent == parent) {
                continue;
            }
            let open = self.document_state.engines.contains_key(parent);
            match outcome {
                ParentDisk::Written(..) if open => {
                    self.document_state.mark_document_dirty(parent);
                    crate::diagnostics::log_warning(format!(
                        "{context}: {} names the renamed sheet only in its saved file; the \
                         saved copy now names the new file, so undoing the unsaved change in \
                         the open sheet would bring the old name back.",
                        parent.display()
                    ));
                }
                ParentDisk::Skipped(why) => crate::diagnostics::log_warning(format!(
                    "{context}: {} {why}, so its saved reference was not updated.",
                    parent.display()
                )),
                ParentDisk::Unchecked(why) if open => crate::diagnostics::log_warning(format!(
                    "{context}: {} {why}, so whether it references the sheet is unknown.",
                    parent.display()
                )),
                _ => {}
            }
        }
    }

    /// The saved parents the disk job scans for references to `from`: those the
    /// plan rewrites plus every page of the owning project, since a symbol
    /// deleted in memory but not yet saved still names the file on disk. A
    /// parent another move or delete holds, or whose open is in flight, is left
    /// out; its reference follows in memory (dirty) instead.
    pub(crate) fn disk_parent_candidates(
        &self,
        owner: Option<crate::app::state::ProjectId>,
        plan: &ReferencePlan,
        from: &Path,
    ) -> Vec<PathBuf> {
        let is_sheet = |path: &Path| crate::app::file_extension::has_extension(path, "snxsch");
        // Only a renamed sheet can be placed by a sheet symbol.
        if !is_sheet(from) {
            return Vec::new();
        }
        let mut candidates: Vec<PathBuf> = plan
            .rewrites
            .iter()
            .map(|rewrite| rewrite.parent.clone())
            .collect();
        if let Some(project) = owner.and_then(|id| self.document_state.project_by_id(id)) {
            let fold = project.case_fold;
            candidates.extend(
                project
                    .data
                    .sheets
                    .iter()
                    .map(|s| project.dir().join(&s.filename))
                    .filter(|page| is_sheet(page))
                    .filter(|page| path_key_with(page, fold) != path_key_with(from, fold)),
            );
        }
        candidates.sort();
        candidates.dedup();
        // A parent another move or delete holds, or one whose open is still in
        // flight (it read the old bytes), follows in memory instead.
        candidates.retain(|parent| {
            self.document_state
                .pending_path_ops
                .blocking_start(parent)
                .is_none()
                && !self.open_in_flight(parent)
        });
        candidates
    }

    /// Commit every parent the disk job rewrote to the project's git history,
    /// as the project file's own patch is.
    pub(crate) fn commit_written_parents(&mut self, disk: &ParentDiskOutcomes, message: &str) {
        let mut written: Vec<_> = disk
            .iter()
            .filter_map(|(path, outcome)| match outcome {
                ParentDisk::Written(bytes, _) => Some((path.clone(), bytes.clone())),
                _ => None,
            })
            .collect();
        if written.is_empty() {
            return;
        }
        // A background read of these sheets is out of date.
        self.document_state.note_document_written_behind_engines();
        written.sort();
        for (path, bytes) in written {
            self.commit_bytes_to_project_git(
                &path,
                message,
                crate::app::state::CommitBytes { bytes, sequence: 0 },
            );
        }
    }

    /// The parents a plan will rewrite that no engine holds open: the rename
    /// worker reads them so the completion never touches the disk.
    pub(super) fn unopened_rewrite_parents(&self, plan: &ReferencePlan) -> Vec<PathBuf> {
        let mut parents: Vec<PathBuf> = plan
            .rewrites
            .iter()
            .map(|rewrite| rewrite.parent.clone())
            .filter(|parent| !self.document_state.engines.contains_key(parent))
            .collect();
        parents.sort();
        parents.dedup();
        parents
    }

    /// After a Save As moved the sheet `from` to `to` (same folder): re-point
    /// the references of every parent that is open, through the engine (dirty,
    /// undoable). Parents that are not open are not rewritten, and sheets that
    /// could not be read are not checked; both are returned for the caller to
    /// report.
    pub(crate) fn repoint_open_parents_after_save_as(
        &mut self,
        project_idx: usize,
        from: &Path,
        to: &Path,
    ) -> SaveAsParentsLeft {
        let leaf = to
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        let mut plan = self.plan_reference_rewrites(project_idx, from, &leaf);
        let mut left = SaveAsParentsLeft {
            unchecked: std::mem::take(&mut plan.unchecked),
            ..Default::default()
        };
        plan.rewrites.retain(|rewrite| {
            if self.document_state.engines.contains_key(&rewrite.parent) {
                return true;
            }
            left.not_rewritten
                .push(rewrite.parent.file_name().map_or_else(
                    || rewrite.parent.display().to_string(),
                    |n| n.to_string_lossy().into_owned(),
                ));
            false
        });
        let outcome = self.apply_reference_rewrites_for(
            &plan,
            ParentSheets::new(),
            &ParentDiskOutcomes::new(),
            "save-as",
        );
        left.not_rewritten.extend(outcome.failed);
        left.not_rewritten.sort();
        left.not_rewritten.dedup();
        left
    }

    /// After a Save As the saved project lists the new file: follow the plan
    /// the worker applied to the saved parents. Open parents follow in memory
    /// (dirty only for other unsaved edits); a parent that is neither open nor
    /// rewritten on disk is returned for the caller to report.
    pub(crate) fn follow_save_as_parents(
        &mut self,
        plan: &ReferencePlan,
        disk: &ParentDiskOutcomes,
    ) -> SaveAsParentsLeft {
        let mut left = SaveAsParentsLeft {
            unchecked: plan.unchecked.clone(),
            ..Default::default()
        };
        let mut followed = ReferencePlan::default();
        for rewrite in &plan.rewrites {
            let written = disk
                .get(&rewrite.parent)
                .is_some_and(ParentDisk::is_written);
            if written || self.document_state.engines.contains_key(&rewrite.parent) {
                followed.rewrites.push(rewrite.clone());
            } else {
                left.not_rewritten
                    .push(rewrite.parent.file_name().map_or_else(
                        || rewrite.parent.display().to_string(),
                        |n| n.to_string_lossy().into_owned(),
                    ));
            }
        }
        // A planned parent left behind is reported once, through `left`.
        let mut disk = disk.clone();
        for rewrite in &plan.rewrites {
            if !followed.rewrites.contains(rewrite) {
                disk.remove(&rewrite.parent);
            }
        }
        let outcome =
            self.apply_reference_rewrites_for(&followed, ParentSheets::new(), &disk, "save-as");
        left.not_rewritten.extend(outcome.failed);
        left.not_rewritten.sort();
        left.not_rewritten.dedup();
        left
    }

    fn apply_one_rewrite(
        &mut self,
        rewrite: &ReferenceRewrite,
        parents: &mut ParentSheets,
        on_disk: bool,
    ) -> Result<(), String> {
        let parent = &rewrite.parent;
        let loaded_here = !self.document_state.engines.contains_key(parent);
        // An open of this parent that started before the rename is still in
        // flight: it read the old bytes and would install them as a clean
        // engine, whatever the disk now holds.
        if loaded_here && self.open_in_flight(parent) {
            return Err("is still opening, so its reference was not updated".to_string());
        }
        // The saved parent already names the new file and no engine holds it:
        // there is nothing in memory to follow.
        if loaded_here && on_disk {
            return Ok(());
        }
        let was_dirty = self.document_state.is_document_dirty(parent);
        if loaded_here {
            // Read by the rename worker; a parent that was not in the plan's
            // read list (an engine closed meanwhile) is reported, not read here.
            let sheet = parents
                .remove(parent)
                .ok_or_else(|| "was not read before the rename".to_string())??;
            let engine = signex_engine::Engine::new_with_path(*sheet, Some(parent.clone()))
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
            // The disk already holds the re-pointed reference, so a parent
            // with no other unsaved edits is exactly what is saved; one with
            // other edits stays dirty and Discard All reverts only those.
            if on_disk && !was_dirty {
                self.document_state.mark_document_clean(parent);
            } else {
                self.document_state.mark_document_dirty(parent);
            }
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

        // The rename worker reads the parent; the completion only consumes it.
        let sheet = signex_types::format::SnxSchematic::parse(
            &std::fs::read_to_string(&parent).expect("read parent"),
        )
        .expect("parse parent")
        .sheet;
        let mut parents = std::collections::HashMap::from([(parent.clone(), Ok(Box::new(sheet)))]);

        let outcome = app.apply_one_rewrite(
            &ReferenceRewrite {
                parent: parent.clone(),
                from: "Missing.snxsch".to_string(),
                to: "Other.snxsch".to_string(),
            },
            &mut parents,
            false,
        );

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
        let outcome = app.apply_reference_rewrites(
            &super::ReferencePlan {
                rewrites: vec![ReferenceRewrite {
                    parent: parent.clone(),
                    from: "child.snxsch".to_string(),
                    to: "renamed.snxsch".to_string(),
                }],
                unchecked: Vec::new(),
                read_only_parents: Vec::new(),
            },
            Default::default(),
            &Default::default(),
        );
        assert_eq!(outcome.failed.len(), 1, "{outcome:?}");
        assert!(!app.document_state.is_document_dirty(&parent));
    }

    /// A parent the disk already names the new file for, whose in-memory
    /// rewrite is refused, differs from memory: it is dirty.
    #[test]
    fn a_failed_in_memory_rewrite_of_a_written_parent_marks_it_dirty() {
        use crate::app::handlers::menu::export::tests as fx;
        let parent = std::path::PathBuf::from("/w/ro-parent2/Parent.snxsch");
        let mut sheet = fx::sheet_with_net("R_TOP", "TOP_NET", &["child.snxsch"]);
        sheet.unknown.newer_minor_source = Some("snxsch/1.7".into());
        let (mut app, _boot) = Signex::new();
        fx::open_with(&mut app.document_state, &parent, sheet);
        app.document_state.mark_document_clean(&parent);
        let disk = std::collections::HashMap::from([(
            parent.clone(),
            crate::app::parent_reference_disk::ParentDisk::Written(
                std::sync::Arc::new(vec![]),
                crate::persist::ContentHash::of(b""),
            ),
        )]);
        let outcome = app.apply_reference_rewrites(
            &super::ReferencePlan {
                rewrites: vec![ReferenceRewrite {
                    parent: parent.clone(),
                    from: "child.snxsch".to_string(),
                    to: "renamed.snxsch".to_string(),
                }],
                unchecked: Vec::new(),
                read_only_parents: Vec::new(),
            },
            Default::default(),
            &disk,
        );
        assert_eq!(outcome.failed.len(), 1, "{outcome:?}");
        assert!(app.document_state.is_document_dirty(&parent));
    }
}
