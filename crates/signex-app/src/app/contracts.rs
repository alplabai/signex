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
    StatusBar(StatusBarRequest),
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
    #[allow(dead_code)]
    ThemeChanged(ThemeId),
    UnitCycled,
    GridToggle,
    GridCycle,
    DragStart(DragTarget),
    DragMove(f32, f32),
    DragEnd,
    FileOpened(Option<PathBuf>),
    #[allow(dead_code)]
    SchematicLoaded(Box<SchematicSheet>),
    DeleteSelected,
    Undo,
    Redo,
    RotateSelected,
    MirrorSelectedX,
    MirrorSelectedY,
    Selection(SelectionRequest),
    Copy,
    Cut,
    Paste,
    SmartPaste,
    Duplicate,
    SaveFile,
    SaveFileAs(PathBuf),
    /// User picked a destination from the Save-As dialog spawned the
    /// first time a freshly-minted `.snxsym` / `.snxfpt` editor tab is
    /// saved (the in-memory tab opened by `Add New ▸ Symbol` /
    /// `Add New ▸ Footprint`). Re-keys the editor + tab from the
    /// suggested path to the user's choice, then writes the file.
    /// `from_path` is the suggested in-memory path the editor is
    /// currently keyed under; `to_path` is the rfd result.
    SavePrimitiveAs {
        from_path: PathBuf,
        to_path: PathBuf,
    },
    CycleDrawMode,
    CancelDrawing,
    TogglePanelList,
    OpenPanel(crate::panels::PanelKind),
    ActiveBar(crate::active_bar::ActiveBarMsg),
    PrePlacementTab,
    /// Resume placement after TAB paused it — clears `pre_placement` and
    /// `placement_paused`. Wired to the big on-canvas "Resume" overlay.
    ResumePlacement,
    TextEditChanged(String),
    TextEditSubmit,
    ShowContextMenu(f32, f32),
    CloseContextMenu,
    /// Right-click landed on a specific tree node — open the per-node
    /// context menu at `last_mouse_pos`. `path = None` → background menu.
    ShowProjectTreeContextMenu(Option<Vec<usize>>),
    /// Dismiss the Projects-panel tree context menu.
    CloseProjectTreeContextMenu,
    /// Menu item picked — route the action.
    ProjectTreeAction(ProjectTreeAction),
    /// Right-click on a document tab — open the per-tab context menu
    /// at `last_mouse_pos`. Carries the clicked tab's index.
    ShowTabContextMenu(usize),
    /// Dismiss the document-tab right-click menu.
    CloseTabContextMenu,
    /// Menu item picked — route the action.
    TabContextAction(TabContextAction),
    /// User picked an option in the project-close confirmation modal
    /// (Save All / Discard All / Cancel) shown when closing a
    /// project that still has entries in `dirty_paths`.
    ProjectCloseConfirm(ProjectCloseChoice),
    /// Text input in the rename modal — updates the live buffer.
    RenameBufferChanged(String),
    /// Commit the rename: fs::rename + update in-memory sheet / tab
    /// state. Errors surface in `RenameDialogState::error`.
    RenameSubmit,
    /// Dismiss the rename modal without applying.
    CloseRenameDialog,
    /// User picked Delete / Exclude in the Remove modal.
    RemoveConfirm(RemoveChoice),
    /// Dismiss the Remove modal without applying.
    CloseRemoveDialog,
    /// Expand a click-to-open submenu inside the right-click context
    /// menu (Place or Align). Toggles off when the same kind is fired
    /// twice, otherwise replaces the current submenu.
    OpenContextSubmenu(ContextSubmenu),
    /// Hover entered a submenu launcher row — start the 200 ms
    /// hover-open timer for that submenu (and cancel any pending
    /// close).
    HoverContextSubmenu(ContextSubmenu),
    /// Hover left the submenu launcher row — cancels any pending
    /// open and starts the 150 ms close timer if a submenu is open.
    LeaveContextSubmenu,
    /// Hover entered the open submenu panel — cancels the close timer
    /// so the panel stays visible while the cursor traverses it.
    EnterContextSubmenuPanel,
    /// Hover left the open submenu panel — starts the close timer.
    LeaveContextSubmenuPanel,
    /// 50 ms tick fired by the subscription while the context menu is
    /// open; promotes a mature `pending_submenu` into an actual open
    /// and a mature `pending_submenu_close` into an actual close.
    TickContextSubmenuHover,
    ContextAction(ContextAction),
    OpenPreferences,
    /// Close the Help ▸ Keyboard Shortcuts modal — fired by the close
    /// chrome ✕ and by Esc dismiss handling.
    CloseKeyboardShortcuts,
    OpenFind,
    OpenReplace,
    ClosePreferences,
    PreferencesNav(crate::preferences::PrefNav),
    PreferencesMsg(crate::preferences::PrefMsg),
    FindReplaceMsg(crate::find_replace::FindReplaceMsg),
    WindowResized(f32, f32),
    /// Resize event carrying the window id. Forwarded by the
    /// `iced::window::resize_events()` subscription so the dispatcher
    /// can drop non-main-window resizes before they touch
    /// `ui_state.window_size`.
    WindowResizedFor(iced::window::Id, f32, f32),
    /// Run the ERC engine against the active schematic snapshot and populate
    /// `ui_state.erc_violations`. Bound to F8.
    RunErc,
    /// Auto-annotate every unannotated symbol (reference ends in `?`).
    /// Three modes: incremental, reset+renumber, reset-only.
    Annotate(signex_engine::AnnotateMode),
    /// Show the Annotate Schematics modal with preview of proposed changes.
    OpenAnnotateDialog,
    /// Dismiss the Annotate dialog without applying.
    CloseAnnotateDialog,
    /// Change the Annotate dialog's order-of-processing choice.
    AnnotateOrderChanged(super::state::AnnotateOrder),
    /// Show the ERC modal (severity matrix + pin-compatibility matrix).
    OpenErcDialog,
    /// Dismiss the ERC dialog.
    CloseErcDialog,
    /// Override the severity for a single rule from within the ERC dialog.
    ErcSeverityChanged(signex_erc::RuleKind, signex_erc::Severity),
    /// Show the Reset-Annotations confirm modal.
    OpenAnnotateResetConfirm,
    /// Dismiss the Reset-Annotations confirm modal.
    CloseAnnotateResetConfirm,
    /// User pressed the title bar of a modal at window-space (x, y) — begin
    /// dragging it. The next DragMove events update its offset.
    ModalDragStart {
        modal: super::state::ModalId,
        x: f32,
        y: f32,
    },
    /// Modal drag released (mouse-up). Clears `modal_dragging`.
    ModalDragEnd,
    /// Navigate to a world-space point on the canvas; optionally replace the
    /// current selection with the given item. Used for click-to-zoom in the
    /// Messages panel.
    FocusAt {
        world_x: f64,
        world_y: f64,
        select: Option<signex_types::schematic::SelectedItem>,
    },
    /// Toggle AutoFocus — dim everything not in the current selection.
    ToggleAutoFocus,
    /// Fired once `iced::window::open` completes for the initial main
    /// window — lets us stash the id so `view(id)` knows which window is
    /// the primary app shell versus a detached modal / undocked tab.
    MainWindowOpened(iced::window::Id),
    /// OS-reported scale factor for the main window. Fired on window
    /// open and on every main-window resize (winit emits a resize event
    /// when Windows moves the window across monitors with different
    /// scale factors). Stored in `ui_state.main_window_scale` and used
    /// by the menu bar to pick the crispest wordmark PNG tier.
    MainWindowScaleChanged(f32),
    /// Fired when any secondary (non-main) window closes. Cleans up the
    /// corresponding entry in `ui_state.windows` so the app can re-attach
    /// the modal or tab to the main window's overlay stack.
    SecondaryWindowClosed(iced::window::Id),
    /// Pop a modal out of the main window into its own OS window. Altium
    /// triggers this when the user drags the modal's title bar past the
    /// main window edge, or clicks the pop-out icon in the title bar.
    DetachModal(super::state::ModalId),
    /// Fired after `window::open` resolves for a detached modal — stores
    /// the new window's id in `ui_state.windows` so `view(id)` can render
    /// it and `SecondaryWindowClosed` can reattach when the user dismisses
    /// the window.
    DetachedModalOpened {
        modal: super::state::ModalId,
        id: iced::window::Id,
    },
    /// Pop a document tab into its own OS window (Altium-style tab
    /// undocking). Fires from the tab bar's ↗ button or when a tab drag
    /// crosses the main window edge.
    UndockTab(usize),
    /// `iced::window::open` resolved for an undocked tab — records the
    /// window id so the tab bar hides the tab while its window lives.
    UndockedTabOpened {
        path: std::path::PathBuf,
        id: iced::window::Id,
    },
    /// Reattach an undocked tab to the main window's tab bar. Closing
    /// the secondary window triggers this implicitly via
    /// `SecondaryWindowClosed`; the in-window "Reattach" button emits it
    /// directly.
    ReattachTab(iced::window::Id),
    /// Convert a floating in-app panel into its own OS window. Fires
    /// when the floating panel's drag crosses the main window edge.
    DetachFloatingPanel(usize),
    /// `iced::window::open` resolved for a detached panel — records its
    /// id + panel kind so `view(id)` can render the panel's content.
    DetachedPanelOpened {
        kind: crate::panels::PanelKind,
        id: iced::window::Id,
    },
    /// User pressed on the borderless modal's header — start an OS-level
    /// window drag for the window hosting this modal. Lets the user move
    /// the detached modal even though `decorations: false` removed the
    /// native title bar.
    StartDetachedWindowDrag(super::state::ModalId),
    /// User pressed on empty chrome (menu-bar row outside buttons) — start
    /// an OS-level window drag for the main borderless window. The chrome
    /// is the replacement for the OS title bar.
    StartMainWindowDrag,
    /// User pressed one of the 6 px edge strips around the borderless
    /// main window — ask the OS to start a resize drag in that
    /// direction. Replaces the WS_THICKFRAME edges we lose when
    /// decorations are disabled.
    StartMainWindowResize(iced::window::Direction),
    /// User pressed one of the 6 px edge strips around a borderless
    /// detached modal window — ask the OS to start a resize drag in
    /// that direction. Same trick as the main window; without this
    /// the modals couldn't be resized because `decorations: false`
    /// strips the OS chrome.
    StartDetachedModalResize {
        modal: super::state::ModalId,
        direction: iced::window::Direction,
    },
    /// Custom min/max/close buttons in the borderless main-window chrome.
    MinimizeMainWindow,
    ToggleMaximizeMainWindow,
    CloseMainWindow,
    /// Open the Move Selection dialog (Altium numeric ΔX / ΔY move).
    OpenMoveSelectionDialog,
    CloseMoveSelectionDialog,
    MoveSelectionDxChanged(String),
    MoveSelectionDyChanged(String),
    /// Apply the current ΔX / ΔY to every selected item. Closes the
    /// dialog on success.
    MoveSelectionApply,
    /// Open the F5 Net Color palette.
    OpenNetColorPalette,
    CloseNetColorPalette,
    /// Assign a color to a net label text, or clear the override.
    NetColorSet {
        net: String,
        color: Option<signex_types::theme::Color>,
    },
    /// Open the Parameter Manager dialog (bulk parameter editor).
    OpenParameterManager,
    CloseParameterManager,
    /// Edit a single parameter on a single symbol via the manager.
    ParameterManagerEdit {
        symbol_uuid: uuid::Uuid,
        key: String,
        value: String,
    },
    /// Click on a pin-connection matrix cell: cycle its severity
    /// Error → Warning → Info → Off → (back to baseline default).
    PinMatrixCellCycled {
        row: u8,
        col: u8,
    },
    /// Toggle the "locked against reannotation" flag on a symbol from
    /// inside the Annotate dialog. Locked symbols keep their current
    /// designator even under Reset & Renumber.
    AnnotateToggleLock(uuid::Uuid),
    /// Cycle Altium's rubber-band selection mode
    /// Inside → Outside → TouchingLine → Inside. Bound to Shift+S.
    CycleSelectionMode,
    /// Close the in-flight lasso polygon (Enter key). Commits the
    /// selection if >= 3 vertices, otherwise cancels.
    LassoCommit,
    /// Show / hide the custom net-color picker modal.
    NetColorCustomShow(bool),
    /// Live-update the draft colour as the user drags the picker.
    NetColorCustomDraft(iced::Color),
    /// Commit the draft colour and arm net-colour flood mode.
    NetColorCustomSubmit(iced::Color),
    /// Edit one R/G/B channel of the custom-picker draft via text
    /// input. Parsed as 0-255; invalid values ignored.
    NetColorCustomChannel(Channel, String),
    /// Apply an edit to a placed SchDrawing. Dispatched from the
    /// post-placement Properties panel (Line / Rect / Circle / Arc /
    /// Polygon editable rows). Engine replaces the stored drawing by
    /// uuid with full undo.
    UpdateDrawingField(uuid::Uuid, DrawingFieldEdit),
    /// Open the unified PDF Export overlay (File → Export → PDF…). Now
    /// delegates to `handle_print_preview_requested`, which sets up
    /// `document_state.preview` with the rasterized pages plus every
    /// PDF setting in one modal.
    ExportPdfOpenDialog,
    /// Completion of PDF export — carries either the saved path or error.
    ExportPdfFinished(Result<std::path::PathBuf, String>),
    /// Completion of netlist export — carries either the saved path or error.
    ExportNetlistFinished(Result<std::path::PathBuf, String>),
    /// User invoked File → Export → Bill of Materials… — open the
    /// BOM preview modal instead of going straight to the file
    /// dialog. Mirrors Print Preview.
    ExportBomRequested,
    /// Completion of BOM export — carries either the saved path or error.
    ExportBomFinished(Result<std::path::PathBuf, String>),
    /// User changed BOM grouping (Grouped / Ungrouped / Flat).
    BomPreviewSetGrouping(signex_output::BomGrouping),
    /// User changed BOM output format (CSV / XLSX / HTML).
    BomPreviewSetFormat(signex_output::BomFormat),
    /// User toggled "Include DNP" in the BOM preview modal.
    BomPreviewSetIncludeDnp(bool),
    /// User toggled "Include Not Fitted" in the BOM preview modal.
    BomPreviewSetIncludeNotFitted(bool),
    /// User toggled a single column on / off in the BOM preview
    /// column picker. The handler flips the column's presence in
    /// `BomOptions.columns`, preserving the existing display order
    /// when re-adding so the user's column ordering survives toggles.
    BomPreviewToggleColumn(signex_output::BomColumn),
    /// User picked a variant in the BOM preview variant dropdown.
    /// `None` means the "Base" (no-variant) view.
    BomPreviewSetVariant(Option<String>),
    /// User clicked a column header — set or cycle the sort spec.
    /// First click on a column sorts ascending; same column again
    /// flips to descending; a third click clears the sort and goes
    /// back to rollup order.
    BomPreviewSortColumn(usize),
    /// User started dragging a column header. Carries the source
    /// index in `options.columns`.
    BomPreviewColumnDragStart(usize),
    /// User dropped a dragged column header onto another header.
    /// The source column moves to the destination index, preserving
    /// the user's column order intent.
    BomPreviewColumnDragDrop(usize),
    /// Cursor entered a column header — used by the in-progress
    /// drag-reorder feedback to highlight the drop target.
    BomPreviewColumnHoverEnter(usize),
    /// Cursor left a column header. Clears the hover state for that
    /// idx; the next on_enter on a sibling header replaces it.
    BomPreviewColumnHoverExit(usize),
    /// User pressed a column's right-edge resize handle. Stores
    /// the start x and start width on `BomPreviewState`; subsequent
    /// mouse-move events compute the new width as
    /// `start_width + (current_x - start_x)`.
    BomPreviewColumnResizeStart(usize),
    /// User released the mouse — clears the in-flight resize state.
    BomPreviewColumnResizeEnd,
    /// User clicked a Properties-sidebar tab (General / Columns) in
    /// the BOM preview modal.
    BomPreviewSetSidebarTab(super::state::BomSidebarTab),
    /// User clicked Export in the BOM preview modal — drives the file
    /// dialog with the live options.
    BomPreviewExport,
    /// User dismissed the BOM preview modal.
    BomPreviewClose,
    /// User triggered print preview via Ctrl+P or menu. Open preview dialog.
    PrintPreviewRequested,
    /// User selected a page in the print preview thumbnail list.
    PrintPreviewSelectPage(usize),
    /// User changed preview colour mode.
    PrintPreviewSetColourMode(signex_output::ColourMode),
    /// User changed preview page range to all sheets.
    PrintPreviewSetPageRangeAll,
    /// User changed preview page range to current sheet.
    PrintPreviewSetPageRangeCurrent,
    /// User changed preview page range to one specific page.
    PrintPreviewSetPageRangeSpecific,
    /// User edited the specific page input in preview.
    PrintPreviewSetSpecificPageInput(String),
    /// User toggled "Fit to Page" in the unified PDF preview modal.
    PrintPreviewSetFitToPage(bool),
    /// User toggled "Include Title Block" in the unified PDF preview modal.
    PrintPreviewSetIncludeTitleBlock(bool),
    /// Mouse wheel scrolled over the preview image. Carries the
    /// vertical delta — positive = scroll up = zoom in. Multiplies
    /// `PreviewState.zoom` by `ZOOM_STEP` per notch.
    PrintPreviewZoom(f32),
    /// User clicked the "Export PDF" button in the preview dialog.
    PrintPreviewExport,
    /// User closed the print preview dialog.
    PrintPreviewClose,
    /// User clicked the Preview / Settings tab inside the unified
    /// Export PDF modal.
    PrintPreviewSetTab(super::state::PdfPreviewTab),
    /// User pressed mouse-down on the preview viewport — kicks off
    /// pan-drag. The handler reads the cursor from
    /// `interaction_state.last_mouse_pos` rather than carrying it on
    /// the message; iced builds messages eagerly at view-render time
    /// so embedded coords would be one frame stale.
    PrintPreviewPanStart,
    /// User released the pan drag — clears `panning`.
    PrintPreviewPanFinished,
    /// User toggled a project file in the Settings → Files list.
    PrintPreviewToggleFile(std::path::PathBuf),
    /// Select all project files in the Settings → Files list.
    PrintPreviewSelectAllFiles,
    /// Deselect all project files (effectively "no override —
    /// fall back to all").
    PrintPreviewClearAllFiles,
    /// Variant picker dropdown — None = Base.
    PrintPreviewSetVariant(Option<String>),
    PrintPreviewSetUsePhysicalStructure(bool),
    PrintPreviewSetPhysicalDesignators(bool),
    PrintPreviewSetPhysicalNetLabels(bool),
    PrintPreviewSetPhysicalPorts(bool),
    PrintPreviewSetPhysicalSheetNumber(bool),
    PrintPreviewSetPhysicalDocumentNumber(bool),
    PrintPreviewSetIncludeNoErcMarkers(bool),
    PrintPreviewSetIncludeParameterSets(bool),
    PrintPreviewSetIncludeProbes(bool),
    PrintPreviewSetIncludeBlankets(bool),
    PrintPreviewSetIncludeNotes(bool),
    PrintPreviewSetIncludeCollapsedNotes(bool),
    PrintPreviewSetQuality(super::state::PdfQuality),
    PrintPreviewSetBookmarkZoom(f32),
    PrintPreviewSetGenerateNetsInfo(bool),
    PrintPreviewSetBookmarkPins(bool),
    PrintPreviewSetBookmarkNetLabels(bool),
    PrintPreviewSetBookmarkPorts(bool),
    PrintPreviewSetIncludeComponentParameters(bool),
    PrintPreviewSetGlobalBookmarks(bool),
    PrintPreviewSetPcbColourMode(signex_output::ColourMode),
    /// User clicked the OK button on the export-error modal.
    DismissExportError,
    /// v0.9 Library subsystem message — folded under one variant so
    /// the dispatcher can route to `library_dispatch::handle` in one
    /// shot. See `crate::library::LibraryMessage` for the inner
    /// shape.
    Library(crate::library::LibraryMessage),
    /// Open the command palette dropdown and focus the chrome-strip
    /// search bar. Bound to Ctrl+Shift+P. Idempotent — already-open
    /// keeps state, just refocuses the input.
    CommandPaletteOpen,
    /// Close the dropdown without executing. Bound to Esc and to
    /// click-outside. Leaves the chrome-strip input visible (it's the
    /// always-on placeholder) but unfocused; query is preserved so a
    /// re-open continues where the user left off.
    CommandPaletteClose,
    /// Live query update from the chrome-strip text_input. Resets the
    /// selected row to 0 because the result list reorders on every
    /// keystroke.
    CommandPaletteQueryChanged(String),
    /// Move the highlighted row by `delta` (clamped to result count).
    /// Wired to ArrowUp / ArrowDown when the palette is open.
    CommandPaletteMoveSelection(i32),
    /// Click on a specific row in the dropdown — sets selected_index
    /// and executes in one shot.
    CommandPaletteSelect(usize),
    /// Execute the currently selected entry. Wired to Enter and to
    /// `text_input::on_submit`.
    CommandPaletteExecuteSelected,
    Noop,
}

/// Per-shape edit descriptor. The Properties panel dispatches one of
/// these per numeric text-input edit; the handler looks up the stored
/// drawing, applies the field change, and emits
/// `Command::UpdateSchDrawing` with the patched variant.
#[derive(Debug, Clone, Copy)]
pub enum DrawingFieldEdit {
    Width(f64),
    Fill(signex_types::schematic::FillType),
    LineStartX(f64),
    LineStartY(f64),
    LineEndX(f64),
    LineEndY(f64),
    RectStartX(f64),
    RectStartY(f64),
    RectWidthMm(f64),
    RectHeightMm(f64),
    CircleCenterX(f64),
    CircleCenterY(f64),
    CircleRadius(f64),
    ArcCenterX(f64),
    ArcCenterY(f64),
    ArcRadius(f64),
    ArcStartAngle(f64),
    ArcEndAngle(f64),
    /// Override the stroke colour; `None` restores the theme default.
    StrokeColor(Option<signex_types::schematic::StrokeColor>),
}

/// R / G / B channel selector for the custom net-colour picker inputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    R,
    G,
    B,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum ContextAction {
    Copy,
    Cut,
    Paste,
    SmartPaste,
    OpenChildSheet,
    Delete,
    SelectAll,
    ZoomFit,
    RotateSelected,
    MirrorX,
    MirrorY,
    /// Run an Active Bar action from a context-menu submenu (Place /
    /// Align). Closes both menus and dispatches the action through the
    /// existing Active Bar handler so all the placement / transform
    /// logic stays in one place.
    ActiveBar(crate::active_bar::ActiveBarAction),
}

/// Which click-to-open submenu is currently expanded inside the right-
/// click context menu, if any. Owned by `InteractionState` and cleared
/// alongside `context_menu` whenever the menu closes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextSubmenu {
    Place,
    Align,
    /// Project-tree → "Add New to Project ›" launcher. Items are
    /// version-tagged placeholders today — actual document creation
    /// lands with project-write support in v0.9.
    AddNewToProject,
}

#[derive(Debug, Clone)]
pub struct TextEditState {
    pub uuid: uuid::Uuid,
    pub kind: signex_types::schematic::SelectedKind,
    pub text: String,
    pub original_text: String,
    /// World-space position of the object being edited (mm). Converted to
    /// screen coords at render time so the inline editor tracks pan/zoom.
    pub world_x: f64,
    pub world_y: f64,
}

#[derive(Debug, Clone)]
pub struct ContextMenuState {
    pub x: f32,
    pub y: f32,
}

/// State for the Projects-panel tree-view right-click menu. The menu's
/// action set is computed from `path` (leaf vs branch vs empty) at render
/// time, so we only need to store the anchor coordinates + the clicked
/// path (or `None` for the background menu).
#[derive(Debug, Clone)]
pub struct ProjectTreeContextMenuState {
    pub x: f32,
    pub y: f32,
    /// `Some(path)` = right-click on a specific node; `None` = right-click
    /// in empty tree area, offering only the generic actions.
    pub path: Option<Vec<usize>>,
}

/// State for the "Close Project — Unsaved Edits" confirmation modal.
/// Opens only when the user closes a project that has at least one
/// entry in `DocumentState.dirty_paths` rooted in the project's
/// directory; the modal lists every dirty file by filename so the
/// user can see what they're about to lose.
#[derive(Debug, Clone)]
pub struct ProjectCloseConfirmState {
    /// Project root tree path the close was requested for. Stored so
    /// the modal's confirm action can dispatch back to
    /// `close_project_at_tree_path` without re-resolving from the
    /// project list (which may shift if the user closes another
    /// project while this modal is up — Altium's modal is dismiss-
    /// only, so this is defence-in-depth).
    pub tree_path: Vec<usize>,
    /// Project display name shown in the modal header.
    pub project_name: String,
    /// Absolute paths of dirty files inside the project's directory.
    /// The view layer renders the file basenames; the handler uses
    /// the absolute paths to locate the engines for save / discard.
    pub dirty_paths: Vec<std::path::PathBuf>,
}

/// User choice from the project-close confirmation modal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectCloseChoice {
    /// Save every dirty file in the project, then close.
    SaveAll,
    /// Drop the engines for every dirty file in the project without
    /// writing to disk, then close.
    DiscardAll,
    /// Dismiss the modal; the project stays open.
    Cancel,
}

/// State for the document-tab right-click menu. The menu's items are
/// derived from `tab_idx` (the clicked tab) so the same menu builder
/// works for any tab; mutually exclusive with the canvas and project-
/// tree context menus.
#[derive(Debug, Clone)]
pub struct TabContextMenuState {
    pub x: f32,
    pub y: f32,
    pub tab_idx: usize,
}

/// Concrete actions dispatched when the user picks a menu item in the
/// document-tab right-click menu.
#[derive(Debug, Clone)]
pub enum TabContextAction {
    /// Close just this tab.
    Close(usize),
    /// Close every tab except the one at this index.
    CloseAllOthers(usize),
    /// Close every open tab.
    CloseAll,
    /// Pop the tab at this index into its own OS window.
    Undock(usize),
}

/// Concrete actions dispatched when the user picks a menu item in the
/// Projects-panel tree-view context menu.
#[derive(Debug, Clone)]
pub enum ProjectTreeAction {
    /// Open the file backed by this leaf in the current document slot.
    OpenNode(Vec<usize>),
    /// Expand (or collapse) a specific branch node.
    ToggleNode(Vec<usize>),
    /// Recursively expand every node in the tree.
    ExpandAll,
    /// Recursively collapse every node in the tree.
    CollapseAll,
    /// Re-scan the project and rebuild the tree from current state.
    Refresh,
    /// Close every open document tab without closing the project
    /// itself. Fired from the project-root "Close Project Documents"
    /// menu item.
    CloseAllDocuments,
    /// Reveal a file (leaf click) or the project directory (root
    /// click) in the OS file manager. The tree path's first index
    /// picks which project's directory the operation resolves
    /// against — leaves nested under project B reveal in B's dir
    /// even when project A is active. A single-element path means
    /// the project root row was clicked.
    RevealInExplorer(Vec<usize>),
    /// Fire the print preview flow — only surfaced on leaves that are
    /// already the active tab.
    PrintActive,
    /// Open the sheet-rename modal for this leaf, preloaded with the
    /// current filename.
    OpenRenameDialog(Vec<usize>),
    /// Open the "Remove from Project" modal (Delete / Exclude / Cancel)
    /// for this leaf.
    OpenRemoveDialog(Vec<usize>),
    /// Close the entire project whose root is at this tree path. Closes
    /// every open tab backed by the project, drops the `LoadedProject`
    /// from the workspace, and promotes another project (or `None`) to
    /// active. The tree path's first index selects the project; other
    /// indices are ignored so the action is safe to fire from any node
    /// underneath a project root.
    CloseProject(Vec<usize>),
}

/// State for the rename modal. Tracks the target file, the live
/// edit buffer, and the clicked tree path so we can rebuild the tree
/// after a successful rename without rediscovering the project.
#[derive(Debug, Clone)]
pub struct RenameDialogState {
    pub target_path: std::path::PathBuf,
    pub tree_path: Vec<usize>,
    pub buffer: String,
    pub error: Option<String>,
}

/// State for the "Remove from Project" modal. `Delete` removes the file
/// from disk; `Exclude` drops it from the session's sheet list but
/// leaves the file in place.
#[derive(Debug, Clone)]
pub struct RemoveDialogState {
    pub target_path: std::path::PathBuf,
    pub tree_path: Vec<usize>,
    pub display_name: String,
}

/// User choice from the Remove-from-Project modal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoveChoice {
    /// Remove from project AND delete the file on disk.
    DeleteFile,
    /// Remove from project; leave the file in its folder.
    ExcludeFromProject,
}

#[derive(Debug, Clone)]
pub enum StatusBarRequest {
    CycleUnit,
    ToggleGrid,
    ToggleSnap,
    TogglePanelList,
    /// Click on the selection-summary segment opens the Properties panel
    /// scoped to the current selection.
    OpenPropertiesForSelection,
}
