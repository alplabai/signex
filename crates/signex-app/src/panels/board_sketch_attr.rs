//! Properties panel edits of a board sketch entity's board attributes
//! (#2234). Keepouts and cutouts use the footprint Role messages.

use signex_sketch::id::SketchEntityId;

/// The user changed what a board sketch entity becomes on the board.
#[derive(Debug, Clone, PartialEq)]
pub enum BoardSketchAttrMsg {
    /// The edge was put into or taken out of the board outline.
    OutlineToggled { id: SketchEntityId, on: bool },
    /// The circle was made a mounting hole, or no longer one.
    MountingHoleToggled { id: SketchEntityId, on: bool },
    /// The hole's drill expression was edited; empty means the circle's
    /// diameter.
    HoleDrillEdited { id: SketchEntityId, value: String },
    /// The hole was made plated or unplated.
    HolePlatedToggled { id: SketchEntityId, on: bool },
    /// The hole's net name was edited; empty means no net.
    HoleNetEdited { id: SketchEntityId, value: String },
}
