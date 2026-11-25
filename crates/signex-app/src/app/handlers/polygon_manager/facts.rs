//! The Polygon Manager's rows (mockup row 178), read from the board and the
//! fills the app holds for its current revision.

use std::collections::BTreeMap;

use pcb_geom::ZoneFills;
use signex_i18n::{I18n, ids, t};
use signex_types::format::parse_pcb_layer;
use signex_types::pcb::{PcbBoard, Zone};
use signex_types::pcb_layer::{CopperLayer, PcbLayer};

use crate::panels::pcb_primitive_properties::unfilled_reason;
use crate::panels::polygons::{PourRow, PourState};
use crate::pcb_canvas::layer_style;

/// Every pour of `board`, grouped by copper layer in stack order and in
/// pour order on each layer, its shelved pours last. `fills` are the
/// board's fills when they are of its current revision.
pub(super) fn pour_rows(board: &PcbBoard, fills: Option<&ZoneFills>, i18n: &I18n) -> Vec<PourRow> {
    let copper = signex_types::pcb_stack::board_copper_layers(&board.layers);
    let places = fill_places(board);
    let mut rows: Vec<PourRow> = board
        .zones
        .iter()
        .enumerate()
        .filter(|(_, zone)| zone.is_pour())
        .enumerate()
        .map(|(number, (index, zone))| {
            let mut row = pour_row(board, &copper, zone, number + 1, i18n);
            row.order = places.get(&index).copied();
            with_fill(row, index, zone, fills)
        })
        .collect();
    // Stable: pours of one place keep board order.
    rows.sort_by_key(|row| (row.layer_place, row.order.unwrap_or(usize::MAX)));
    rows
}

/// Each filling pour's place in the fill order of its layer, from 1, by
/// its index in `PcbBoard::zones`.
fn fill_places(board: &PcbBoard) -> BTreeMap<usize, usize> {
    let mut per_layer: BTreeMap<&str, usize> = BTreeMap::new();
    pcb_geom::pour_fill_order(board)
        .into_iter()
        .map(|index| {
            let place = per_layer
                .entry(board.zones[index].layer.as_str())
                .or_default();
            *place += 1;
            (index, *place)
        })
        .collect()
}

/// The row of pour number `number` (from 1, in board order) before its fill
/// is read.
fn pour_row(
    board: &PcbBoard,
    copper: &[CopperLayer],
    zone: &Zone,
    number: usize,
    i18n: &I18n,
) -> PourRow {
    let layer = parse_pcb_layer(&zone.layer);
    let layer_name = layer.map_or_else(
        || zone.layer.clone(),
        |layer| layer_style::layer_name(i18n, layer).into_string(),
    );
    // A pour off the copper layers groups after them.
    let layer_place = match layer {
        Some(PcbLayer::Copper(c)) => copper.iter().position(|l| *l == c),
        _ => None,
    }
    .map_or(copper.len() + 1, |at| at + 1);
    let net = zone.net_name.clone();
    let named_net = if net.is_empty() {
        t!(i18n, ids::PCB_PRIM_NO_NET).into_string()
    } else {
        net.clone()
    };
    let name = board
        .options
        .polygon_naming
        .name(&named_net, layer_place, &layer_name, number);
    PourRow {
        zone: zone.uuid,
        name,
        net,
        layer: layer_name,
        layer_place,
        order: None,
        state: if zone.is_shelved_pour() {
            PourState::Shelved
        } else {
            PourState::Poured
        },
        unfilled: None,
        clearance_mm: zone.clearance,
        min_width_mm: zone.min_thickness,
        islands_below_mm2: zone.min_island_area,
        dead_copper: zone.remove_dead_islands,
        area_mm2: None,
        pieces: None,
        islands_removed: None,
    }
}

/// `row` with what the fill of the pour at `index` holds, or why it has
/// none. Without fills the row keeps no copper facts.
fn with_fill(mut row: PourRow, index: usize, zone: &Zone, fills: Option<&ZoneFills>) -> PourRow {
    let Some(fills) = fills else {
        return row;
    };
    if let Some(fill) = fills.of(index).filter(|f| f.zone == zone.uuid) {
        row.area_mm2 = Some(fill.area_mm2());
        row.pieces = Some(fill.pieces.len());
        row.islands_removed = Some(fill.islands_removed);
    } else if let Some(why) = fills.unfilled(index) {
        row.state = PourState::NotFilled;
        row.unfilled = Some(unfilled_reason(why));
    }
    row
}
