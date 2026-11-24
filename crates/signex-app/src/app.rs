//! Main Iced application — module root, update loop, view tree.

use iced::{Task, Theme};
use signex_types::coord::Unit;
use signex_types::theme::ThemeId;

use crate::canvas::CanvasEvent;
use crate::dock::PanelPosition;
use crate::menu_bar::{self, MenuMessage};
use crate::tab_bar::{self, TabMessage};
use crate::toolbar::ToolMessage;

mod actions;
pub(crate) mod agent;
pub(crate) mod assign_footprints;
pub(crate) mod back_annotate;
pub(crate) mod board_output;
pub(crate) mod board_panel;
pub(crate) mod board_reannotate;
pub(crate) mod board_reuse;
pub(crate) mod board_tools;
mod bootstrap;
pub(crate) mod channel_view;
pub(crate) mod command;
pub mod command_palette;
pub(crate) mod compare_board;
pub(crate) mod compiled_design;
pub mod contracts;
pub(crate) mod core;
mod dir_listing;
pub(crate) mod dispatch;
mod document;
pub mod document_ids;
pub mod document_read;
mod documents;
pub(crate) mod edge_pan;
pub(crate) mod engine_notices;
pub(crate) mod fab_outputs;
// #1282: Design ▸ Differential Pair Classes….
pub(crate) mod diff_pair_classes;
pub use diff_pair_classes::{
    DiffPairClassesMsg, DiffPairClassesState, PairClassField, PairClassProblem, PairMemberRow,
};
pub(crate) mod file_extension;
pub mod gerber_read;
// Mockup rows 193 and 193f: the Gerber Viewer tab.
pub mod gerber_viewer;
pub use gerber_viewer::{GerberViewerMsg, GerberViewerState};
mod handlers;
#[cfg(test)]
pub(crate) mod hierarchy_fixture;
mod layer_stack_tab;
mod library_delete;
mod load_gateway;
mod mutation_gateway;
pub(crate) mod new_project;
pub(crate) mod page_tab;
pub mod parent_reference_disk;
mod parked_sheets;
pub mod path_op_jobs;
pub mod path_ops;
pub(crate) mod pin_junction_repair;
mod pin_table_tab;
pub mod project_entry_disk;
pub(crate) mod project_git;
mod project_tab;
pub(crate) mod projector;
pub(crate) mod rail_rename;
pub use rail_rename::RailRenameMsg;
pub(crate) mod rooms;
mod rules_tab;
pub(crate) mod sch_tables;
pub(crate) mod sheet_numbering;
pub(crate) mod sheet_repairs;
pub(crate) mod sheet_sync;
pub(crate) mod sheet_write;
pub mod unions;
pub use project_git::ProjectGitMsg;
pub use project_tab::{ProjectGeneralFacts, ProjectSection, ProjectTabMsg, ProjectTabState};
pub use rules_tab::{RulesTabMsg, RulesTabState};
pub use sch_tables::TablesMsg;
pub(crate) mod design_read;
pub(crate) mod designator_scan;
#[cfg(test)]
mod designator_scan_tests;
pub(crate) mod document_setup;
#[cfg(test)]
mod edit_tail_seam_tests;
#[cfg(test)]
mod edit_tail_tests;
pub mod palette_index;
#[cfg(test)]
mod palette_index_tests;
pub(crate) mod pcb_bar_groups;
mod pcb_dirty_adapter;
pub(crate) mod pcb_tools;
pub(crate) mod project_sheets;
mod read_only_refusal;
pub mod restore_job;
mod runtime;
pub(crate) mod saved_channel_naming;
mod selection_request;
pub(crate) mod sheet_hierarchy;
mod state;
pub mod window_geometry;
pub(crate) use state::library_entry;
pub(crate) use state::rule_profile_ui;
pub(crate) use state::scope::{deepest_owner, deepest_owners, path_key_with};
pub(crate) use state::volume_case::probe_directory_case;
// The Library dialogs' window-size tests name the modal they open.
#[cfg(test)]
pub(crate) use state::ModalId;
#[cfg(test)]
mod draft_save_point_tests;
pub(crate) mod stitched_netlist;
mod symbol_history;
mod undo_depth;
mod undo_groups;
pub(crate) mod unread_sheets;
pub(crate) mod update_pcb;
pub use undo_groups::{UndoGroupKind, UndoGroupSheets};
pub(crate) mod view;

pub use assign_footprints::{
    AssignFilter, AssignFootprintsMsg, AssignFootprintsState, AssignProblem, FootprintOffer,
    FootprintPart, PartAccess, TickedBar,
};
pub use back_annotate::{BackAnnotateMsg, BackAnnotateState};
pub use board_output::{
    BoardOutputDialog, BoardOutputMsg, BoardOutputState, BoardPrintFlag, BoardPrintState,
    BoardReportsState,
};
pub use board_panel::BoardPanelMsg;
pub use board_reannotate::{BoardReannotateMsg, BoardReannotateState, ReannotateProblem};
pub use board_reuse::ReuseBlockMsg;
pub use compare_board::{CompareBoardMsg, CompareBoardState, CompareRead, JumpSide};
pub use compiled_design::CompiledDesignMsg;
pub use contracts::Board3dShot;
pub use contracts::ErcProfileAction;
pub use contracts::{
    AnnotateMsg, AppQuitConfirmState, Board3dGesture, Board3dModelsRead, Board3dMsg, BoardNetMsg,
    BoardSketchMsg, BoardToolsMsg, BomPreviewMsg, CommandPaletteMsg, ContextAction, ContextMenuMsg,
    ContextMenuState, ContextSubmenu, CrossProbeMsg, DragTarget, DrcRunResult, DxfImportMsg,
    EditMsg, EnableVersionControlMsg, EnableVersionControlOutcome, EnableVersionControlState,
    ErcMsg, ExcludeDirtyChoice, ExportMsg, FileMsg, FootprintSwapMsg, GitCommitFailure,
    GridPickerState, GridPropertiesMsg, GridPropertiesState, InputMsg, KeymapModMigration,
    LayerSetCard, LayerSetMenu, LayerSetMenuMsg, LayerSetMsg, LayerStripMsg, LayerTabMenu,
    LayerTabMenuMsg, LayerTabSubmenu, LibraryRowRef, Message, MoveSelectionMsg, NetColorMsg,
    NetNavMsg, OverlayMsg, ParameterManagerMsg, PartSwapMsg, PasteIntent, PasteRequest, PcbBarMenu,
    PcbBarMsg, PcbMsg, PcbViewMsg, PdfOptionMsg, PlacementExtrasMsg, PreferencesMsg,
    PrintPreviewMsg, ProjectCloseChoice, ProjectCloseConfirmState, ProjectMsg, ProjectRenameJob,
    ProjectTreeAction, ProjectTreeContextMenuState, RemoveChoice, RemoveDialogState, RemoveMsg,
    RenameDialogState, RenameMsg, SelectionFilterCustomState, SelectionFilterMsg, StatusBarRequest,
    TabContextAction, TabContextMenuState, TextEditMsg, TextEditState, TextEditTarget, TrackItem,
    TuneMsg, UiMsg, VersionControlScope, ViewAngleEntry, ViewRotationMsg, WindowMsg,
};
pub(crate) use document::{Document, DocumentKey};
pub use document_setup::{DocumentSetupMsg, DocumentSetupState, SetupKind, SheetChoice};
pub use documents::{
    ComponentEditorTab, FootprintDocument, FootprintHistorySnapshot, GraphicFillPicker,
    LocalColorPicker, LocalColorSlot, SymbolDocument, SymbolViewRequest, TabDocument, TabInfo,
    TabKind, Tool,
};
pub use fab_outputs::{BoardNote, FabOutput, FabOutputsMsg, FabOutputsState, FabProblem};
pub(crate) use handlers::document_files::is_pcb_path;
pub(crate) use handlers::selection_workflow::passes_filter;
pub use handlers::window_geometry_events::GeometryReadout;
pub use handlers::window_tiling::{Tile, TileLayout, TilePlan, tiles};
pub use new_project::{NewProjectMsg, NewProjectState, ProfileUndo};
pub use sheet_numbering::{NumberingProblem, SheetNumberingMsg, SheetNumberingState};
pub use sheet_sync::{SheetSyncMsg, SheetSyncProblem, SheetSyncState};
pub use signex_history::History;
pub(crate) use signex_history::SavePoint;
#[cfg(test)]
pub(crate) use state::BoardSketchSession;
#[cfg(test)]
pub(crate) use state::WindowKind;
pub use state::net_class_styles::{ResolveFailed, ResolvedStyles};
pub use state::pcb_edit::{BoardEditError, CheckedStep};
pub use state::project_case::CaseProbeOutcome;
pub use state::{
    CaseFold, DocumentState, InteractionState, KeymapRecorderState, LoadedProject, PageSetupInputs,
    PreviewState, ProjectId, Signex, UiState,
};
pub use symbol_history::SymbolHistorySnapshot;
pub use update_pcb::{RowAction, RowFilter, TableGroup, UpdatePcbMsg, UpdatePcbState};

// Re-exported so modal dialogs outside `state` can reference the type.
pub use state::AnnotateOrder;

impl Signex {
    pub fn update(&mut self, message: Message) -> Task<Message> {
        let _probe = crate::frame_probe::span_with("update", || {
            crate::frame_probe::message_name(&format!("{message:?}"))
        });
        // Messages that complete a save, delete, rename or library write may
        // have changed the disk under an unchanged path set; the probe below
        // then runs now instead of at the next 2 s tick.
        if state::project_fs::message_touches_disk(&message) {
            self.document_state.project_fs.invalidate();
        }
        let before = self.active_tab_path();
        let popover = self.watch_grid_snap_popover();
        self.ui_state.command_replay.enter_update();
        let mut tasks = vec![self.dispatch_update(message)];
        if self.ui_state.command_replay.leave_update() {
            self.expire_replay_pending();
        }
        // Text typed into a note that is no longer selected (another pick, a
        // tab switch) goes to its own sheet as one edit.
        self.settle_owed_note_drafts();
        // A power port's rename in a project reads its closed sheets first.
        tasks.push(self.start_rail_rename());
        // So does a drawing value still typed when its field's drawing goes.
        self.settle_live_drawing_edit();
        self.drop_line_width_text_of_another_symbol();
        // The Grid & Snap popover goes with its sheet or under a new dialog.
        self.sync_grid_snap_popover(popover);
        // Cross select follows whatever this message selected, and a Cross
        // Probe asked for runs once its target is open (#1312).
        tasks.push(self.sync_cross_select());
        // An Update PCB waiting for its board starts reading once it opened.
        tasks.push(self.sync_update_pcb());
        tasks.push(self.sync_back_annotate());
        // The Annotations panel reads the notebook when shown or when the
        // project in view changes (mockup row 165).
        tasks.push(self.sync_annotations_panel());
        tasks.push(self.sync_compare_board());
        // A Properties draft of a board no window shows any more goes (#1312).
        self.prune_pcb_footprint_draft();
        self.prune_pcb_component_draft();
        // A New PCB / New Schematic dialog of a closed project closes (#2232).
        self.drop_document_setup_of_closed_project();
        // The Rules tab in view re-reads its flags when its rules changed (#2398).
        self.sync_rules_analysis();
        self.sync_undo_depth();
        // The Start screen's time column reads its clock from the Model.
        self.sync_start_screen_clock();
        // A resize, a dock collapsed or expanded, or a panel docked into an
        // empty side all change the room beside the canvas (#2347).
        self.fit_side_docks_to_window();
        // A Save completed behind the quit prompt must not leave it listing
        // files that are no longer unsaved (#1833).
        tasks.push(self.prune_settled_quit_confirm());
        tasks.extend(self.sync_detached_modals());
        // Any activation path (select, open, close-neighbour, Ctrl+Tab)
        // ends here: scroll the tab strip to the newly active tab.
        if self.active_tab_path() != before {
            let docs = &self.document_state;
            tasks.push(tab_bar::reveal_active_tab(docs.active_tab, docs.tabs.len()));
        }
        // Every schematic open queued a look for an earlier run's recovery
        // copy; start them whichever path opened the tab (#1208).
        let checks = self.start_due_recovery_checks();
        tasks.push(checks);
        // `refresh_panel_ctx` only records what the project tree needs from
        // the disk; the probe that fetches it starts here (#111).
        if let Some(probe) = self.start_project_fs_probe(false) {
            tasks.push(probe);
        }
        // Likewise the Navigator hierarchy: the refresh records the request,
        // the sheet reads run here off-thread (#1470).
        if let Some(assembly) = self.start_navigator_assembly() {
            tasks.push(assembly);
        }
        tasks.extend(self.start_window_hierarchy_assemblies());
        // And the symbol and footprint summaries of the selected library row.
        if let Some(read) = self.start_row_summary_read() {
            tasks.push(read);
        }
        // The projects an agent can reach are listed in the registry (#1924).
        if let Some(registry) = self.sync_agent_registry() {
            tasks.push(registry);
        }
        if tasks.len() == 1 {
            return tasks.pop().expect("one task");
        }
        Task::batch(tasks)
    }
}
