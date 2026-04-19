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
    Tab(TabMessage),
    Dock(DockMessage),
    StatusBar(StatusBarRequest),
    CanvasEvent(CanvasEvent),
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
    ContextAction(ContextAction),
    OpenPreferences,
    OpenFind,
    OpenReplace,
    ClosePreferences,
    PreferencesNav(crate::preferences::PrefNav),
    PreferencesMsg(crate::preferences::PrefMsg),
    FindReplaceMsg(crate::find_replace::FindReplaceMsg),
    WindowResized(f32, f32),
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
    Noop,
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
    Delete,
    SelectAll,
    ZoomFit,
    RotateSelected,
    MirrorX,
    MirrorY,
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
