//! v0.26 — right-click canvas context menu state.

/// `(x, y)` are **window-absolute** screen coords (already include
/// menu-bar + tab-bar offsets). `target` records what the cursor was
/// over at right-click time so the renderer can pick between the
/// empty-canvas variant and the on-pad variant. `submenu` tracks
/// which submenu (if any) is hover-expanded.
#[derive(Debug, Clone)]
pub struct FootprintContextMenuState {
    pub x: f32,
    pub y: f32,
    pub target: FootprintContextTarget,
    pub submenu: Option<FootprintContextSubmenu>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FootprintContextTarget {
    /// Right-clicked on bare canvas (no pad / silk / sketch hit).
    Empty,
    /// Right-clicked on pad index `idx` in `state.pads`.
    Pad(usize),
    /// Right-clicked on silk-front graphic index `idx` in `silk_f`.
    SilkF(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FootprintContextSubmenu {
    Place,
    View,
    Selection,
    /// v0.26-G — Altium-parity Pad Actions submenu.
    PadActions,
}

/// v0.26 — actions issued from the context menu that don't already
/// have a dedicated `FootprintEditorMsg` variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FootprintContextAction {
    SelectAllPads,
    DeselectAll,
    FitToWindow,
}
