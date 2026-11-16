//! Via stitching (#2408): vias of one net on a grid over a pour, a copper
//! area or a rectangle, each placed only where `pcb_geom::stitch_points`
//! finds it clear. The vias take the board's via size. One undo step.

use pcb_geom::{PointNm, Polygon, stitch_points, stitch_spec_mm, zone_area, zone_fills};
use signex_types::pcb::{PcbBoard, Point, Via};
use uuid::Uuid;

use super::{ToolEdit, ToolRefusal, ToolReport};
use crate::pcb::command::BoardCommand;
use crate::pcb::draw::new_via;
use crate::pcb::items::BoardItem;
use crate::pcb::placement::grid_step_mm;

/// The least grid step the stitching takes when none is asked for.
pub const DEFAULT_STITCH_PITCH_MM: f64 = 2.0;

/// Where to stitch.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StitchArea {
    /// Over a zone: a pour's computed fill, else the zone's outline less its
    /// holes. Its net is the default net.
    Zone(Uuid),
    /// Over the rectangle spanned by two corners.
    Rect(Point, Point),
}

/// The default grid step on `board`: the smallest multiple of its snap grid
/// that is at least [`DEFAULT_STITCH_PITCH_MM`].
pub fn default_stitch_pitch_mm(board: &PcbBoard) -> f64 {
    let grid = grid_step_mm(board);
    (DEFAULT_STITCH_PITCH_MM / grid).ceil().max(1.0) * grid
}

/// Stitch `area` with vias of `net` (the zone's net when `None`) every
/// `pitch_mm`, kept `clearance(a, b)` from copper of other nets. `ids` names
/// each new via.
pub fn stitch_vias<F: Fn(u32, u32) -> i64>(
    board: &PcbBoard,
    area: StitchArea,
    net: Option<u32>,
    pitch_mm: f64,
    clearance: F,
    mut ids: impl FnMut() -> Uuid,
) -> Result<ToolEdit, ToolRefusal> {
    let (pieces, area_net) = area_pieces(board, area)?;
    let net = net
        .or(area_net)
        .filter(|n| *n != 0)
        .ok_or(ToolRefusal::NoNet)?;
    let template = new_via(board, Uuid::nil(), Point::new(0.0, 0.0));
    if !(pitch_mm.is_finite() && pitch_mm >= template.diameter) {
        return Err(ToolRefusal::PitchTooSmall {
            pitch: pitch_mm,
            diameter: template.diameter,
        });
    }
    let spec = stitch_spec_mm(net, pitch_mm, template.diameter).ok_or(ToolRefusal::NoArea)?;
    let points = stitch_points(board, &pieces, spec, clearance);
    if points.is_empty() {
        return Err(ToolRefusal::NothingFits);
    }
    let vias: Vec<Via> = points
        .into_iter()
        .map(|p| Via {
            uuid: ids(),
            position: p.to_mm(),
            net,
            ..template.clone()
        })
        .collect();
    let report = ToolReport {
        added: vias.iter().map(|v| v.uuid).collect(),
        ..ToolReport::default()
    };
    Ok(ToolEdit {
        commands: vec![BoardCommand::AddItems {
            items: vias.into_iter().map(BoardItem::Via).collect(),
        }],
        report,
    })
}

/// The area's pieces and the net it is on, if any.
fn area_pieces(
    board: &PcbBoard,
    area: StitchArea,
) -> Result<(Vec<Polygon>, Option<u32>), ToolRefusal> {
    match area {
        StitchArea::Zone(id) => {
            let (index, zone) = board
                .zones
                .iter()
                .enumerate()
                .find(|(_, z)| z.uuid == id)
                .ok_or(ToolRefusal::NothingSelected)?;
            let pieces = if zone.is_pour() {
                zone_fills(board)
                    .of(index)
                    .map(|fill| fill.pieces.clone())
                    .unwrap_or_default()
            } else {
                zone_area(zone).into_iter().collect()
            };
            if pieces.is_empty() {
                return Err(ToolRefusal::NoArea);
            }
            Ok((pieces, Some(zone.net)))
        }
        StitchArea::Rect(a, b) => {
            let (Ok(a), Ok(b)) = (
                PointNm::from_mm(a, "stitch.corner"),
                PointNm::from_mm(b, "stitch.corner"),
            ) else {
                return Err(ToolRefusal::NoArea);
            };
            if a.x == b.x || a.y == b.y {
                return Err(ToolRefusal::NoArea);
            }
            let ring = vec![a, PointNm::new(b.x, a.y), b, PointNm::new(a.x, b.y)];
            Ok((vec![Polygon::new(ring, Vec::new())], None))
        }
    }
}
