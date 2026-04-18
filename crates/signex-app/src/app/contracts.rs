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
