#[derive(Debug, Clone)]
pub enum SelectionRequest {
    SelectAll,
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
    },
    /// Click-hit a wire/bus; expand to every wire/bus/junction/label that is
    /// connected to it by shared endpoints (transitive). Labels attached to a
    /// wire anchor are included; symbols are not — only the net geometry.
    SelectConnected {
        world_x: f64,
        world_y: f64,
    },
    /// Start a move-drag on the next click regardless of whether the click
    /// lands on an already-selected item — used for the Active Bar "Drag"
    /// action. Cleared on Escape or after the next click commits.
    ArmDrag,
}
