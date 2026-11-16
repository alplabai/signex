//! Board items drawn with the PCB placement tools (#2373). Pure: each
//! function builds what one `BoardCommand::AddItems` adds as one undo step,
//! with the board's own defaults and the net of the copper it lands on.

use pcb_geom::{PointNm, layer_bit, net_at};
use signex_types::corner::CornerRule;
use signex_types::format::pcb_layer_token;
use signex_types::pcb::{
    PCB_DEFAULT_TRACE_WIDTH_MM, PCB_DEFAULT_VIA_DIAMETER_MM, PCB_DEFAULT_VIA_DRILL_MM, PcbBoard,
    PcbSetup, Point, Segment, Via, ViaType, Zone,
};
use signex_types::pcb_layer::{CopperLayer, PcbLayer};
use signex_types::zone_keepout::{KEEPOUT_FILL, keepout_copper};
use signex_types::zone_pour::POUR_FILL;
use uuid::Uuid;

use super::command::BoardCommand;
use super::items::BoardItem;

/// A setup value when it is set (finite and above zero), else `default`.
fn setup_or(board: &PcbBoard, field: impl Fn(&PcbSetup) -> f64, default: f64) -> f64 {
    board
        .setup
        .as_ref()
        .map(field)
        .filter(|v| v.is_finite() && *v > 0.0)
        .unwrap_or(default)
}

/// The net of the copper under `at` on the copper layers of `layers` (a
/// `pcb_geom::layer_bit` mask), 0 over bare board or a point the geometry
/// cannot hold.
pub fn net_under(board: &PcbBoard, at: Point, layers: u32) -> u32 {
    PointNm::from_mm(at, "point")
        .ok()
        .and_then(|p| net_at(board, p, layers))
        .unwrap_or(0)
}

/// A through via at `at` with the board's via size (`PcbSetup` diameter and
/// drill, else the PCB defaults), on the net of the copper it lands on.
pub fn new_via(board: &PcbBoard, id: Uuid, at: Point) -> Via {
    let diameter = setup_or(board, |s| s.via_diameter, PCB_DEFAULT_VIA_DIAMETER_MM);
    let drill = setup_or(board, |s| s.via_drill, PCB_DEFAULT_VIA_DRILL_MM).min(diameter);
    Via {
        uuid: id,
        position: at,
        diameter,
        drill,
        layers: vec!["TopCopper".into(), "BottomCopper".into()],
        net: net_under(board, at, u32::MAX),
        locked: false,
        layer_sizes: None,
        via_type: ViaType::Through,
        unknown_columns: Default::default(),
    }
}

/// Place a via at `at`: one undo step.
pub fn place_via(board: &PcbBoard, id: Uuid, at: Point) -> BoardCommand {
    BoardCommand::AddItems {
        items: vec![BoardItem::Via(new_via(board, id, at))],
    }
}

/// How a track run is drawn: its copper layer, width and net.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrackStyle {
    pub layer: CopperLayer,
    pub width_mm: f64,
    pub net: u32,
}

impl TrackStyle {
    /// A run starting at `at` on `layer`: the board's track width
    /// (`PcbSetup::trace_width`, else the PCB default) and the net of the
    /// copper under `at` on that layer.
    pub fn starting_at(board: &PcbBoard, at: Point, layer: CopperLayer) -> Self {
        Self {
            layer,
            width_mm: setup_or(board, |s| s.trace_width, PCB_DEFAULT_TRACE_WIDTH_MM),
            net: net_under(board, at, 1 << layer_bit(layer)),
        }
    }
}

/// The tracks of a run from `from` to `to`, bent by `rule` into one or two
/// legs (#2372); none when the points are the same. `ids` names each new
/// track.
pub fn track_run(
    from: Point,
    to: Point,
    rule: CornerRule,
    style: TrackStyle,
    mut ids: impl FnMut() -> Uuid,
) -> Vec<Segment> {
    let layer = pcb_layer_token(PcbLayer::Copper(style.layer));
    rule.legs(from, to)
        .into_iter()
        .map(|(start, end)| Segment {
            locked: false,
            uuid: ids(),
            start,
            end,
            width: style.width_mm,
            layer: layer.clone(),
            net: style.net,
            unknown_columns: Default::default(),
        })
        .collect()
}

/// Add a run's tracks, with the via that takes it to another layer first
/// when there is one: one undo step. `None` when there is nothing to add.
pub fn place_route(tracks: Vec<Segment>, via: Option<Via>) -> Option<BoardCommand> {
    let items: Vec<BoardItem> = via
        .map(BoardItem::Via)
        .into_iter()
        .chain(tracks.into_iter().map(BoardItem::Segment))
        .collect();
    (!items.is_empty()).then_some(BoardCommand::AddItems { items })
}

/// The corners of the rectangle spanned by `a` and `b`, in drawing order.
pub fn rectangle(a: Point, b: Point) -> Vec<Point> {
    vec![a, Point::new(b.x, a.y), b, Point::new(a.x, b.y)]
}

/// Solid copper over `outline` on `layer` (a Fill or a Solid Region, stored
/// as a zone), on the net of the copper under its first point. `None` for an
/// outline with no area or a point the geometry cannot hold.
pub fn new_copper_area(
    board: &PcbBoard,
    id: Uuid,
    outline: Vec<Point>,
    layer: CopperLayer,
) -> Option<Zone> {
    if !has_area(&outline) {
        return None;
    }
    let net = net_under(board, *outline.first()?, 1 << layer_bit(layer));
    let net_name = board
        .nets
        .iter()
        .find(|n| n.number == net)
        .map(|n| n.name.clone())
        .unwrap_or_default();
    Some(Zone {
        uuid: id,
        net,
        net_name,
        layer: pcb_layer_token(PcbLayer::Copper(layer)),
        outline,
        holes: Vec::new(),
        priority: 0,
        fill_type: signex_types::pcb::SOLID_FILL.into(),
        thermal_relief: false,
        thermal_gap: 0.0,
        thermal_width: 0.0,
        clearance: 0.0,
        min_thickness: 0.0,
        unknown: Default::default(),
    })
}

/// A polygon pour over `outline` on `layer` (#2216): a zone with the pour
/// fill, on the net of the copper under its first point, or no net. Its
/// clearance is left at 0, which follows the board's clearance
/// (`Zone::pour_clearance_mm`). `None` for an outline with no area or a
/// point the geometry cannot hold.
pub fn new_pour(
    board: &PcbBoard,
    id: Uuid,
    outline: Vec<Point>,
    layer: CopperLayer,
) -> Option<Zone> {
    new_copper_area(board, id, outline, layer).map(|zone| Zone {
        fill_type: POUR_FILL.into(),
        ..zone
    })
}

/// A keepout over `outline` drawn on `layer` (#2373): a zone with the
/// keepout fill and no net, which keeps all copper off the copper layer
/// `layer` stands for (`zone_keepout::keepout_copper`). `None` for a layer
/// that keeps no copper clear, or an outline with no area.
pub fn new_keepout(id: Uuid, outline: Vec<Point>, layer: PcbLayer) -> Option<Zone> {
    keepout_copper(layer)?;
    if !has_area(&outline) {
        return None;
    }
    Some(Zone {
        uuid: id,
        net: 0,
        net_name: String::new(),
        layer: pcb_layer_token(layer),
        outline,
        holes: Vec::new(),
        priority: 0,
        fill_type: KEEPOUT_FILL.into(),
        thermal_relief: false,
        thermal_gap: 0.0,
        thermal_width: 0.0,
        clearance: 0.0,
        min_thickness: 0.0,
        unknown: Default::default(),
    })
}

/// Add `zone`: one undo step.
pub fn place_zone(zone: Zone) -> BoardCommand {
    BoardCommand::AddItems {
        items: vec![BoardItem::Zone(zone)],
    }
}

/// A board string (#2373) reading `text` at `at` on `layer`, at the PCB
/// default size and unturned. `None` off the overlay (silkscreen) layers:
/// copper text would be copper the copper list, DRC and ratsnest never read.
pub fn new_text(
    id: Uuid,
    at: Point,
    layer: PcbLayer,
    text: String,
) -> Option<signex_types::pcb::BoardText> {
    if !layer.is_overlay() {
        return None;
    }
    Some(signex_types::pcb::BoardText {
        uuid: id,
        text,
        position: at,
        layer: pcb_layer_token(layer),
        font_size: signex_types::pcb::PCB_DEFAULT_TEXT_SIZE_MM,
        rotation: 0.0,
    })
}

/// Add a board string: one undo step.
pub fn place_text(text: signex_types::pcb::BoardText) -> BoardCommand {
    BoardCommand::AddItems {
        items: vec![BoardItem::Text(text)],
    }
}

/// Whether the polygon encloses any area, in integer nm (exact): false for
/// fewer than three points, points on one line, or a point the geometry
/// cannot hold.
fn has_area(points: &[Point]) -> bool {
    let Ok(nm) = points
        .iter()
        .map(|p| PointNm::from_mm(*p, "outline"))
        .collect::<Result<Vec<_>, _>>()
    else {
        return false;
    };
    let next = nm.iter().cycle().skip(1);
    let twice_area: i128 = nm
        .iter()
        .zip(next)
        .map(|(a, b)| i128::from(a.x) * i128::from(b.y) - i128::from(b.x) * i128::from(a.y))
        .sum();
    nm.len() >= 3 && twice_area != 0
}

#[cfg(test)]
#[path = "draw_tests.rs"]
mod tests;
