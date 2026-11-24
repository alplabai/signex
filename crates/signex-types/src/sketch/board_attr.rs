//! The board-only bake targets of a sketch entity (decision 2 of the board
//! sketch, Caner 2026-11-20): Slot, Mechanical layer and Component anchor,
//! and the board items a bake of them (and of a board pour) writes. A board
//! pour reuses [`crate::sketch::attr::PourAttr`]. See `docs/PCB_SKETCH.md`,
//! "Targets".

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::pcb::{BoardGraphic, Footprint, Point, Zone};
use crate::pcb_layer::MechanicalLayer;
use crate::sketch::baked_kinds::BakedKind;
use crate::sketch::data::SketchData;

/// Marks a board cutout whose profile must be a slot: two parallel lines
/// joined by two tangent arcs of one radius. Set together with
/// `board_cutout` by the Slot role; the bake refuses any other profile.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BoardSlotAttr {}

/// The entity bakes as a drawing on a mechanical layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanicalAttr {
    /// Written as the board-layer token (`Mechanical3`).
    #[serde(with = "mechanical_serde")]
    pub layer: MechanicalLayer,
}

impl Default for MechanicalAttr {
    /// Mechanical 1, the layer the role picker starts with.
    fn default() -> Self {
        Self {
            layer: first_mechanical(),
        }
    }
}

/// Mechanical 1.
pub fn first_mechanical() -> MechanicalLayer {
    match MechanicalLayer::new(1) {
        Some(layer) => layer,
        None => unreachable!("1 is in 1..=MechanicalLayer::MAX"),
    }
}

/// The point is where a placed component's origin goes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentAnchorAttr {
    /// The placed footprint (`PcbBoard::footprints`) the point moves;
    /// `None` until one is picked, and the bake refuses it until then.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub footprint: Option<Uuid>,
}

/// A component a bake moves: its footprint, its designator for display, and
/// the solved point its origin goes to.
#[derive(Clone, Debug, PartialEq)]
pub struct BakedAnchor {
    /// The anchor point's entity uuid.
    pub entity: Uuid,
    pub footprint: Uuid,
    pub reference: String,
    pub position: Point,
}

/// What a bake of the board pours, mechanical drawings and component anchors
/// of a board sketch writes. Zones and drawings take the uuid of the entity
/// they came from, so the board engine knows them as the sketch's own.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SketchBoardItems {
    pub zones: Vec<Zone>,
    pub graphics: Vec<BoardGraphic>,
    pub anchors: Vec<BakedAnchor>,
}

impl SketchBoardItems {
    /// How many items of `kind` the bake wrote; `None` for a kind these
    /// items never hold.
    pub fn count(&self, kind: BakedKind) -> Option<usize> {
        Some(match kind {
            BakedKind::BoardPours => self.zones.len(),
            BakedKind::Mechanical => self.graphics.len(),
            BakedKind::ComponentAnchors => self.anchors.len(),
            _ => return None,
        })
    }

    /// The anchors whose footprint is locked: a bake leaves those where they
    /// are, and the caller says so.
    pub fn locked_anchors<'a>(&'a self, footprints: &[Footprint]) -> Vec<&'a BakedAnchor> {
        self.anchors
            .iter()
            .filter(|anchor| {
                footprints
                    .iter()
                    .any(|fp| fp.uuid == anchor.footprint && fp.locked)
            })
            .collect()
    }
}

/// The board targets that bake into the board's own zones, drawings and
/// footprint positions rather than into the stored board sketch.
pub const BOARD_ITEM_KINDS: [BakedKind; 3] = [
    BakedKind::BoardPours,
    BakedKind::Mechanical,
    BakedKind::ComponentAnchors,
];

impl SketchData {
    /// Whether a board sketch carries an attribute a build before these
    /// targets would drop on save: a slot, a pour, a mechanical drawing or
    /// a component anchor. It picks the `.snxpcb` token.
    pub fn carries_board_targets(&self) -> bool {
        self.entities.iter().any(|e| {
            e.board_slot.is_some()
                || e.pour.is_some()
                || e.mechanical.is_some()
                || e.component_anchor.is_some()
        })
    }

    /// Whether a board bake of this sketch writes board items: it tags or
    /// baked before a pour, a mechanical drawing or a component anchor.
    pub fn owns_board_items(&self) -> bool {
        BOARD_ITEM_KINDS.iter().any(|&kind| self.owns_kind(kind))
    }
}

/// Serde for [`MechanicalAttr::layer`]: the board-layer token, and nothing
/// but a mechanical layer reads back.
mod mechanical_serde {
    use serde::de::Error;
    use serde::{Deserialize, Deserializer, Serializer};

    use crate::format::{parse_pcb_layer, pcb_layer_token};
    use crate::pcb_layer::{MechanicalLayer, PcbLayer};

    pub fn serialize<S: Serializer>(layer: &MechanicalLayer, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&pcb_layer_token(PcbLayer::Mechanical(*layer)))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<MechanicalLayer, D::Error> {
        let value = String::deserialize(d)?;
        match parse_pcb_layer(&value) {
            Some(PcbLayer::Mechanical(layer)) => Ok(layer),
            _ => Err(D::Error::custom(format!(
                "mechanical layer attribute: `{value}` is not a mechanical layer"
            ))),
        }
    }
}
