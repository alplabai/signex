//! Exclude-from-project on a document with unsaved edits (#738).
//!
//! The Remove modal's second stage: Save / Discard / Cancel. Cancel is
//! `RemoveMsg::Close`, so Esc and the close button need no extra wiring.
//! The stage covers the target itself and every dirty document under it (a
//! `.snxlib` row or a folder), and Discard is withheld while another open
//! project still uses the target, because engines and editors are keyed by
//! path alone and a discard would wipe that project's edits too.

use super::*;
use crate::app::RemoveDialogState;
use crate::app::state::scope::{CaseFold, KeyMap, path_is_under, path_key_with};

/// Why the unsaved-edits prompt withholds Discard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DiscardBlock {
    /// Another open project reaches the target, or lists a library holding it.
    Shared,
    /// Another open project has sheets that are not loaded, so whether it
    /// reaches the target cannot be told without reading disk.
    Unknown,
}

impl DiscardBlock {
    fn prompt_reason(self) -> &'static str {
        match self {
            Self::Shared => "Another open project also uses it, so the edits cannot be discarded.",
            Self::Unknown => {
                "Another open project has sheets that are not loaded, so it cannot be ruled out \
                 that it uses this, and the edits cannot be discarded."
            }
        }
    }

    fn log_reason(self) -> &'static str {
        match self {
            Self::Shared => "another open project also uses it.",
            Self::Unknown => {
                "another open project has sheets that are not loaded, so shared use cannot be \
                 ruled out."
            }
        }
    }
}

/// How many document names the prompt lists before saying "and N more".
const PROMPT_NAMES_SHOWN: usize = 3;

impl Signex {
    /// Dirty documents at or under `target`, sorted. Pure: callers that need
    /// a footprint draft's flag folded into `dirty_paths` run
    /// `sync_footprint_dirty_paths` first.
    pub(crate) fn unsaved_documents(&self, target: &std::path::Path) -> Vec<std::path::PathBuf> {
        let fold = CaseFold::host_default();
        let doc = &self.document_state;
        let mut found: Vec<std::path::PathBuf> = doc
            .dirty_paths
            .iter()
            .filter(|p| path_is_under(p, target, fold))
            // A project file's own dirty marker is metadata, not a document
            // the user can lose by excluding a row.
            .filter(|p| {
                let key = path_key_with(p, fold);
                !doc.projects
                    .iter()
                    .any(|lp| path_key_with(&lp.path, fold) == key)
            })
            .cloned()
            .collect();
        found.sort();
        found
    }

    /// Does `target`, or anything under it, still hold unsaved edits?
    pub(crate) fn has_unsaved_edits(&self, target: &std::path::Path) -> bool {
        !self.unsaved_documents(target).is_empty()
    }

    /// Does another open project list a `.snxlib` that is `path`, lies under
    /// it, or contains it? The ownership walk only sees sheets, so library
    /// entries are checked here, each resolved against its own project's
    /// directory. Both sides are normalised lexically (`..` and `.` folded,
    /// no IO) and compared under the prompt's case rule, so `b/../Lib.snxlib`
    /// and `Lib.snxlib` are one library.
    fn library_listed_by_other_project(&self, owner: usize, path: &std::path::Path) -> bool {
        let fold = CaseFold::host_default();
        let keys = KeyMap::lexical(fold);
        let target = std::path::PathBuf::from(keys.lexical_key(path));
        self.document_state
            .projects
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != owner)
            .flat_map(|(_, p)| {
                p.data
                    .libraries
                    .iter()
                    .map(|entry| p.data.resolve_library_path(entry))
            })
            .map(|lib| std::path::PathBuf::from(keys.lexical_key(&lib)))
            .any(|lib| lib.starts_with(&target) || target.starts_with(&lib))
    }

    /// Why Discard is withheld for this modal, if it is: another open project
    /// uses the target (or an unsaved document under it), or the other
    /// projects' sheets are not all loaded so what they reach is unknown.
    ///
    /// The ownership walk is built from the loaded engines only. Reading the
    /// other projects' sheets from disk would block `update`, and the Remove
    /// modal has no resolved `SheetRefs` to reuse, so an incomplete walk is
    /// treated as shared.
    pub(crate) fn discard_block(&self, state: &RemoveDialogState) -> Option<DiscardBlock> {
        let owner = state.tree_path.first().copied()?;
        let reach = self.document_state.lexical_reach_sets();
        let docs = self.unsaved_documents(&state.target_path);
        let shared = |p: &std::path::Path| {
            reach.reached_under_by_other(owner, p) || self.library_listed_by_other_project(owner, p)
        };
        if shared(&state.target_path) || docs.iter().any(|p| shared(p)) {
            return Some(DiscardBlock::Shared);
        }
        reach
            .other_walk_incomplete(owner)
            .then_some(DiscardBlock::Unknown)
    }

    /// Is Discard withheld for this modal?
    pub(crate) fn discard_blocked_by_other_project(&self, state: &RemoveDialogState) -> bool {
        self.discard_block(state).is_some()
    }

    /// Body text of the unsaved-edits prompt, and whether Discard is offered.
    pub(crate) fn exclude_dirty_prompt(&self, state: &RemoveDialogState) -> (String, bool) {
        let docs = self.unsaved_documents(&state.target_path);
        let name = &state.display_name;
        let mut body = if docs.len() <= 1 {
            format!("\"{name}\" has unsaved edits.")
        } else {
            let names: Vec<String> = docs
                .iter()
                .take(PROMPT_NAMES_SHOWN)
                .map(|p| {
                    p.file_name()
                        .map(|f| f.to_string_lossy().into_owned())
                        .unwrap_or_else(|| p.display().to_string())
                })
                .collect();
            let more = docs.len().saturating_sub(PROMPT_NAMES_SHOWN);
            let tail = if more > 0 {
                format!(" and {more} more")
            } else {
                String::new()
            };
            format!(
                "\"{name}\" holds {} documents with unsaved edits: {}{tail}.",
                docs.len(),
                names.join(", ")
            )
        };
        let block = self.discard_block(state);
        match block {
            None => body.push_str(" Save them before excluding it from the project?"),
            Some(why) => {
                body.push(' ');
                body.push_str(why.prompt_reason());
                body.push_str(" Save them to exclude it, or Cancel.");
            }
        }
        (body, block.is_none())
    }

    /// Resolve the Save / Discard choice of the unsaved-edits stage.
    pub(crate) fn handle_exclude_dirty_choice(
        &mut self,
        choice: crate::app::ExcludeDirtyChoice,
    ) -> iced::Task<Message> {
        use crate::app::{ExcludeDirtyChoice, RemoveChoice};

        let Some(state) = self.ui_state.remove_dialog.take() else {
            return iced::Task::none();
        };
        if !state.unsaved_prompt {
            self.ui_state.remove_dialog = Some(state);
            return iced::Task::none();
        }
        self.sync_footprint_dirty_paths();
        let docs = self.unsaved_documents(&state.target_path);
        match choice {
            ExcludeDirtyChoice::Save => {
                for doc_path in &docs {
                    if let Err(err) = self.try_save_dirty_path(doc_path) {
                        self.report_exclude_save_failure(&state, doc_path, &err);
                        // Keep the prompt so the user can retry or cancel.
                        self.ui_state.remove_dialog = Some(state);
                        self.refresh_panel_ctx();
                        return iced::Task::none();
                    }
                }
                self.apply_remove(&state, RemoveChoice::ExcludeFromProject)
            }
            ExcludeDirtyChoice::Discard => {
                if let Some(why) = self.discard_block(&state) {
                    crate::diagnostics::log_warning(format!(
                        "Cannot discard the unsaved edits of {}: {} Save them or cancel.",
                        state.display_name,
                        why.log_reason()
                    ));
                    self.ui_state.remove_dialog = Some(state);
                    self.refresh_panel_ctx();
                    return iced::Task::none();
                }
                let fold = CaseFold::host_default();
                let mut tasks: Vec<iced::Task<Message>> = docs
                    .iter()
                    .flat_map(|p| self.close_tabs_of_removed(p, false, fold))
                    .collect();
                tasks.push(self.apply_remove(&state, RemoveChoice::ExcludeFromProject));
                self.discard_unsaved_edits(&docs, &state.display_name);
                // The discard was logged after `apply_remove` refreshed the panels.
                self.refresh_panel_ctx();
                iced::Task::batch(tasks)
            }
        }
    }

    fn report_exclude_save_failure(
        &mut self,
        state: &RemoveDialogState,
        failed: &std::path::Path,
        err: &anyhow::Error,
    ) {
        let name = failed
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_else(|| state.display_name.clone());
        crate::diagnostics::log_error(&format!("Exclude from Project: could not save {name}"), err);
        self.document_state.error_notice = Some(crate::app::state::ErrorNotice::save(format!(
            "Could not save \"{name}\", so \"{}\" was not excluded: {err:#}",
            state.display_name
        )));
    }

    /// Drop the engines / editors and dirty markers held for `docs`, so a
    /// later Save All writes nothing for them.
    fn discard_unsaved_edits(&mut self, docs: &[std::path::PathBuf], display_name: &str) {
        let fold = CaseFold::host_default();
        let keys: std::collections::HashSet<String> =
            docs.iter().map(|p| path_key_with(p, fold)).collect();
        let doc = &mut self.document_state;
        let is_discarded = |p: &std::path::PathBuf| keys.contains(&path_key_with(p, fold));
        doc.engines.retain(|p, _| !is_discarded(p));
        doc.symbol_editors.retain(|p, _| !is_discarded(p));
        doc.footprint_editors.retain(|p, _| !is_discarded(p));
        doc.dirty_paths.retain(|p| !is_discarded(p));
        crate::diagnostics::log_warning(format!(
            "Excluded {display_name} from the project: discarded {} unsaved document(s)",
            docs.len()
        ));
    }
}
