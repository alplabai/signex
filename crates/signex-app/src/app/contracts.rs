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
    /// User picked an option in the close-confirmation modal shown when they
    /// try to close a tab with unsaved changes. Drives the modal via
    /// `ui_state.close_tab_confirm`.
    CloseTabConfirm(CloseTabChoice),
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
    /// Open the PDF export options dialog (user clicks File → Export → PDF…).
    /// Replaces the old direct-to-file flow.
    ExportPdfOpenDialog,
    /// User changed page size in the PDF options dialog.
    ExportPdfSetPageSize(signex_output::PageSize),
    /// User changed orientation in the PDF options dialog.
    ExportPdfSetOrientation(signex_output::Orientation),
    /// User changed colour mode in the PDF options dialog.
    ExportPdfSetColourMode(signex_output::ColourMode),
    /// User changed sheet template in the PDF options dialog. None = no template.
    ExportPdfSetTemplate(Option<signex_output::TemplateId>),
    /// User toggled the "Fit to Page" checkbox in the PDF options dialog.
    ExportPdfSetFitToPage(bool),
    /// User toggled the "Include Title Block" checkbox in the PDF options dialog.
    ExportPdfSetIncludeTitleBlock(bool),
    /// User changed page-range mode to export all sheets.
    ExportPdfSetPageRangeAll,
    /// User changed page-range mode to export only the active sheet.
    ExportPdfSetPageRangeCurrent,
    /// User changed page-range mode to export one specific page number.
    ExportPdfSetPageRangeSpecific,
    /// User edited the specific page number input in the PDF options dialog.
    ExportPdfSetSpecificPageInput(String),
    /// User clicked Cancel in the PDF options dialog.
    ExportPdfDialogCancel,
    /// User clicked Export in the PDF options dialog — proceed with file
    /// picker and export using the dialog's options.
    ExportPdfDialogConfirm,
    /// Completion of PDF export — carries either the saved path or error.
    ExportPdfFinished(Result<std::path::PathBuf, String>),
    /// Completion of netlist export — carries either the saved path or error.
    ExportNetlistFinished(Result<std::path::PathBuf, String>),
    /// User invoked File → Export → Bill of Materials…
    ExportBomRequested,
    /// Completion of BOM export — carries either the saved path or error.
    ExportBomFinished(Result<std::path::PathBuf, String>),
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
    /// User clicked the "Export PDF" button in the preview dialog.
    PrintPreviewExport,
    /// User closed the print preview dialog.
    PrintPreviewClose,
    /// User clicked the OK button on the export-error modal.
    DismissExportError,
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

#[derive(Debug, Clone, Copy)]
pub enum CloseTabChoice {
    SaveAndClose,
    DiscardAndClose,
    Cancel,
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

#[derive(Debug, Clone)]
pub enum StatusBarRequest {
    CycleUnit,
    ToggleGrid,
    ToggleSnap,
    TogglePanelList,
}
