#[derive(Debug, Clone)]
pub enum SelectionRequest {
    SelectAll,
    /// Empty the selection (`unselect_all`).
    ClearAll,
    StoreSlot {
        slot: usize,
    },
    RecallSlot {
        slot: usize,
    },
    HitAt {
        world_x: f64,
        world_y: f64,
    },
    BoxSelect {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        direction: crate::canvas::BoxDirection,
    },
    /// Expand `seed` (a wire, junction, label or bus) to the net geometry
    /// connected to it, as `signex_net::connected_net_items` derives it.
    /// Symbols are not included.
    SelectConnected {
        seed: signex_types::schematic::SelectedItem,
    },
    /// Start a move-drag on the next click regardless of whether the click
    /// lands on an already-selected item — used for the Active Bar "Drag"
    /// action. Cleared on Escape or after the next click commits.
    ArmDrag,
    /// `drag_keep_connections` (G): the next object picked is dragged with
    /// its wires attached, whatever "Keep wires connected when moving" says.
    DragKeepingConnections,
    /// The Find Similar Objects dialog (#1257).
    FindSimilar(crate::find_similar::FindSimilarMsg),
    /// Cycle the rubber-band selection mode (Inside / Touching). Bound
    /// to Shift+S.
    CycleMode,
    /// Enter key: commit the armed multi-click buffer (polyline or
    /// lasso polygon).
    LassoCommit,
}
