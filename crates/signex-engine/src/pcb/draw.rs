//! Board items drawn with the PCB placement tools (#2373). Pure: each
//! function builds what one `BoardCommand::AddItems` adds as one undo step,
//! with the board's own defaults and the net of the copper it lands on.

use pcb_geom::{PointNm, net_at};
use signex_types::pcb::{
    PCB_DEFAULT_VIA_DIAMETER_MM, PCB_DEFAULT_VIA_DRILL_MM, PcbBoard, PcbSetup, Point, Via, ViaType,
};
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

#[cfg(test)]
#[path = "draw_tests.rs"]
mod tests;
