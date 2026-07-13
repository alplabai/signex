//! Dock data model — panel positions, messages, and region/area state.

use crate::panels::{PanelKind, PanelMsg};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelPosition {
    Left,
    Right,
    Bottom,
}

#[derive(Debug, Clone)]
pub enum DockMessage {
    SelectTab(PanelPosition, usize),
    ToggleCollapse(PanelPosition),
    ClosePanel(PanelPosition, usize),
    /// Undock a panel to floating (drag tab out).
    UndockPanel(PanelPosition, usize),
    /// Mouse down on a tab — arms drag-to-undock detection.
    TabDragStart(PanelPosition, usize),
    /// Mouse released on a tab — if no undock happened, treat as click → select.
    TabClick(PanelPosition, usize),
    /// Reorder tabs within a dock region. `from` is the dragged tab's
    /// original index, `to` is the index of the tab it was released
    /// on. Currently produced by the internal TabClick handler — not
    /// emitted directly by the UI yet (left available for a future
    /// pointer-tracking drop indicator).
    #[allow(dead_code)]
    ReorderTab {
        pos: PanelPosition,
        from: usize,
        to: usize,
    },
    /// Scroll tabs left/right when they overflow the panel width.
    TabScroll(PanelPosition, i32),
    /// Pointer entered a dock tab — feeds the hover highlight on
    /// inactive tabs. Container-style tabs can't read `button::Status`,
    /// so we track the hovered tab in `DockArea::hovered_tab` via
    /// `mouse_area::on_enter` / `on_exit`.
    TabHoverEnter(PanelPosition, usize),
    /// Pointer left the named dock tab. Carries the tab's coords so we
    /// only clear `hovered_tab` when the exit matches the currently
    /// hovered tab — otherwise a fast move from A to B where iced
    /// fires `on_enter(B)` before `on_exit(A)` would blow the new
    /// hover away and leave the highlight stuck off.
    TabHoverExit(PanelPosition, usize),
    /// Move a floating panel by delta.
    #[allow(dead_code)]
    MoveFloating(usize, f32, f32),
    /// Start dragging a floating panel.
    StartDragFloating(usize),
    /// Mouse released after dragging a floating panel — try to dock at mouse pos.
    FloatingDragEnd(usize),
    /// Re-dock a floating panel (close floating → add to right dock).
    DockFloating(usize),
    /// Re-dock a floating panel to a specific region.
    DockFloatingTo(usize, PanelPosition),
    Panel(PanelMsg),
    /// v0.9 Library panel message — wraps `crate::library::LibraryMessage`
    /// when the Library panel is active inside a dock region. The
    /// dispatcher unwraps this back into `Message::Library(_)`.
    Library(crate::library::LibraryMessage),
}

/// A panel floating as an overlay window.
#[derive(Debug, Clone)]
pub struct FloatingPanel {
    pub kind: PanelKind,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub dragging: bool,
}

pub(super) struct DockRegion {
    pub(super) panels: Vec<PanelKind>,
    pub(super) active: usize,
    pub(super) collapsed: bool,
    /// First visible tab index (for overflow scrolling).
    pub(super) tab_offset: usize,
}

pub struct DockArea {
    pub(super) left: DockRegion,
    pub(super) right: DockRegion,
    pub(super) bottom: DockRegion,
    pub floating: Vec<FloatingPanel>,
    /// Active tab drag: (region, tab index). Set on mouse-down, cleared on release or undock.
    pub tab_drag: Option<(PanelPosition, usize)>,
    /// Dock tab currently under the pointer, for Altium-style hover
    /// highlight on inactive tabs. `None` when no tab is hovered.
    pub hovered_tab: Option<(PanelPosition, usize)>,
}
