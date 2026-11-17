//! The board's free primitives (#2373): arcs, pads and dimensions placed
//! on the board itself rather than in a footprint. Each list is its own
//! object kind with its own uuid per item; they share one `PcbBoard` field
//! so the board's other lists keep their shape. Saved in the optional
//! `.snxpcb` `[free]` section (`format::board_free`).
//!
//! A free pad holds its board position and turn directly. Code that places
//! footprint pads reads it through [`free_pad_frame`], a footprint at the
//! board origin with no turn, so a free pad is drawn, hit and checked by
//! the same geometry as a footprint's.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::pcb::{ComponentAttributes, Footprint, Pad, Point};
use crate::pcb_arc::TrackArc;
use crate::pcb_dimension::LinearDimension;

/// The free arcs, pads and dimensions of a board.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FreePrimitives {
    /// Arcs on any layer: copper arcs are tracks of their net, arcs on
    /// other layers are drawings (net 0).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub arcs: Vec<TrackArc>,
    /// Pads with no footprint, at their board position and turn.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pads: Vec<Pad>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dimensions: Vec<LinearDimension>,
}

impl FreePrimitives {
    /// Whether the board has none: the file then has no `[free]` section.
    pub fn is_empty(&self) -> bool {
        self.arcs.is_empty() && self.pads.is_empty() && self.dimensions.is_empty()
    }

    /// The uuid of every free item, for the board's one id space.
    pub fn uuids(&self) -> impl Iterator<Item = Uuid> + '_ {
        self.arcs
            .iter()
            .map(|a| a.uuid)
            .chain(self.pads.iter().map(|p| p.uuid))
            .chain(self.dimensions.iter().map(|d| d.uuid))
    }
}

/// The frame a free pad is placed in: a footprint at the board origin, not
/// turned, on the top side, holding no pads. Its uuid is nil, which no
/// board item uses.
pub fn free_pad_frame() -> Footprint {
    Footprint {
        uuid: Uuid::nil(),
        reference: String::new(),
        value: String::new(),
        footprint_id: String::new(),
        position: Point::new(0.0, 0.0),
        rotation: 0.0,
        layer: "TopCopper".into(),
        locked: false,
        pads: Vec::new(),
        graphics: Vec::new(),
        properties: Vec::new(),
        schematic_link: None,
        library_ref: None,
        component: Box::new(ComponentAttributes::default()),
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}
