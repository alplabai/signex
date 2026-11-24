//! What the Properties Bake panel of a board sketch shows while its bake
//! preview is open (mockup row 179): the values Bake writes to the board, or
//! why the sketch does not bake. Built by the board sketch session from the
//! current solve; the view is `footprint_editor_properties/bake_panel.rs`.

use signex_sketch::id::SketchEntityId;
use signex_types::pcb_layer::PcbLayer;

use crate::library::editor::footprint::GapOffer;

/// The open bake preview of a board sketch.
#[derive(Debug, Clone, PartialEq)]
pub enum BoardBakePreview {
    /// What Bake writes to the board.
    Writes(BoardBakeRows),
    /// Why the sketch does not bake now.
    Refused(BoardBakeRefusal),
}

/// Why the open preview's sketch does not bake.
#[derive(Debug, Clone, PartialEq)]
pub struct BoardBakeRefusal {
    /// The bake's reason; `None` when the sketch is not solved.
    pub reason: Option<String>,
    /// The entity the reason names: a click on the line selects it.
    pub place: Option<SketchEntityId>,
    /// An open profile's gap, with Close gap when it heals.
    pub gap: Option<GapOffer>,
}

/// The values Bake writes, target by target. An empty list writes nothing
/// to its target and shows no row.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BoardBakeRows {
    /// `None`: the sketch draws no outline and the board keeps its own.
    pub outline: Option<BakedOutline>,
    pub holes: Vec<BakedHole>,
    /// The layers of each keepout.
    pub keepouts: Vec<Vec<PcbLayer>>,
    /// Cutouts that are not slots; a slot is counted under `slots` only.
    pub cutouts: usize,
    /// Each slot's `(length, width)` in mm.
    pub slots: Vec<(f64, f64)>,
    pub v_scores: usize,
    pub pours: usize,
    pub mechanical: usize,
    /// The designator each component anchor moves.
    pub anchors: Vec<String>,
}

/// The outline Bake writes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BakedOutline {
    /// Its extent, `(width, height)` in mm.
    Size(f64, f64),
    /// The sketch's outline was deleted: Bake removes the board's.
    Removed,
}

/// A mounting hole Bake drills.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BakedHole {
    pub drill_mm: f64,
    pub plated: bool,
}
