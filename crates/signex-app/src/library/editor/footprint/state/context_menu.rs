//! v0.26 — right-click canvas context menu state.

use super::pad::AlignOp;

/// `(x, y)` are **window-absolute** screen coords (already include
/// menu-bar + tab-bar offsets). `target` records what the cursor was
/// over at right-click time so the renderer can pick between the
/// empty-canvas variant and the on-pad variant. `submenu` tracks
/// which submenu (if any) is expanded in place.
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
    /// Pad operations (rotate, flip layer, the planned custom-pad rows).
    PadActions,
    /// The align and distribute operations on the pad selection.
    Align,
}

/// v0.26 — actions issued from the context menu that don't already
/// have a dedicated `FootprintEditorMsg` variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FootprintContextAction {
    SelectAllPads,
    DeselectAll,
    FitToWindow,
    /// A row of the menu on a pad: closes the menu, then runs the action.
    OnSelection(SelectionAction),
}

/// The rows of the menu on a pad that act on the selection. Each runs the
/// same message as its active-bar row; the menu closes first so a dialog
/// it opens is not drawn under the menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionAction {
    /// Open the Move Selection By dialog.
    MoveBy,
    /// Open the Align Selection dialog.
    AlignDialog,
    /// One align or distribute operation.
    Align(AlignOp),
    /// Snap the selection to the grid.
    AlignToGrid,
    /// Rotate the selection 90° counter-clockwise.
    Rotate,
    /// Flip the selection to the other side.
    Flip,
    /// Arm the sketch's Rectangular Pattern tool.
    RectangularPattern,
    /// Switch the editor to Sketch mode.
    EditSketch,
}
