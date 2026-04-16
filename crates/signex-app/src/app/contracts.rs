use std::path::PathBuf;

use signex_types::schematic::SchematicSheet;
use signex_types::theme::ThemeId;

use crate::canvas::CanvasEvent;
use crate::dock::DockMessage;
use crate::menu_bar::MenuMessage;
use crate::tab_bar::TabMessage;
use crate::toolbar::ToolMessage;

use super::selection_message::SelectionMessage;

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
    StatusBar(StatusBarMsg),
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
    Selection(SelectionMessage),
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
    Noop,
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
    pub screen_x: f32,
    pub screen_y: f32,
}

#[derive(Debug, Clone)]
pub struct ContextMenuState {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone)]
pub enum StatusBarMsg {
    CycleUnit,
    ToggleGrid,
    ToggleSnap,
    TogglePanelList,
}