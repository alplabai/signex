//! Electrical hotspots: the points a wire can connect to without a grid
//! point lying under them.
//!
//! A pin tip, a wire end or a junction is where connectivity happens, and it
//! need not sit on the snap grid (a part drawn on another grid, or after a
//! grid change). [`snap_to_hotspot`] finds the nearest one to a cursor, so a
//! wire click lands exactly on it instead of on the nearest grid dot beside it.

use super::{Point, SchematicSheet, SymbolTransform, instance_pins};

/// Upper bound on the capture radius (mm): one 50 mil step around a hotspot.
pub const SCHEMATIC_HOTSPOT_RADIUS_MM: f64 = 1.27;

/// The capture radius (mm) to use with a snap grid of `grid_mm` (`0.0` or
/// less when snapping is off): [`SCHEMATIC_HOTSPOT_RADIUS_MM`], capped at half
/// a grid step. The radius must stay under one step, or a click on the grid
/// point next to a hotspot would be captured by the hotspot and join a net
/// the user never meant to touch.
pub fn hotspot_capture_radius(grid_mm: f64) -> f64 {
    if grid_mm > 0.0 {
        SCHEMATIC_HOTSPOT_RADIUS_MM.min(0.5 * grid_mm)
    } else {
        SCHEMATIC_HOTSPOT_RADIUS_MM
    }
}

/// The electrical hotspot nearest to `point` strictly within `radius` (mm), if any.
///
/// Candidates are every placed symbol pin tip (of the instance's unit), both
/// ends of every wire and every junction. The first of two equally near
/// hotspots wins, so the result is deterministic.
pub fn snap_to_hotspot(sheet: &SchematicSheet, point: Point, radius: f64) -> Option<Point> {
    nearest_hotspot(sheet, point, radius, &[], &[]).map(|(_, p)| p)
}

/// [`snap_to_hotspot`] ignoring the hotspots of the items in `excluded`
/// (symbol, wire and junction ids), returning the distance with the point.
fn nearest_hotspot(
    sheet: &SchematicSheet,
    point: Point,
    radius: f64,
    excluded: &[uuid::Uuid],
    ignore_at: &[Point],
) -> Option<(f64, Point)> {
    let mut best: Option<(f64, Point)> = None;
    let mut consider = |candidate: Point| {
        if ignore_at
            .iter()
            .any(|p| (p.x - candidate.x).abs() < 1e-6 && (p.y - candidate.y).abs() < 1e-6)
        {
            return;
        }
        let dist = (candidate.x - point.x).hypot(candidate.y - point.y);
        if dist < radius && best.is_none_or(|(d, _)| dist < d) {
            best = Some((dist, candidate));
        }
    };
    for sym in sheet.symbols.iter().filter(|s| !excluded.contains(&s.uuid)) {
        let Some(lib) = sheet.resolve_lib_symbol(sym) else {
            continue;
        };
        let xform = SymbolTransform::from_symbol(sym);
        for lp in instance_pins(&lib, sym) {
            consider(xform.apply(lp.pin.position));
        }
    }
    for wire in sheet.wires.iter().filter(|w| !excluded.contains(&w.uuid)) {
        consider(wire.start);
        consider(wire.end);
    }
    for junction in sheet
        .junctions
        .iter()
        .filter(|j| !excluded.contains(&j.uuid))
    {
        consider(junction.position);
    }
    best
}

/// The move delta `(dx, dy)` adjusted so a moved connection point lands
/// exactly on a hotspot of an item that is not moving.
///
/// `moved_points` are the moved items' connection points (pin tips, wire
/// ends) before the move; `excluded` are the moved items' ids, whose own
/// hotspots travel with them and are never targets. Of all moved points that
/// would land strictly within `radius` of a hotspot, the closest wins; with
/// none in range the proposed delta is returned unchanged. A hotspot sitting
/// on a moved point's original position (a wire attached to a moved pin) is
/// never a target, or a small move would snap the pin back where it was.
pub fn snap_move_to_hotspot(
    sheet: &SchematicSheet,
    moved_points: &[Point],
    excluded: &[uuid::Uuid],
    delta: (f64, f64),
    radius: f64,
) -> (f64, f64) {
    let mut best: Option<(f64, (f64, f64))> = None;
    for p in moved_points {
        let landing = Point::new(p.x + delta.0, p.y + delta.1);
        if let Some((dist, h)) = nearest_hotspot(sheet, landing, radius, excluded, moved_points)
            && best.is_none_or(|(d, _)| dist < d)
        {
            best = Some((dist, (h.x - p.x, h.y - p.y)));
        }
    }
    best.map_or(delta, |(_, d)| d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schematic::{Junction, Wire};

    fn sheet_with_wire(start: (f64, f64), end: (f64, f64)) -> SchematicSheet {
        let mut sheet = SchematicSheet::default();
        sheet.wires.push(Wire {
            unknown_columns: Default::default(),
            uuid: uuid::Uuid::new_v4(),
            start: Point::new(start.0, start.1),
            end: Point::new(end.0, end.1),
            stroke_width: 0.0,
            locked: false,
        });
        sheet
    }

    #[test]
    fn wire_end_within_radius_is_returned_exactly() {
        let sheet = sheet_with_wire((0.0, 0.0), (10.3, 5.1));
        let p = snap_to_hotspot(&sheet, Point::new(10.9, 5.4), 1.27).expect("in radius");
        assert_eq!((p.x, p.y), (10.3, 5.1));
    }

    #[test]
    fn nothing_beyond_the_radius() {
        let sheet = sheet_with_wire((0.0, 0.0), (10.3, 5.1));
        assert!(snap_to_hotspot(&sheet, Point::new(13.0, 5.1), 1.27).is_none());
    }

    #[test]
    fn nearer_hotspot_wins() {
        let sheet = sheet_with_wire((0.0, 0.0), (1.0, 0.0));
        let p = snap_to_hotspot(&sheet, Point::new(0.8, 0.0), 1.27).expect("in radius");
        assert_eq!((p.x, p.y), (1.0, 0.0));
    }

    #[test]
    fn junction_is_a_hotspot() {
        let mut sheet = SchematicSheet::default();
        sheet.junctions.push(Junction {
            position: Point::new(4.2, 3.3),
            ..Default::default()
        });
        let p = snap_to_hotspot(&sheet, Point::new(4.5, 3.0), 1.27).expect("in radius");
        assert_eq!((p.x, p.y), (4.2, 3.3));
    }

    #[test]
    fn a_point_exactly_one_radius_away_does_not_capture() {
        let sheet = sheet_with_wire((0.0, 0.0), (10.0, 0.0));
        let p = snap_to_hotspot(&sheet, Point::new(11.24, 0.0), 1.25).expect("inside");
        assert_eq!((p.x, p.y), (10.0, 0.0));
        assert!(snap_to_hotspot(&sheet, Point::new(11.25, 0.0), 1.25).is_none());
    }

    #[test]
    fn the_radius_stays_under_one_grid_step() {
        assert_eq!(hotspot_capture_radius(1.27), 0.635);
        assert_eq!(hotspot_capture_radius(0.254), 0.127);
        assert_eq!(hotspot_capture_radius(5.0), SCHEMATIC_HOTSPOT_RADIUS_MM);
        assert_eq!(hotspot_capture_radius(0.0), SCHEMATIC_HOTSPOT_RADIUS_MM);
    }

    #[test]
    fn a_grid_point_next_to_a_hotspot_is_not_captured() {
        let sheet = sheet_with_wire((-5.0, 0.0), (0.0, 0.0));
        for grid in [1.27, 0.254] {
            let radius = hotspot_capture_radius(grid);
            assert!(snap_to_hotspot(&sheet, Point::new(grid, 0.0), radius).is_none());
        }
    }

    #[test]
    fn equally_near_hotspots_resolve_to_the_first() {
        let sheet = sheet_with_wire((0.0, 0.0), (2.0, 0.0));
        let p = snap_to_hotspot(&sheet, Point::new(1.0, 0.0), 1.27).expect("in radius");
        assert_eq!((p.x, p.y), (0.0, 0.0));
    }

    #[test]
    fn an_off_grid_moved_point_is_pulled_onto_a_hotspot() {
        let sheet = sheet_with_wire((0.0, 0.0), (10.3, 5.1));
        // A moved point at (2.0, 0.0) with a grid-snapped delta of (8.0, 5.0)
        // would land at (10.0, 5.0), 0.32 mm from the wire end.
        let d = snap_move_to_hotspot(&sheet, &[Point::new(2.0, 0.0)], &[], (8.0, 5.0), 0.635);
        assert_eq!(d, (10.3 - 2.0, 5.1));
    }

    #[test]
    fn a_delta_with_nothing_in_radius_is_unchanged() {
        let sheet = sheet_with_wire((0.0, 0.0), (10.3, 5.1));
        let d = snap_move_to_hotspot(&sheet, &[Point::new(2.0, 0.0)], &[], (3.0, 3.0), 0.635);
        assert_eq!(d, (3.0, 3.0));
    }

    #[test]
    fn the_moved_items_own_hotspots_are_ignored() {
        let sheet = sheet_with_wire((0.0, 0.0), (10.3, 5.1));
        let id = sheet.wires[0].uuid;
        // A moved point 0.5 mm from the wire's own end lands 0.61 mm from it.
        let pts = [Point::new(0.5, 0.0)];
        let d = snap_move_to_hotspot(&sheet, &pts, &[id], (0.1, 0.1), 0.635);
        assert_eq!(d, (0.1, 0.1));
        let d = snap_move_to_hotspot(&sheet, &pts, &[], (0.1, 0.1), 0.635);
        assert_eq!(d, (-0.5, 0.0));
    }

    #[test]
    fn a_wire_end_on_the_moved_pin_does_not_cancel_a_small_move() {
        // Another wire end sits on the moved point's original position.
        let sheet = sheet_with_wire((0.0, 0.0), (10.0, 5.0));
        let d = snap_move_to_hotspot(&sheet, &[Point::new(10.0, 5.0)], &[], (0.4, 0.2), 1.27);
        assert_eq!(d, (0.4, 0.2));
    }
}
