//! The two halves of a [`CommandContext`] the enablement gates read (ADR-0006
//! slice 7): [`CoreContext`], the document, engine and project state a core
//! command's gate reads, and [`UiContext`], the window, canvas, selection,
//! tool, clipboard and panel state only a GUI gate reads. A core-bound
//! [`super::needs::Needs`] is met from a `CoreContext` alone.
//!
//! A `CoreContext` fact is read from document, engine, project or library
//! state. The window state (`ui_state`) only resolves which document the
//! window a command came from shows, as a front end fills in an implicit
//! target (ADR-0006, "Driving ports"); a fact read from a panel, the canvas
//! or the interaction state belongs in `UiContext`
//! (`core_facts_ignore_panel_and_interaction_state` checks it).
//!
//! Known exception, pending kill-check decision 8 (`BoardSketchSession` lives
//! in `InteractionState`): `document_is_dirty_board` and a board's
//! `can_undo` / `can_redo` read its sketch session, and so does
//! `has_dirty_documents`, which counts an unapplied sketch as the board's
//! unsaved work. A board sketch is board work, so the facts stay core;
//! `a_board_sketch_session_is_the_known_interaction_read_of_the_core_facts`
//! pins the dirty-board read, so moving the session turns it red.

use super::CommandContext;
use signex_commands::PcbTargets;

/// The facts of the document a command acts on, its engine and its project.
/// Each field is the [`CommandContext`] field of the same name.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct CoreContext {
    pub has_schematic: bool,
    pub has_pcb: bool,
    pub document_is_schematic: bool,
    pub document_is_editor: bool,
    pub document_is_dirty_board: bool,
    pub document_has_file: bool,
    pub has_sheets: bool,
    pub has_symbol_editor: bool,
    pub has_footprint_editor: bool,
    pub can_undo: bool,
    pub can_redo: bool,
    pub project_dirty: bool,
    pub has_dirty_documents: bool,
    pub has_document: bool,
    pub has_open_project: bool,
    pub has_active_path: bool,
    pub has_open_library: bool,
    pub has_project_sheet: bool,
    pub pcb: CoreBoard,
}

/// The board facts that are document state: [`PcbTargets::shown`] and
/// [`PcbTargets::read_only`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct CoreBoard {
    pub shown: bool,
    pub read_only: bool,
}

/// The facts of the window a command came from: its view, canvas, selection,
/// tool, the clipboard and the panels. Each field is the [`CommandContext`]
/// field of the same name; `pcb` leaves `shown` and `read_only` to
/// [`CoreContext::pcb`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct UiContext {
    pub pcb_view_turned: bool,
    pub undocked_has_canvas: bool,
    pub has_selection: bool,
    pub selection_all_locked: bool,
    pub has_drawing_selection: bool,
    pub drawing: bool,
    /// Read from the Navigator panel's hierarchy cache, not the project.
    pub has_parent_sheet: bool,
    pub can_tile_windows: bool,
    pub is_tiled: bool,
    pub turn_in_hand: bool,
    pub pcb: PcbTargets,
}

impl CommandContext {
    /// The core half: what a core-bound gate may read.
    pub(crate) fn core(&self) -> CoreContext {
        CoreContext {
            has_schematic: self.has_schematic,
            has_pcb: self.has_pcb,
            document_is_schematic: self.document_is_schematic,
            document_is_editor: self.document_is_editor,
            document_is_dirty_board: self.document_is_dirty_board,
            document_has_file: self.document_has_file,
            has_sheets: self.has_sheets,
            has_symbol_editor: self.has_symbol_editor,
            has_footprint_editor: self.has_footprint_editor,
            can_undo: self.can_undo,
            can_redo: self.can_redo,
            project_dirty: self.project_dirty,
            has_dirty_documents: self.has_dirty_documents,
            has_document: self.has_document,
            has_open_project: self.has_open_project,
            has_active_path: self.has_active_path,
            has_open_library: self.has_open_library,
            has_project_sheet: self.has_project_sheet,
            pcb: CoreBoard {
                shown: self.pcb.shown,
                read_only: self.pcb.read_only,
            },
        }
    }

    /// The GUI half: what only a GUI gate reads.
    pub(crate) fn ui(&self) -> UiContext {
        UiContext {
            pcb_view_turned: self.pcb_view_turned,
            undocked_has_canvas: self.undocked_has_canvas,
            has_selection: self.has_selection,
            selection_all_locked: self.selection_all_locked,
            has_drawing_selection: self.has_drawing_selection,
            drawing: self.drawing,
            has_parent_sheet: self.has_parent_sheet,
            can_tile_windows: self.can_tile_windows,
            is_tiled: self.is_tiled,
            turn_in_hand: self.turn_in_hand,
            pcb: PcbTargets {
                shown: false,
                read_only: false,
                ..self.pcb
            },
        }
    }
}
