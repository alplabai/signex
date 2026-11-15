//! The facts of the area panel (#2393): fills, solid regions, polygon pours
//! and keepouts, read from the zones. The kind is named the way the pick
//! filter names it (`pcb_geom::PickKind::of_zone`).

use pcb_geom::{ItemRef, PickKind};
use signex_engine::pcb::BoardCommand;
use signex_engine::pcb::primitive_edit::{PrimitiveEditRefusal, ZoneEdit, zone_edit_command};
use signex_i18n::I18n;
use signex_types::format::parse_pcb_layer;
use signex_types::pcb::{PcbBoard, Zone};
use signex_types::pcb_layer::{PcbLayer, TechnicalLayer};
use signex_types::zone_keepout::KEEPOUT_FILL;

use super::PrimitiveEdit;
use crate::panels::pcb_primitive_properties::{
    AreaLayerChoice, PrimitiveCommon, PrimitiveField, ZoneFacts, ZoneKind, shared,
};
use crate::pcb_canvas::layer_style;

/// What kind of area `zone` is; `None` for a zone none of the tools makes.
fn zone_kind(zone: &Zone) -> Option<ZoneKind> {
    if zone.is_pour() {
        return Some(ZoneKind::Pour);
    }
    match PickKind::of_zone(zone)? {
        PickKind::Keepouts => Some(ZoneKind::Keepout),
        PickKind::Fills => Some(ZoneKind::Fill),
        PickKind::Regions => Some(ZoneKind::Region),
        _ => None,
    }
}

/// The layers an area can be on: the board's copper layers, and for
/// keepouts the two keep-out layers too.
fn area_layers(board: &PcbBoard, keepout: bool, i18n: &I18n) -> Vec<AreaLayerChoice> {
    let copper = signex_renderer::pcb::board_copper_layers(&board.layers)
        .into_iter()
        .map(PcbLayer::Copper);
    let keep_out = [TechnicalLayer::KeepoutTop, TechnicalLayer::KeepoutBottom]
        .into_iter()
        .map(PcbLayer::Technical)
        .filter(|_| keepout);
    copper
        .chain(keep_out)
        .map(|layer| AreaLayerChoice {
            layer,
            label: layer_style::layer_name(i18n, layer).into_string(),
        })
        .collect()
}

pub(super) fn zone_facts(
    board: &PcbBoard,
    common: PrimitiveCommon,
    zones: &[&Zone],
    i18n: &I18n,
) -> ZoneFacts {
    let kind = shared(zones.iter().map(|z| zone_kind(z))).flatten();
    let all_keepouts = zones.iter().all(|z| z.fill_type == KEEPOUT_FILL);
    let layer = shared(zones.iter().map(|z| z.layer.as_str()))
        .and_then(parse_pcb_layer)
        .map(|layer| AreaLayerChoice {
            layer,
            label: layer_style::layer_name(i18n, layer).into_string(),
        });
    ZoneFacts {
        kind,
        is_copper: zones.iter().all(|z| z.fill_type != KEEPOUT_FILL),
        is_pour: zones.iter().all(|z| z.is_pour()),
        layer,
        layers: area_layers(board, all_keepouts, i18n),
        clearance: shared(zones.iter().map(|z| z.clearance)),
        min_thickness: shared(zones.iter().map(|z| z.min_thickness)),
        common,
    }
}

/// The area edit `edit` stands for; `None` for a field areas do not have.
pub(crate) fn zone_edit(edit: &PrimitiveEdit) -> Option<ZoneEdit> {
    use PrimitiveField as F;
    Some(match *edit {
        PrimitiveEdit::AreaLayer(layer) => ZoneEdit::Layer(layer),
        PrimitiveEdit::Layer(layer) => ZoneEdit::Layer(PcbLayer::Copper(layer)),
        PrimitiveEdit::Net(net) => ZoneEdit::Net(net),
        PrimitiveEdit::Number(F::Clearance, v) => ZoneEdit::Clearance(v),
        PrimitiveEdit::Number(F::MinThickness, v) => ZoneEdit::MinThickness(v),
        _ => return None,
    })
}

/// The command for `edit` on the selected areas `items`.
pub(super) fn zone_command(
    board: &PcbBoard,
    items: &[ItemRef],
    edit: &PrimitiveEdit,
) -> Result<Option<BoardCommand>, PrimitiveEditRefusal> {
    let ids: Vec<_> = items
        .iter()
        .filter_map(|i| match i {
            ItemRef::Zone { zone, .. } => Some(*zone),
            _ => None,
        })
        .collect();
    match zone_edit(edit) {
        Some(e) => zone_edit_command(board, &ids, e),
        None => Ok(None),
    }
}
