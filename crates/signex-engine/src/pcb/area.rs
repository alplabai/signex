//! The board areas the area tools add (#2373, #2216, #2871): a Fill or a
//! Solid Region (solid copper), a Polygon Pour, a Keepout and a Pour
//! Cutout, each one zone. The tool's click and its operand form (#2470)
//! build the zone here, so both add the same area for the same outline.

use signex_types::pcb::{PcbBoard, Point, Zone};
use signex_types::pcb_layer::PcbLayer;
use signex_types::pcb_stack::board_copper_layers;
use signex_types::zone_keepout::keepout_copper;
use uuid::Uuid;

use super::draw;
use super::primitive_edit::{ZoneEdit, apply_zone_edit};

/// The kind of area an area tool adds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AreaKind {
    /// Solid copper over a rectangle.
    Fill,
    /// Solid copper over a polygon.
    SolidRegion,
    /// Copper computed clear of other nets.
    PolygonPour,
    /// Keeps all copper off a copper layer.
    Keepout,
    /// A region every pour on its layer leaves empty.
    PourCutout,
}

impl AreaKind {
    /// Whether the area carries a net: a keepout and a pour cutout do not.
    pub const fn takes_a_net(self) -> bool {
        !matches!(self, Self::Keepout | Self::PourCutout)
    }

    /// The word an answer names the kind by.
    pub const fn id(self) -> &'static str {
        match self {
            Self::Fill => "fill",
            Self::SolidRegion => "region",
            Self::PolygonPour => "pour",
            Self::Keepout => "keepout",
            Self::PourCutout => "pour_cutout",
        }
    }
}

/// Why an area named by its operands is not added.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AreaRefusal {
    #[error("this area goes on a copper layer")]
    NeedsCopper,
    #[error("a keepout goes on a copper or keep-out layer")]
    NeedsKeepoutLayer,
    #[error("the board's layer stack has no such copper layer")]
    LayerNotOnBoard,
    #[error("the outline encloses no area")]
    NoArea,
    #[error("a keepout or a pour cutout has no net")]
    TakesNoNet,
}

/// The area of `kind` over `outline` drawn on `layer`, as the tool adds it
/// before its form's values: on the net of the copper under its corners
/// for a copper area. `None` on a layer the kind cannot be drawn on, or an
/// outline with no area.
pub fn new_area(
    board: &PcbBoard,
    kind: AreaKind,
    id: Uuid,
    outline: Vec<Point>,
    layer: PcbLayer,
) -> Option<Zone> {
    match kind {
        AreaKind::Keepout => draw::new_keepout(id, outline, layer),
        AreaKind::PourCutout => draw::new_pour_cutout(id, outline, layer.copper()?),
        AreaKind::PolygonPour => draw::new_pour(board, id, outline, layer.copper()?),
        AreaKind::Fill | AreaKind::SolidRegion => {
            draw::new_copper_area(board, id, outline, layer.copper()?)
        }
    }
}

/// [`new_area`] for an outline and a layer a caller named (#2470): the
/// layer must be one of the board's copper layers (a keep-out layer stands
/// for its copper layer), and `net`, when given, is the area's net (0 is no
/// net) in place of the copper under its corners.
pub fn named_area(
    board: &PcbBoard,
    kind: AreaKind,
    id: Uuid,
    outline: Vec<Point>,
    layer: PcbLayer,
    net: Option<u32>,
) -> Result<Zone, AreaRefusal> {
    let copper = match kind {
        AreaKind::Keepout => keepout_copper(layer).ok_or(AreaRefusal::NeedsKeepoutLayer)?,
        _ => layer.copper().ok_or(AreaRefusal::NeedsCopper)?,
    };
    if !board_copper_layers(&board.layers).contains(&copper) {
        return Err(AreaRefusal::LayerNotOnBoard);
    }
    if net.is_some() && !kind.takes_a_net() {
        return Err(AreaRefusal::TakesNoNet);
    }
    let mut zone = new_area(board, kind, id, outline, layer).ok_or(AreaRefusal::NoArea)?;
    if let Some(net) = net {
        // A net edit refuses no copper area.
        apply_zone_edit(board, &mut zone, ZoneEdit::Net(net)).map_err(|_| AreaRefusal::NoArea)?;
    }
    Ok(zone)
}

#[cfg(test)]
#[path = "area_tests.rs"]
mod tests;
