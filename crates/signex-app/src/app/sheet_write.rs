//! Edits written into several sheets of a project as ONE undo group in ONE
//! `update`: Number Schematic Sheets and Back Annotate.
//!
//! Data-loss rules (the project Replace's, `handlers/find_replace`):
//! - a sheet no engine holds is loaded into a tabless engine from the copy
//!   the caller read, and left unsaved; nothing is written to disk;
//! - every check runs before the first change, so a refusal changes nothing;
//!   a sheet being saved or moved, or a read-only one, refuses the whole run;
//! - a failure part-way puts every sheet back.
//!
//! The caller checks staleness and, for a closed sheet, that its file is
//! still what the copy was read from, before calling.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use signex_engine::Command;
use signex_types::schematic::SchematicSheet;

use crate::app::{Signex, UndoGroupKind};
use crate::library::closed_sheets::FileStamp;

/// Why a grouped write changed nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SheetWriteRefusal {
    /// A sheet that is not open is being saved, renamed or moved.
    Busy(PathBuf),
    /// A sheet was written by a newer Signex and opens read-only.
    ReadOnly(PathBuf),
    /// A sheet that is not open could not be loaded.
    NotLoaded(PathBuf),
    /// A sheet refused the change.
    NotChanged(PathBuf),
}

/// One grouped write: its log label, its undo wording and the copies of the
/// sheets no engine held when the caller read them, by listed path.
pub(crate) struct SheetWrite<'a> {
    pub label: &'a str,
    /// What a sheet being saved or moved "could not be", for its refusal.
    pub outcome: &'a str,
    pub kind: UndoGroupKind,
    pub loaded: &'a HashMap<PathBuf, SchematicSheet>,
}

/// A sheet the run changed, before the run is committed.
struct Changed {
    path: PathBuf,
    tabless: bool,
    invalidation: Option<crate::schematic_runtime::RenderInvalidation>,
}

impl Signex {
    /// Run, per sheet of `edits` (by listed path), the commands `commands`
    /// builds from the sheet's live document, all as one undo group. The
    /// number of sheets changed, or why nothing was.
    pub(crate) fn write_sheets_as_group<T>(
        &mut self,
        write: &SheetWrite<'_>,
        edits: BTreeMap<PathBuf, T>,
        commands: impl Fn(&SchematicSheet, &T) -> Vec<Command>,
    ) -> Result<usize, SheetWriteRefusal> {
        let mut targets = Vec::with_capacity(edits.len());
        for (listed, edit) in edits {
            let path = self.resolve_held_spelling(listed.clone());
            let tabless = !self.document_state.engines.contains_key(&path);
            if tabless
                && (self.document_state.document_saves.is_saving(&path)
                    || self.refuse_while_path_op(&path, write.outcome).is_err())
            {
                return Err(SheetWriteRefusal::Busy(path));
            }
            // An open sheet's live document, else the copy a closed one will
            // be loaded from: either way the engine would refuse the edit.
            let read_only = match self.document_state.engines.get(&path) {
                Some(engine) => engine.document().is_read_only(),
                None => write
                    .loaded
                    .get(&listed)
                    .is_some_and(SchematicSheet::is_read_only),
            };
            if read_only {
                return Err(SheetWriteRefusal::ReadOnly(path));
            }
            targets.push((listed, path, tabless, edit));
        }
        let group = self.mint_undo_group();
        let mut changed: Vec<Changed> = Vec::new();
        for (listed, path, tabless, edit) in targets {
            if tabless
                && !self.install_tabless_engine(write.label, &path, write.loaded.get(&listed))
            {
                self.roll_back_sheet_write(write.label, group, &changed);
                return Err(SheetWriteRefusal::NotLoaded(path));
            }
            let Some(engine) = self.document_state.engines.get_mut(&path) else {
                continue;
            };
            let batch = commands(engine.document(), &edit);
            match engine.execute_batch_grouped(batch, Some(group)) {
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
                        &format!("{} failed on '{}'", write.label, path.display()),
                        &anyhow::Error::new(error),
                    );
                    self.drop_unchanged_tabless(&path, tabless);
                    self.roll_back_sheet_write(write.label, group, &changed);
                    return Err(SheetWriteRefusal::NotChanged(path));
                }
            }
        }
        let count = changed.len();
        self.commit_sheet_write(write.kind, group, changed);
        Ok(count)
    }

    /// Put back every sheet already changed: pop the group's entry and drop
    /// the engines this run loaded.
    fn roll_back_sheet_write(
        &mut self,
        label: &str,
        group: signex_engine::UndoGroupId,
        changed: &[Changed],
    ) {
        for done in changed {
            if let Some(engine) = self.document_state.engines.get_mut(&done.path)
                && let Err(error) = engine.discard_undo_group_top(group)
            {
                crate::diagnostics::log_error(
                    &format!("{label}: could not put back '{}'", done.path.display()),
                    &anyhow::Error::new(error),
                );
            }
            self.drop_unchanged_tabless(&done.path, done.tabless);
        }
    }

    /// Every sheet changed: mark them unsaved, register the group and
    /// refresh the canvas.
    fn commit_sheet_write(
        &mut self,
        kind: UndoGroupKind,
        group: signex_engine::UndoGroupId,
        changed: Vec<Changed>,
    ) {
        let active = self.document_state.active_path.clone();
        let mut recorded = crate::app::UndoGroupSheets {
            kind,
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

/// The files of `stamps` that differ now from their stamp at read time; a
/// file that could not be stated then counts as changed. Blocking.
pub(crate) fn disk_changed(stamps: &[(PathBuf, Option<FileStamp>)]) -> Vec<PathBuf> {
    stamps
        .iter()
        .filter(|(path, then)| then.is_none() || FileStamp::of(path) != *then)
        .map(|(path, _)| path.clone())
        .collect()
}
