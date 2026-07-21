use std::path::PathBuf;

use signex_types::schematic::SchematicSheet;
use signex_types::theme::ThemeId;

use crate::canvas::CanvasEvent;
use crate::dock::DockMessage;
use crate::menu_bar::MenuMessage;
use crate::tab_bar::TabMessage;
use crate::toolbar::ToolMessage;

use super::selection_request::SelectionRequest;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragTarget {
    LeftPanel,
    RightPanel,
    BottomPanel,
    ComponentsSplit,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum Message {
    Menu(MenuMessage),
    Tool(ToolMessage),
    /// Tab-bar message carrying the id of the window whose tab bar emitted
    /// it. Lets the handler distinguish main-window tab reorder/select
    /// (which mutates `document_state.active_tab`) from an undocked
    /// window's tab bar, which only has one visible tab and must not
    /// clobber the main window's active index.
    Tab {
        window_id: iced::window::Id,
        msg: TabMessage,
    },
    Dock(DockMessage),
    /// UI / chrome message family (ADR-0001 D3). Theme, unit + grid
    /// toggles, the grid picker, layout drag, window resize, and the
    /// status-bar request enum. Routed to `dispatch_ui_message`.
    Ui(UiMsg),
    CanvasEvent(CanvasEvent),
    /// Canvas event stamped with the window that produced it. The
    /// dispatch layer swaps the window's `SchematicCanvas` into the
    /// main canvas slot for the duration of the handler so the
    /// hundreds of `active_canvas_mut()` call sites read and write the
    /// right canvas transparently. Keyboard-generated canvas events
    /// (FitAll shortcut, etc.) continue to use the unwrapped
    /// `Message::CanvasEvent` variant and always target the main
    /// window.
    CanvasEventInWindow {
        window_id: iced::window::Id,
        event: CanvasEvent,
    },
    /// Grid Properties dialog — namespaced (ADR-0001 D3).
    GridProperties(GridPropertiesMsg),
    /// Custom Selection Filter modal message family — namespaced
    /// (ADR-0001 D3). Routed to `dispatch_selection_filter_message`.
    SelectionFilter(SelectionFilterMsg),
    /// File / save message family (ADR-0001 D3). Namespaced under
    /// `Message::File` and routed to `dispatch_file_message`.
    File(FileMsg),
    /// Edit-command message family (ADR-0001 D3). Namespaced under
    /// `Message::Edit` and routed to `dispatch_edit_message`.
    Edit(EditMsg),
    Selection(SelectionRequest),
    /// Overlay / modal-chrome message family (ADR-0001 D3). Panel-list
    /// toggle, panel open, find/replace open, keyboard-shortcuts and
    /// first-run-tour dismiss, modal drag, canvas focus-at, auto-focus
    /// toggle. Routed to `dispatch_overlay_message`.
    Overlay(OverlayMsg),
    ActiveBar(crate::active_bar::ActiveBarMsg),
    /// In-canvas text-edit message family (ADR-0001 D3). Namespaced
    /// under `Message::TextEdit` and routed to
    /// `dispatch_text_edit_message`.
    TextEdit(TextEditMsg),
    /// Context-menu subsystem message family — namespaced (ADR-0001
    /// D3). Canvas / project-tree / tab right-click menus plus the
    /// submenu hover state machine. Routed to
    /// `dispatch_context_menu_message`.
    ContextMenu(ContextMenuMsg),
    /// Project lifecycle message family (ADR-0001 D3). Namespaced
    /// under `Message::Project` and routed to `dispatch_project_message`.
    Project(ProjectMsg),
    /// Rename modal message family — namespaced (ADR-0001 D3). Routed
    /// to `dispatch_rename_message`.
    Rename(RenameMsg),
    /// Remove-from-project modal message family — namespaced (ADR-0001
    /// D3). Routed to `dispatch_remove_message`.
    Remove(RemoveMsg),
    /// Enable Version Control modal message family — namespaced
    /// (ADR-0001 D3). Routed to `dispatch_enable_version_control_message`.
    EnableVersionControl(EnableVersionControlMsg),
    /// Preferences modal message family — namespaced (ADR-0001 D3).
    /// Routed to `dispatch_preferences_message`.
    Preferences(PreferencesMsg),
    FindReplaceMsg(crate::find_replace::FindReplaceMsg),
    /// Window lifecycle / docking / native-chrome message
    /// family (ADR-0001 D3). Routed to `dispatch_window_message`.
    Window(WindowMsg),
    /// ERC dialog message family — namespaced (ADR-0001 D3). Routed to
    /// `dispatch_erc_message`.
    Erc(ErcMsg),
    /// Annotate dialog message family — namespaced (ADR-0001 D3). Routed
    /// to `dispatch_annotate_message`.
    Annotate(AnnotateMsg),
    /// Move Selection dialog family (ADR-0001 D3). Altium numeric
    /// ΔX / ΔY move. Routed to `dispatch_move_selection_message`.
    MoveSelection(MoveSelectionMsg),
    /// Per-net colour overrides + F5 palette open/close — namespaced
    /// (ADR-0001 D3). Routed to `dispatch_net_color_message`.
    NetColor(NetColorMsg),
    /// Parameter Manager dialog family (ADR-0001 D3). Bulk parameter
    /// editor. Routed to `dispatch_parameter_manager_message`.
    ParameterManager(ParameterManagerMsg),
    /// Cycle Altium's rubber-band selection mode
    /// Inside → Outside → TouchingLine → Inside. Bound to Shift+S.
    CycleSelectionMode,
    /// Close the in-flight lasso polygon (Enter key). Commits the
    /// selection if >= 3 vertices, otherwise cancels.
    LassoCommit,
    /// Apply an edit to a placed SchDrawing. Dispatched from the
    /// post-placement Properties panel (Line / Rect / Circle / Arc /
    /// Polygon editable rows). Engine replaces the stored drawing by
    /// uuid with full undo.
    UpdateDrawingField(uuid::Uuid, DrawingFieldEdit),
    /// Export subsystem message family — namespaced (ADR-0001 D3).
    /// PDF / netlist / BOM export lifecycle plus the export-error
    /// modal dismiss. Routed to `dispatch_export_message`.
    Export(ExportMsg),
    /// BOM-preview modal — namespaced family (ADR-0001 D3). Routed to
    /// `dispatch_bom_preview_message`.
    BomPreview(BomPreviewMsg),
    /// Print-preview modal — namespaced family (ADR-0001 D3). Routed to
    /// `dispatch_print_preview_message`.
    PrintPreview(PrintPreviewMsg),
    /// v0.9 Library subsystem message — folded under one variant so
    /// the dispatcher can route to `library_dispatch::handle` in one
    /// shot. See `crate::library::LibraryMessage` for the inner
    /// shape.
    Library(crate::library::LibraryMessage),
    /// Command-palette message family (ADR-0001 D3). Namespaced under
    /// `Message::CommandPalette` and routed to
    /// `dispatch_command_palette_message`.
    CommandPalette(CommandPaletteMsg),
    /// Async result of a per-file Git history load issued by the
    /// right-dock History panel. `generation` is the value of
    /// `DocumentState.history.generation` at the time the load was
    /// kicked off; the handler discards stale results whose
    /// generation no longer matches the current counter (the user
    /// has switched tabs since). `path` is the file the load
    /// targeted, surfaced for diagnostic logging only — the
    /// generation token is the authoritative staleness check.
    HistoryLoaded {
        generation: u32,
        path: std::path::PathBuf,
        result: Result<Vec<signex_widgets::HistoryEntry>, String>,
    },
    /// v0.14.2 — keyboard shortcut for footprint editor mode switch.
    /// Routed from the global `1` / `2` / `3` key handler in
    /// `bootstrap.rs::subscription`. The dispatcher checks whether
    /// the active tab is a footprint editor; if yes, sets the mode;
    /// otherwise no-op so the keys don't steal text input on other
    /// tabs.
    FootprintModeShortcut(crate::library::editor::footprint::state::EditorMode),
    /// v0.15 — Esc-key tool cancel routed through the dispatcher.
    /// If the active tab is a footprint editor, fires
    /// `FootprintToolEscape` (resets PadsTool + SketchTool +
    /// tool_pending); otherwise falls back to the schematic
    /// `Tool::Select` reset.
    EscapePressed,
    Noop,
}

mod dialogs;
mod messages;
mod state;

pub use dialogs::*;
pub use messages::*;
pub use state::*;
