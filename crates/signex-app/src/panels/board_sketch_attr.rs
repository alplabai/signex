//! Properties panel edits of a board sketch entity's board attributes
//! (#2234). Keepouts and cutouts use the footprint Role messages.

use signex_sketch::id::SketchEntityId;
use signex_types::pcb_layer::{MechanicalLayer, PcbLayer};
use uuid::Uuid;

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
    /// A board pour's copper layer was picked (decision 2, 2026-11-20).
    PourLayerPicked { id: SketchEntityId, layer: PcbLayer },
    /// A mechanical drawing's layer was picked.
    MechanicalLayerPicked {
        id: SketchEntityId,
        layer: MechanicalLayer,
    },
    /// The component a component anchor moves was picked.
    AnchorComponentPicked { id: SketchEntityId, footprint: Uuid },
}

/// The board target rows of the selected entity (decision 2): what its role
/// shows and the choices its pickers offer.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum BoardTargetProps {
    /// No row: the entity has none of these roles.
    #[default]
    None,
    /// A Slot: its size (length, width) at the current solve, `None` while
    /// the profile is not a slot yet. Read-only.
    Slot { size: Option<(f64, f64)> },
    /// A board pour on `layer`, one of the board's copper `layers`.
    Pour {
        layer: PcbLayer,
        layers: Vec<LayerChoice>,
    },
    /// A drawing on mechanical `layer`, one of `layers`.
    Mechanical {
        layer: MechanicalLayer,
        layers: Vec<MechanicalLayer>,
    },
    /// A component anchor moving `picked`, one of the board's placed
    /// components.
    Anchor {
        picked: Option<Uuid>,
        components: Vec<ComponentChoice>,
    },
}

/// A copper layer as the pour picker lists it: the board's name for it.
#[derive(Debug, Clone, PartialEq)]
pub struct LayerChoice {
    pub layer: PcbLayer,
    pub name: String,
}

impl std::fmt::Display for LayerChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name)
    }
}

/// A placed component as the anchor picker lists it: by designator.
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentChoice {
    pub footprint: Uuid,
    pub reference: String,
}

impl std::fmt::Display for ComponentChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.reference)
    }
}
