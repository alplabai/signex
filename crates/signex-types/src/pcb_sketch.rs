//! Board geometry baked from a board sketch (#2234): keepout regions,
//! cutouts and mounting holes.
//!
//! The board sketch ([`crate::sketch::SketchData`]) is the source of truth;
//! these are what a bake produces from it and what DRC and the ratsnest
//! read. Coordinates are board millimetres, like [`crate::pcb::PcbBoard`]'s
//! outline. Data only: nothing writes or persists them yet.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::pcb::Point;
use crate::pcb_layer::PcbLayer;
use crate::sketch::attr::KeepoutKinds;

/// A region of the board that forbids some object types on some layers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PcbKeepout {
    pub uuid: Uuid,
    /// Closed boundary, without the closing vertex repeated. A circle is
    /// tessellated by the bake.
    pub boundary: Vec<Point>,
    /// The layers the region applies to, written as board-layer tokens.
    #[serde(with = "crate::sketch::layer_serde::list")]
    pub layers: Vec<PcbLayer>,
    /// What the region forbids: the full set, so a keepout that forbids
    /// both tracks and vias stays exactly that.
    pub forbids: KeepoutKinds,
}

/// An internal opening in the board.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PcbCutout {
    pub uuid: Uuid,
    /// Closed boundary, without the closing vertex repeated.
    pub boundary: Vec<Point>,
    /// Corner radius of the routed edge, in mm; 0 for a sharp edge.
    #[serde(default)]
    pub edge_radius: f64,
    /// Whether the opening goes through the whole board.
    #[serde(default = "default_through")]
    pub through: bool,
}

fn default_through() -> bool {
    true
}

/// A mounting hole.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PcbHole {
    pub uuid: Uuid,
    /// Centre of the hole.
    pub position: Point,
    /// Drill diameter in mm.
    pub drill: f64,
    /// Whether the hole is plated.
    #[serde(default)]
    pub plated: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pcb_layer::CopperLayer;

    fn square() -> Vec<Point> {
        [(0.0, 0.0), (5.0, 0.0), (5.0, 5.0), (0.0, 5.0)]
            .into_iter()
            .map(|(x, y)| Point { x, y })
            .collect()
    }

    #[test]
    fn a_keepout_keeps_every_forbidden_kind_through_serde() {
        let keepout = PcbKeepout {
            uuid: Uuid::now_v7(),
            boundary: square(),
            layers: vec![PcbLayer::Copper(CopperLayer::Top)],
            forbids: KeepoutKinds {
                no_routing: true,
                no_vias: true,
                ..KeepoutKinds::default()
            },
        };
        let json = serde_json::to_string(&keepout).unwrap();
        assert!(json.contains(r#""layers":["TopCopper"]"#), "{json}");
        assert_eq!(serde_json::from_str::<PcbKeepout>(&json).unwrap(), keepout);
    }

    #[test]
    fn a_cutout_and_a_hole_round_trip_and_default_sensibly() {
        let cutout = PcbCutout {
            uuid: Uuid::now_v7(),
            boundary: square(),
            edge_radius: 0.5,
            through: true,
        };
        let json = serde_json::to_string(&cutout).unwrap();
        assert_eq!(serde_json::from_str::<PcbCutout>(&json).unwrap(), cutout);

        let bare = format!(r#"{{"uuid":"{}","boundary":[]}}"#, Uuid::nil());
        let read: PcbCutout = serde_json::from_str(&bare).unwrap();
        assert!(read.through, "a cutout goes through unless it says not");
        assert_eq!(read.edge_radius, 0.0);

        let hole = PcbHole {
            uuid: Uuid::now_v7(),
            position: Point { x: 4.0, y: 4.0 },
            drill: 3.2,
            plated: false,
        };
        let json = serde_json::to_string(&hole).unwrap();
        assert_eq!(serde_json::from_str::<PcbHole>(&json).unwrap(), hole);
    }
}
