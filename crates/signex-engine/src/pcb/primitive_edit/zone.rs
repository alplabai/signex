//! Area fields (#2393): the layer and net of a fill, solid region, polygon
//! pour or keepout, and a pour's clearance and minimum width.

use signex_types::format::{parse_pcb_layer, pcb_layer_token};
use signex_types::pcb::{PcbBoard, Zone};
use signex_types::pcb_layer::PcbLayer;
use signex_types::zone_keepout::{KEEPOUT_FILL, keepout_copper};
use uuid::Uuid;

use super::{PrimitiveEditRefusal, finite};
use crate::pcb::{BoardCommand, BoardItem};

/// One field of the selected areas, set to an absolute value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ZoneEdit {
    /// A copper area takes a copper layer; a keepout a copper or keep-out
    /// layer.
    Layer(PcbLayer),
    /// 0 is no net. A keepout has no net and is left as it is.
    Net(u32),
    /// A pour's clearance in mm; 0 follows the board's. Other areas are
    /// left as they are.
    Clearance(f64),
    /// A pour's minimum copper width in mm. Other areas are left as they
    /// are.
    MinThickness(f64),
}

/// Whether `layer` can hold `zone`: copper for a copper area, a layer that
/// keeps copper clear for a keepout.
pub fn zone_layer_allowed(zone: &Zone, layer: PcbLayer) -> bool {
    if zone.fill_type == KEEPOUT_FILL {
        keepout_copper(layer).is_some()
    } else {
        layer.is_copper()
    }
}

/// The command setting `edit` on every area in `zones`.
pub fn zone_edit_command(
    board: &PcbBoard,
    zones: &[Uuid],
    edit: ZoneEdit,
) -> Result<Option<BoardCommand>, PrimitiveEditRefusal> {
    if zones.is_empty() {
        return Err(PrimitiveEditRefusal::Empty);
    }
    let mut changed = Vec::new();
    for id in zones {
        let old = board
            .zones
            .iter()
            .find(|z| z.uuid == *id)
            .ok_or(PrimitiveEditRefusal::Gone)?;
        let mut new = old.clone();
        apply_zone_edit(board, &mut new, edit)?;
        if new != *old {
            changed.push(BoardItem::Zone(new));
        }
    }
    Ok((!changed.is_empty()).then_some(BoardCommand::ReplaceItems { items: changed }))
}

/// Set `edit` on `zone`, as [`zone_edit_command`] does on each area. The
/// placement tools use it for the area about to be placed.
pub fn apply_zone_edit(
    board: &PcbBoard,
    zone: &mut Zone,
    edit: ZoneEdit,
) -> Result<(), PrimitiveEditRefusal> {
    let is_keepout = zone.fill_type == KEEPOUT_FILL;
    if let ZoneEdit::Clearance(v) | ZoneEdit::MinThickness(v) = edit
        && finite(v)? < 0.0
    {
        return Err(PrimitiveEditRefusal::Negative);
    }
    match edit {
        ZoneEdit::Layer(layer) => {
            if !zone_layer_allowed(zone, layer) {
                return Err(PrimitiveEditRefusal::LayerNotAllowed);
            }
            if parse_pcb_layer(&zone.layer) != Some(layer) {
                zone.layer = pcb_layer_token(layer);
            }
        }
        ZoneEdit::Net(_) if is_keepout => {}
        ZoneEdit::Net(net) => {
            zone.net = net;
            zone.net_name = board
                .nets
                .iter()
                .find(|n| n.number == net && net != 0)
                .map(|n| n.name.clone())
                .unwrap_or_default();
        }
        ZoneEdit::Clearance(_) | ZoneEdit::MinThickness(_) if !zone.is_pour() => {}
        ZoneEdit::Clearance(v) => zone.clearance = v,
        ZoneEdit::MinThickness(v) => zone.min_thickness = v,
    }
    Ok(())
}
