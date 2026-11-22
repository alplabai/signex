//! Project-wide undo steps: one history entry per sheet, stamped with a shared
//! [`signex_engine::UndoGroupId`], undone and redone together.
//!
//! An annotate run under "Annotate undo: One step for the whole project"
//! stamps each touched sheet's entry with one group. Ctrl+Z / Ctrl+Y on any of
//! those sheets then moves every live sheet of the group in one step.
//!
//! Out-of-order rule: a grouped step moves only when every live sheet of the
//! group has the group's entry on top of the relevant stack. A sheet edited
//! after the run has a newer entry on top, so undoing the group would revert
//! the run underneath that edit; the step is refused instead, with a Messages
//! panel line naming the sheets, and nothing changes. Undo that newer edit
//! first. For redo, a sheet the undo did not revert owes nothing; one that has
//! the group deeper in its undo stack but not on its redo stack (edited after
//! the undo) refuses.
//!
//! Undo until you save (#2760): a member no engine holds (saved and closed) or
//! whose history no longer holds the run (reopened, or trimmed past the history
//! cap) refuses the whole step, in both directions, before any sheet moves.
//! One Messages line names those sheets; a project is never left half undone.

use super::*;

/// The sheets one undo group touched.
#[derive(Debug, Clone, Default)]
pub struct UndoGroupSheets {
    /// Sheets whose open engine recorded the group's entry.
    pub live: Vec<std::path::PathBuf>,
    /// Sheets the last undo of the group actually reverted. A redo requires
    /// exactly these, so one edited after the undo blocks it.
    pub undone: Vec<std::path::PathBuf>,
    /// What the group is, named in its undo and redo messages.
    pub kind: UndoGroupKind,
    /// Sheets the run loaded into tabless parked engines (re-prefix or reset
    /// of a sheet that was not open). Back at their loaded state after an undo,
    /// they are clean, not unsaved.
    pub parked: Vec<std::path::PathBuf>,
    /// Sheets with no unsaved edits when the run began. One whose tab closes
    /// before the undo joins `parked`: it holds nothing the file lacks.
    pub clean_before: Vec<std::path::PathBuf>,
    /// Parked sheets an undo took back to disk state and dropped. Their
    /// history is gone, so the group can no longer be redone as a whole.
    pub dropped: Vec<std::path::PathBuf>,
    /// Boards whose step carries the group, moved with its sheets (#2366).
    pub boards: Vec<std::path::PathBuf>,
}

/// Which run stamped an undo group.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum UndoGroupKind {
    #[default]
    Annotate,
    Reprefix,
    Replace,
    /// Tools ▸ Assign Footprints (#2248).
    Footprints,
    /// Tools ▸ Annotation ▸ Number Schematic Sheets (#2247).
    SheetNumbers,
    /// Tools ▸ Annotation ▸ Back Annotate Schematics (#1312).
    BackAnnotate,
    /// An agent's `apply_edits` call that edited several sheets (#2178).
    AgentEdit,
    /// A board footprint swap and the linked parts' footprints (#2366).
    FootprintSwap,
    /// A board pin or gate swap and its schematic rewiring (#2410).
    PartSwap,
    /// Synchronize Sheet Entries and Ports writing a sheet's ports (#1276).
    SheetEntrySync,
}

impl UndoGroupSheets {
    pub(crate) fn noun(&self) -> &'static str {
        match self.kind {
            UndoGroupKind::Annotate => "annotate",
            UndoGroupKind::Reprefix => "re-prefix",
            UndoGroupKind::Replace => "replace",
            UndoGroupKind::Footprints => "footprint assignment",
            UndoGroupKind::SheetNumbers => "sheet numbering",
            UndoGroupKind::BackAnnotate => "back annotate",
            UndoGroupKind::AgentEdit => "agent edit",
            UndoGroupKind::FootprintSwap => "footprint change",
            UndoGroupKind::PartSwap => "pin or gate swap",
            UndoGroupKind::SheetEntrySync => "sheet entry sync",
        }
    }

    /// The step as a message names it: "the project-wide annotate".
    pub(crate) fn wide(&self) -> String {
        let scope = match self.kind {
            UndoGroupKind::AgentEdit => "cross-sheet",
            _ => "project-wide",
        };
        format!("{scope} {}", self.noun())
    }

    pub(crate) fn title(&self) -> &'static str {
        match self.kind {
            UndoGroupKind::Annotate => "Annotate",
            UndoGroupKind::Reprefix => "Re-prefix",
            UndoGroupKind::Replace => "Replace",
            UndoGroupKind::Footprints => "Assign Footprints",
            UndoGroupKind::SheetNumbers => "Sheet numbering",
            UndoGroupKind::BackAnnotate => "Back annotate",
            UndoGroupKind::AgentEdit => "Agent edit",
            UndoGroupKind::FootprintSwap => "Change Footprint",
            UndoGroupKind::PartSwap => "Pin or gate swap",
            UndoGroupKind::SheetEntrySync => "Sheet entry sync",
        }
    }

    /// The one line of a step refused because `sheets` cannot move with the
    /// others (#2760).
    pub(crate) fn out_of_reach(&self, undo: bool, sheets: &[std::path::PathBuf]) -> String {
        format!(
            "Cannot {} the {}: {} was saved and closed, or no longer holds this step, so no \
             sheet was changed.",
            if undo { "undo" } else { "redo" },
            self.wide(),
            file_names(sheets),
        )
    }
}

pub(crate) fn file_names(paths: &[std::path::PathBuf]) -> String {
    paths
        .iter()
        .map(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string())
        })
        .collect::<Vec<_>>()
        .join(", ")
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum StepDirection {
    Undo,
    Redo,
}

impl StepDirection {
    pub(super) fn top(self, engine: &signex_engine::Engine) -> Option<signex_engine::UndoGroupId> {
        match self {
            StepDirection::Undo => engine.next_undo_group(),
            StepDirection::Redo => engine.next_redo_group(),
        }
    }
}

impl crate::app::state::DocumentState {
    /// Record which sheets a group touched; a group that touched none goes.
    pub(crate) fn record_undo_group(
        &mut self,
        group: signex_engine::UndoGroupId,
        sheets: UndoGroupSheets,
    ) {
        if sheets.live.is_empty() {
            self.undo_groups.remove(&group);
        } else {
            self.undo_groups.insert(group, sheets);
        }
    }
}

impl Signex {
    /// Allocate a group id for a project-wide step.
    pub(super) fn mint_undo_group(&mut self) -> signex_engine::UndoGroupId {
        let id = signex_engine::UndoGroupId::new(self.document_state.next_undo_group);
        self.document_state.next_undo_group += 1;
        id
    }

    /// Record which sheets a group touched, once the run knows.
    pub(super) fn set_undo_group_sheets(
        &mut self,
        group: signex_engine::UndoGroupId,
        sheets: UndoGroupSheets,
    ) {
        self.document_state.record_undo_group(group, sheets);
    }

    /// Whether `group` holds board steps (#2366): those move through
    /// [`Self::step_board_undo_group`], not the sheets' grouped step.
    pub(super) fn undo_group_has_boards(&self, group: signex_engine::UndoGroupId) -> bool {
        (self.document_state.undo_groups.get(&group))
            .is_some_and(|sheets| !sheets.boards.is_empty())
    }

    /// Project a grouped step of the sheets: its notices and the other
    /// members' windows, then the active canvas. The panel context follows
    /// any step that ran, so the ERC panel's out-of-date notice does even
    /// when the active sheet itself was not touched. Returns the step's
    /// `stepped`.
    pub(super) fn project_group_step(
        &mut self,
        step: crate::app::state::sheet_change::GroupStep,
        update_selection_info: bool,
    ) -> bool {
        use crate::app::projector::RefreshScope;
        self.project_events(step.members, RefreshScope::Member);
        let scope = RefreshScope::ActiveCanvas {
            clear_overlay: true,
            update_selection_info,
        };
        self.project_events(step.active, scope);
        if step.ran {
            self.refresh_panel_ctx();
        }
        step.stepped
    }

    /// Undo (`undo`) or redo a group that holds board steps as well as sheet
    /// steps (#2366), from the board or from a sheet: every live member moves
    /// in one step. Refused, with nothing moved, when a member has a later
    /// step above the group's, or a member is out of reach (#2760).
    pub(crate) fn step_board_undo_group(
        &mut self,
        group: signex_engine::UndoGroupId,
        undo: bool,
    ) -> bool {
        let Some(recorded) = self.document_state.undo_groups.get(&group).cloned() else {
            return false;
        };
        let direction = if undo {
            StepDirection::Undo
        } else {
            StepDirection::Redo
        };
        let owed = |path: &std::path::PathBuf| undo || recorded.undone.contains(path);
        let (mut sheets, mut boards, mut blocked, mut skipped) =
            (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for path in &recorded.live {
            match self.document_state.engines.get(path) {
                Some(engine) if direction.top(engine) == Some(group) => sheets.push(path.clone()),
                // A redo with no history at all: reopened, not edited.
                Some(engine)
                    if (undo && engine.has_undo_group(group))
                        || (!undo && owed(path) && (engine.can_undo() || engine.can_redo())) =>
                {
                    blocked.push(path.clone());
                }
                _ if owed(path) => skipped.push(path.clone()),
                _ => {}
            }
        }
        for path in &recorded.boards {
            let Some(engine) = self.document_state.board_engine(path) else {
                if owed(path) {
                    skipped.push(path.clone());
                }
                continue;
            };
            let top = if undo {
                engine.next_undo_group()
            } else {
                engine.next_redo_group()
            };
            if top == Some(group) {
                boards.push(path.clone());
            } else if (undo && engine.has_undo_group(group)) || (!undo && owed(path)) {
                blocked.push(path.clone());
            } else if owed(path) {
                skipped.push(path.clone());
            }
        }
        let verb = if undo { "undo" } else { "redo" };
        if !blocked.is_empty() {
            crate::diagnostics::log_warning(format!(
                "Cannot {verb} the {}: {} has a later step. Undo the newer edit there first.",
                recorded.noun(),
                file_names(&blocked),
            ));
            return false;
        }
        if !skipped.is_empty() {
            crate::diagnostics::log_warning(recorded.out_of_reach(undo, &skipped));
            return false;
        }
        if sheets.is_empty() && boards.is_empty() {
            return false;
        }
        let active = self.document_state.active_path.clone();
        let mut active_patch = None;
        let mut moved = Vec::new();
        for path in &sheets {
            let Some(engine) = self.document_state.engines.get_mut(path) else {
                continue;
            };
            match if undo { engine.undo() } else { engine.redo() } {
                Ok(Some(pair)) => {
                    moved.push(path.clone());
                    // Undo/redo can bring back an older embedded library symbol.
                    self.library.drift.check_pending = true;
                    if active.as_ref() == Some(path) {
                        active_patch = Some(pair.document);
                    } else {
                        self.document_state.sync_history_dirty(path);
                        self.repaint_member_sheet(path, pair.document);
                    }
                }
                Ok(None) => {}
                Err(error) => crate::diagnostics::log_error(
                    "Engine group step failed",
                    &anyhow::Error::new(error),
                ),
            }
        }
        for path in &boards {
            if self.step_board_once(path, undo) {
                moved.push(path.clone());
            }
        }
        if undo && let Some(record) = self.document_state.undo_groups.get_mut(&group) {
            record.undone = moved;
        }
        self.document_state.note_sheet_edit();
        self.finish_schematic_mutation(active_patch, None, true, true);
        self.refresh_panel_ctx();
        true
    }
}
