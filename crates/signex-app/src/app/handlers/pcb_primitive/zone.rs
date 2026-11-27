//! The facts of the area panel (#2393): fills, solid regions, polygon pours,
//! keepouts and pour cutouts (#2871), read from the zones. The kind is named the way the pick
//! filter names it (`pcb_geom::PickKind::of_zone`). A pour also shows its
//! priority, island rules and the copper its fill holds (#2216), and its
//! Copper card repours, unfills (#2506) and shelves (#2523): Unfill and
//! Shelve are one board undo step each; Repour refreshes the computed fill
//! and changes the document only to fill an unfilled pour again.

use std::path::Path;

use pcb_geom::{ItemRef, PickKind, ZoneFills};
use signex_engine::pcb::BoardCommand;
use signex_engine::pcb::pour_edit::shelve_all_pours;
use signex_engine::pcb::primitive_edit::{PrimitiveEditRefusal, ZoneEdit, zone_edit_command};
use signex_i18n::{I18n, ids};
use signex_types::format::parse_pcb_layer;
use signex_types::pcb::{PcbBoard, Zone};
use signex_types::pcb_layer::{PcbLayer, TechnicalLayer};
use signex_types::zone_keepout::KEEPOUT_FILL;
use uuid::Uuid;

use super::PrimitiveEdit;
use crate::app::Signex;
use crate::panels::pcb_primitive_properties::{
    AreaLayerChoice, PourMsg, PrimitiveCommon, PrimitiveField, PrimitiveTarget, ZoneFacts,
    ZoneKind, shared,
};
use crate::pcb_canvas::layer_style;

impl Signex {
    /// An event of a pour's Copper card (#2523).
    pub(crate) fn handle_pour_msg(&mut self, msg: &PourMsg) {
        let panel = &mut self.interaction_state.pcb_primitive;
        match msg {
            PourMsg::MenuToggled => {
                panel.pour_menu_open = !panel.pour_menu_open;
                return;
            }
            PourMsg::RepourPressed { target } => {
                self.repour_pours(&target.board, &zone_ids(&target.items).collect());
            }
            PourMsg::UnfillPressed { target } if !target.is_placement() => {
                let zones: Vec<Uuid> = zone_ids(&target.items).collect();
                self.unfill_pours(&target.board, &zones);
            }
            PourMsg::UnfillPressed { .. } => {}
            PourMsg::ShelveToggled { target, shelved } => self.shelve_pours(target, *shelved),
            PourMsg::RepourAllPressed { board } => self.repour_board(board, false),
            PourMsg::ShelveAllPressed { board } => self.shelve_board_pours(board, true),
            PourMsg::UnshelveAllPressed { board } => self.shelve_board_pours(board, false),
        }
        // A pick closes the menu.
        self.interaction_state.pcb_primitive.pour_menu_open = false;
    }

    /// Shelve or unshelve the selected pours as one undo step.
    fn shelve_pours(&mut self, target: &PrimitiveTarget, shelved: bool) {
        if target.is_placement() {
            return;
        }
        let zones: Vec<_> = zone_ids(&target.items).collect();
        let what = if shelved {
            ids::PCB_PRIM_POUR_SHELVE
        } else {
            ids::PCB_PRIM_POUR_UNSHELVE
        };
        self.run_guarded_board_step(&target.board, what, |board| {
            Ok(
                zone_edit_command(board, &zones, ZoneEdit::Shelved(shelved))?
                    .into_iter()
                    .collect(),
            )
        });
    }

    /// Shelve or unshelve every pour of the board at `path` (the board the
    /// panel shows, which need not be the tab in view) as one undo step.
    fn shelve_board_pours(&mut self, path: &Path, shelved: bool) {
        let what = if shelved {
            ids::PCB_PRIM_POUR_SHELVE_ALL
        } else {
            ids::PCB_PRIM_POUR_UNSHELVE_ALL
        };
        self.run_guarded_board_step(path, what, |board| {
            Ok(shelve_all_pours(board, shelved).into_iter().collect())
        });
    }
}

/// The zones among `items`.
fn zone_ids(items: &[ItemRef]) -> impl Iterator<Item = Uuid> + '_ {
    items.iter().filter_map(|i| match i {
        ItemRef::Zone { zone, .. } => Some(*zone),
        _ => None,
    })
}

/// What kind of area `zone` is; `None` for a zone none of the tools makes.
fn zone_kind(zone: &Zone) -> Option<ZoneKind> {
    if zone.is_pour() {
        return Some(ZoneKind::Pour);
    }
    if zone.is_pour_cutout() {
        return Some(ZoneKind::Cutout);
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

/// The copper the fills of the pours in `zones` hold: area in mm² and
/// pieces. `None` without fills, or when a zone is not a filled pour.
fn pour_copper(
    board: &PcbBoard,
    zones: &[&Zone],
    fills: Option<&ZoneFills>,
) -> Option<(f64, usize)> {
    let fills = fills?;
    let mut total = (0.0, 0);
    for zone in zones {
        let index = board.zones.iter().position(|z| z.uuid == zone.uuid)?;
        let fill = fills.of(index).filter(|f| f.zone == zone.uuid)?;
        total.0 += fill.area_mm2();
        total.1 += fill.pieces.len();
    }
    Some(total)
}

/// The facts of the area panel for `zones`; `fills` are the board's pour
/// fills when they are computed.
pub(super) fn zone_facts(
    board: &PcbBoard,
    common: PrimitiveCommon,
    zones: &[&Zone],
    fills: Option<&ZoneFills>,
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
        is_copper: zones
            .iter()
            .all(|z| z.fill_type != KEEPOUT_FILL && !z.is_pour_cutout()),
        is_pour: zones.iter().all(|z| z.is_pour()),
        layer,
        layers: area_layers(board, all_keepouts, i18n),
        clearance: shared(zones.iter().map(|z| z.clearance)),
        min_thickness: shared(zones.iter().map(|z| z.min_thickness)),
        priority: shared(zones.iter().map(|z| z.priority)),
        min_island_area: shared(zones.iter().map(|z| z.min_island_area)),
        remove_dead_islands: shared(zones.iter().map(|z| z.remove_dead_islands)),
        copper: pour_copper(board, zones, fills),
        shelved: zones
            .iter()
            .all(|z| z.is_pour())
            .then(|| shared(zones.iter().map(|z| z.shelved)))
            .flatten(),
        corners: shared(zones.iter().map(|z| z.corners)),
        corner_size: shared(zones.iter().map(|z| z.corner_size)),
        fill_mode: shared(zones.iter().map(|z| z.fill_mode)),
        hatch_width: shared(zones.iter().map(|z| z.hatch_settings().width_mm)),
        hatch_gap: shared(zones.iter().map(|z| z.hatch_settings().gap_mm)),
        thermal: shared(
            zones
                .iter()
                .map(|z| (z.thermal_relief, z.thermal_width, z.thermal_gap)),
        ),
        menu_open: false,
        unfilled: pour_unfilled(board, zones, fills),
        outline: match zones {
            [zone] => Some((zone.outline.len(), outline_area_mm2(zone))),
            _ => None,
        },
        common,
    }
}

/// The area of `zone`'s outline less its holes, in mm²; 0 for an outline
/// the geometry cannot hold.
fn outline_area_mm2(zone: &Zone) -> f64 {
    pcb_geom::zone_area(zone).map_or(0.0, |area| area.twice_area() as f64 / 2.0e12)
}

/// Why the first selected pour with no fill has none.
fn pour_unfilled(
    board: &PcbBoard,
    zones: &[&Zone],
    fills: Option<&ZoneFills>,
) -> Option<pcb_geom::Unfilled> {
    let fills = fills?;
    zones.iter().find_map(|zone| {
        let index = board.zones.iter().position(|z| z.uuid == zone.uuid)?;
        fills.unfilled(index)
    })
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
        PrimitiveEdit::Number(F::Priority, v) => ZoneEdit::Priority(v),
        PrimitiveEdit::Number(F::MinIslandArea, v) => ZoneEdit::MinIslandArea(v),
        PrimitiveEdit::DeadIslands(on) => ZoneEdit::RemoveDeadIslands(on),
        PrimitiveEdit::PourCorners(corners) => ZoneEdit::Corners(corners),
        PrimitiveEdit::Number(F::CornerSize, v) => ZoneEdit::CornerSize(v),
        PrimitiveEdit::PourFill(mode) => ZoneEdit::FillMode(mode),
        PrimitiveEdit::Number(F::HatchWidth, v) => ZoneEdit::HatchWidth(v),
        PrimitiveEdit::Number(F::HatchGap, v) => ZoneEdit::HatchGap(v),
        PrimitiveEdit::ThermalRelief(on) => ZoneEdit::ThermalRelief(on),
        PrimitiveEdit::Number(F::ThermalSpokeWidth, v) => ZoneEdit::ThermalSpokeWidth(v),
        PrimitiveEdit::Number(F::ThermalAirGap, v) => ZoneEdit::ThermalAirGap(v),
        _ => return None,
    })
}

/// The command for `edit` on the selected areas `items`.
pub(super) fn zone_command(
    board: &PcbBoard,
    items: &[ItemRef],
    edit: &PrimitiveEdit,
) -> Result<Option<BoardCommand>, PrimitiveEditRefusal> {
    let ids: Vec<_> = zone_ids(items).collect();
    match zone_edit(edit) {
        Some(e) => zone_edit_command(board, &ids, e),
        None => Ok(None),
    }
}
