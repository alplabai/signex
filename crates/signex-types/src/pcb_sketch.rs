//! Board geometry baked from a board sketch (#2234): the outline, keepout
//! regions, cutouts, mounting holes and V-scores.
//!
//! The board sketch ([`crate::sketch::SketchData`]) is the source of truth;
//! these are what a bake (`signex_bake::bake_board`) produces from it and
//! what DRC reads. Coordinates are board millimetres, like
//! [`crate::pcb::PcbBoard`]'s outline. A board holds them in
//! [`crate::pcb::PcbBoard::sketch`], written only by the board engine's
//! `SetBoardSketch` command. See `docs/PCB_SKETCH.md`.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::format::UnknownColumns;
use crate::pcb::Point;
use crate::pcb_layer::PcbLayer;
use crate::sketch::SketchData;
use crate::sketch::attr::{KeepoutKinds, VScoreSide};
use crate::sketch::baked_kinds::BakedKind;

/// What a bake of a board sketch produced. Each item takes the uuid of the
/// sketch entity it came from (the seed edge or the circle), so baking an
/// unchanged sketch again gives an equal value.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BakedBoard {
    /// The board outline when the sketch owns it: the closed profile, or an
    /// empty list once the sketch's outline was deleted after a bake.
    /// `None`: the sketch draws no outline and the board keeps its own.
    pub outline: Option<Vec<Point>>,
    pub keepouts: Vec<PcbKeepout>,
    pub cutouts: Vec<PcbCutout>,
    pub holes: Vec<PcbHole>,
    pub v_scores: Vec<PcbVScore>,
}

/// A board sketch as the board stores it, with what it baked to. The baked
/// outline is not here: it is the board's own `outline`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BoardSketch {
    pub sketch: SketchData,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keepouts: Vec<PcbKeepout>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cutouts: Vec<PcbCutout>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub holes: Vec<PcbHole>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub v_scores: Vec<PcbVScore>,
}

impl BoardSketch {
    /// Store `sketch` with what it baked to, and hand back the outline to
    /// write to the board ([`BakedBoard::outline`]). Records in the sketch
    /// whether it baked an outline, so deleting that outline later still
    /// clears the board's.
    pub fn from_bake(mut sketch: SketchData, baked: BakedBoard) -> (Self, Option<Vec<Point>>) {
        let has_outline = baked.outline.as_ref().is_some_and(|o| !o.is_empty());
        sketch.record_baked(BakedKind::BoardOutline, has_outline);
        let stored = Self {
            sketch,
            keepouts: baked.keepouts,
            cutouts: baked.cutouts,
            holes: baked.holes,
            v_scores: baked.v_scores,
        };
        (stored, baked.outline)
    }

    /// Whether the board outline is the sketch's to write.
    pub fn owns_outline(&self) -> bool {
        self.sketch.owns_kind(BakedKind::BoardOutline)
    }
}

/// A V-score: a straight scoring line across the board.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PcbVScore {
    pub uuid: Uuid,
    pub from: Point,
    pub to: Point,
    /// Depth of the cut as a fraction of the board thickness, `0..=1`.
    pub depth_fraction: f64,
    /// Thinnest web the cut must leave, in mm; 0 when unset.
    #[serde(default)]
    pub min_web: f64,
    #[serde(default)]
    pub side: VScoreSide,
}

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
    /// The net a plated hole joins (for example GND), or `None` for an
    /// isolated or unplated hole (Caner, 2026-11-09).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub net: Option<u32>,
    /// TSV columns this build does not recognise, kept per row so a save
    /// re-emits them (see [`UnknownColumns`]). A bake writes none.
    #[serde(default, skip_serializing_if = "UnknownColumns::is_empty")]
    pub unknown_columns: UnknownColumns,
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
            net: None,
            unknown_columns: UnknownColumns::default(),
        };
        let json = serde_json::to_string(&hole).unwrap();
        assert!(
            !json.contains("net"),
            "an isolated hole writes no net: {json}"
        );
        assert_eq!(serde_json::from_str::<PcbHole>(&json).unwrap(), hole);

        let grounded = PcbHole {
            plated: true,
            net: Some(1),
            ..hole
        };
        let json = serde_json::to_string(&grounded).unwrap();
        assert_eq!(serde_json::from_str::<PcbHole>(&json).unwrap(), grounded);
    }
}
