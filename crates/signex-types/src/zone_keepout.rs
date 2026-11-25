//! A keepout drawn on the board (#2373): a zone whose `fill_type` is
//! [`KEEPOUT_FILL`]. It keeps out copper of any net on the copper layer it
//! stands for, as a board-sketch keepout forbidding all copper does, so the
//! design rule check reads both the same way.

use crate::format::parse_pcb_layer;
use crate::pcb::Zone;
use crate::pcb_layer::{CopperLayer, PcbLayer, TechnicalLayer};
use crate::pcb_sketch::PcbKeepout;
use crate::sketch::attr::KeepoutKinds;

/// The `fill_type` of a keepout zone.
pub const KEEPOUT_FILL: &str = "keepout";

/// The copper layer a keepout drawn on `layer` keeps clear: the layer
/// itself for copper, the side's outer copper for a keep-out layer, none for
/// any other layer.
pub fn keepout_copper(layer: PcbLayer) -> Option<CopperLayer> {
    match layer {
        PcbLayer::Copper(copper) => Some(copper),
        PcbLayer::Technical(TechnicalLayer::KeepoutTop) => Some(CopperLayer::Top),
        PcbLayer::Technical(TechnicalLayer::KeepoutBottom) => Some(CopperLayer::Bottom),
        PcbLayer::Technical(_) | PcbLayer::Mechanical(_) => None,
    }
}

impl Zone {
    /// The keepout this zone is, when it is one on a layer that keeps
    /// copper clear: its outline, the copper layer and all copper
    /// forbidden.
    pub fn as_keepout(&self) -> Option<PcbKeepout> {
        if self.fill_type != KEEPOUT_FILL {
            return None;
        }
        let copper = keepout_copper(parse_pcb_layer(&self.layer)?)?;
        Some(PcbKeepout {
            uuid: self.uuid,
            boundary: self.outline.clone(),
            layers: vec![PcbLayer::Copper(copper)],
            forbids: KeepoutKinds::ALL_COPPER,
        })
    }

    /// Whether the zone is a rule area (a keepout or a pour cutout among
    /// them), not copper: its fill type or its layer names a rule, or it is
    /// a keepout or a pour cutout (#2871).
    pub fn is_rule_area(&self) -> bool {
        let fill = self.fill_type.to_ascii_lowercase();
        let layer = self.layer.to_ascii_lowercase();
        fill.contains("rule")
            || fill.contains("keepout")
            || self.is_pour_cutout()
            || layer.contains("rule")
    }
}
