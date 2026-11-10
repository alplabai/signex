//! Apply: every manual number the dialog changed, written into its parent
//! sheet's sheet symbol, as ONE undo group in ONE `update`.
//!
//! Data-loss rules (the project Replace's, `handlers/find_replace`):
//! - a parent sheet no engine holds is loaded into a tabless engine and left
//!   unsaved; nothing is written to disk;
//! - each symbol's entries are re-read from the live document, so only the
//!   one entry that numbers the page changes;
//! - a stale read, a sheet being saved or moved, or a read-only sheet refuses
//!   the whole Apply, and a failure part-way puts every sheet back.

use std::collections::BTreeMap;
use std::path::PathBuf;

use iced::Task;
use signex_engine::Command;
use signex_types::schematic::InstancePath;
use uuid::Uuid;

use super::state::PageEdit;
use crate::app::{Message, Signex};

const LABEL: &str = "Number Schematic Sheets";

/// One sheet symbol's new number: which placement, and the page or `None`.
type SymbolEdit = (Uuid, InstancePath, Option<usize>);

/// A parent sheet Apply changed, before the run is committed.
struct Changed {
    path: PathBuf,
    tabless: bool,
    invalidation: Option<crate::schematic_runtime::RenderInvalidation>,
}

impl Signex {
    /// Apply the dialog's numbers. On success the dialog closes; on a refusal
    /// it stays open and says why.
    pub(super) fn apply_sheet_numbers(&mut self) -> Task<Message> {
        let Some(state) = self.ui_state.sheet_numbering.as_ref() else {
            return Task::none();
        };
        if !state.can_apply() {
            return Task::none();
        }
        let Some(read) = state.read.clone() else {
            return Task::none();
        };
        let project_name = state.project_name.clone();
        let stale = state.epoch != self.document_state.documents_epoch
            || state.child_sheets != self.child_sheet_signature();
        if stale {
            return self.refuse_sheet_numbers(
                "The sheets changed while this dialog was open. Close it and open it again.",
            );
        }
        let edits = by_parent(&read.rows, &state.edits());
        match self.write_sheet_numbers(&read, &project_name, edits) {
            Ok(count) => {
                crate::diagnostics::log_info(format!(
                    "{LABEL}: renumbered pages on {count} sheet(s); save to keep them"
                ));
                self.ui_state.sheet_numbering = None;
            }
            Err(why) => return self.refuse_sheet_numbers(&why),
        }
        Task::none()
    }

    fn refuse_sheet_numbers(&mut self, why: &str) -> Task<Message> {
        crate::diagnostics::log_warning(format!("{LABEL}: nothing was changed. {why}"));
        if let Some(state) = self.ui_state.sheet_numbering.as_mut() {
            state.error = Some(why.to_string());
        }
        Task::none()
    }

    /// Write `edits` (per parent sheet), all or nothing. The number of sheets
    /// changed, or why nothing was.
    fn write_sheet_numbers(
        &mut self,
        read: &super::NumberingRead,
        project_name: &str,
        edits: BTreeMap<PathBuf, Vec<SymbolEdit>>,
    ) -> Result<usize, String> {
        // Every check before the first change: a refusal changes nothing.
        let mut targets = Vec::with_capacity(edits.len());
        for (listed, list) in edits {
            let path = self.resolve_held_spelling(listed.clone());
            let tabless = !self.document_state.engines.contains_key(&path);
            if tabless
                && (self.document_state.document_saves.is_saving(&path)
                    || self.refuse_while_path_op(&path, "renumbered").is_err())
            {
                return Err(format!("{} is being saved or moved.", path.display()));
            }
            if let Some(engine) = self.document_state.engines.get(&path)
                && engine.document().unknown.newer_minor_source.is_some()
            {
                return Err(format!(
                    "{} is read-only (written by a newer Signex).",
                    path.display()
                ));
            }
            targets.push((listed, path, tabless, list));
        }
        let group = self.mint_undo_group();
        let mut changed: Vec<Changed> = Vec::new();
        for (listed, path, tabless, list) in targets {
            if tabless && !self.install_tabless_engine(LABEL, &path, read.loaded.get(&listed)) {
                self.roll_back_sheet_numbers(group, &changed);
                return Err(format!("{} could not be loaded.", path.display()));
            }
            let Some(engine) = self.document_state.engines.get_mut(&path) else {
                continue;
            };
            let commands = number_commands(engine.document(), project_name, &list);
            match engine.execute_batch_grouped(commands, Some(group)) {
                Ok(result) if result.changed => changed.push(Changed {
                    path,
                    tabless,
                    invalidation: result
                        .patch_pair
                        .map(|pair| Self::render_invalidation_for_patch(pair.document)),
                }),
                Ok(_) => self.drop_unchanged_tabless(&path, tabless),
                Err(error) => {
                    crate::diagnostics::log_error(
                        &format!("{LABEL} failed on '{}'", path.display()),
                        &anyhow::Error::new(error),
                    );
                    self.drop_unchanged_tabless(&path, tabless);
                    self.roll_back_sheet_numbers(group, &changed);
                    return Err(format!("{} could not be changed.", path.display()));
                }
            }
        }
        let count = changed.len();
        self.commit_sheet_numbers(group, changed);
        Ok(count)
    }

    /// Put back every sheet already changed: pop the group's entry and drop
    /// the engines this run loaded.
    fn roll_back_sheet_numbers(&mut self, group: signex_engine::UndoGroupId, changed: &[Changed]) {
        for done in changed {
            if let Some(engine) = self.document_state.engines.get_mut(&done.path)
                && let Err(error) = engine.discard_undo_group_top(group)
            {
                crate::diagnostics::log_error(
                    &format!("{LABEL}: could not put back '{}'", done.path.display()),
                    &anyhow::Error::new(error),
                );
            }
            self.drop_unchanged_tabless(&done.path, done.tabless);
        }
    }

    /// Every sheet changed: mark them unsaved, register the group and
    /// refresh the canvas.
    fn commit_sheet_numbers(&mut self, group: signex_engine::UndoGroupId, changed: Vec<Changed>) {
        let active = self.document_state.active_path.clone();
        let mut recorded = crate::app::UndoGroupSheets {
            kind: crate::app::UndoGroupKind::SheetNumbers,
            ..Default::default()
        };
        let mut active_invalidation = None;
        for done in &changed {
            recorded.live.push(done.path.clone());
            if done.tabless {
                recorded.parked.push(done.path.clone());
            }
            if active.as_deref() == Some(done.path.as_path()) {
                active_invalidation = done.invalidation;
            } else {
                self.document_state.mark_history_edit_dirty(&done.path);
            }
        }
        self.set_undo_group_sheets(group, recorded);
        if let Some(invalidation) = active_invalidation {
            self.finish_schematic_mutation(invalidation, true, true);
        }
        self.refresh_panel_ctx();
    }
}

/// The edits grouped by the parent sheet that stores them, in path order.
fn by_parent(
    rows: &[super::NumberingRow],
    edits: &[PageEdit],
) -> BTreeMap<PathBuf, Vec<SymbolEdit>> {
    let mut grouped: BTreeMap<PathBuf, Vec<SymbolEdit>> = BTreeMap::new();
    for edit in edits {
        let Some(placement) = rows.get(edit.row).and_then(|r| r.placement.as_ref()) else {
            continue;
        };
        grouped.entry(placement.parent.clone()).or_default().push((
            placement.symbol,
            placement.parent_path.clone(),
            edit.page,
        ));
    }
    grouped
}

/// One `SetChildSheetInstances` per sheet symbol `list` changes, each from
/// the symbol's live entries. A symbol placed in several parent placements
/// takes all of its edits in one command.
fn number_commands(
    document: &signex_types::schematic::SchematicSheet,
    project_name: &str,
    list: &[SymbolEdit],
) -> Vec<Command> {
    let mut entries: BTreeMap<Uuid, Vec<signex_types::schematic::SheetInstance>> = BTreeMap::new();
    for (symbol, parent_path, page) in list {
        let current = match entries.get(symbol) {
            Some(edited) => edited.clone(),
            None => match document.child_sheets.iter().find(|cs| cs.uuid == *symbol) {
                Some(cs) => cs.instances.clone(),
                None => continue,
            },
        };
        let next = signex_net::with_page_number(&current, parent_path, project_name, *page);
        entries.insert(*symbol, next);
    }
    entries
        .into_iter()
        .map(|(sheet_id, instances)| Command::SetChildSheetInstances {
            sheet_id,
            instances,
        })
        .collect()
}
