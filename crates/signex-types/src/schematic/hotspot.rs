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
    MoveHotspots::new(sheet, moved_points, excluded, radius).snap(delta)
}

/// The hotspots a move can snap to, gathered once so each cursor step of a
/// drag only looks at the few near a landing point (#2077). The answer is the
/// one [`snap_move_to_hotspot`] gives for the same inputs.
#[derive(Debug, Clone)]
pub struct MoveHotspots {
    /// Candidates in the order the full scan meets them (symbol pin tips,
    /// wire ends, junctions), so equal distances break the same way.
    points: Vec<Point>,
    /// The moved connection points, before the move, this was built for.
    moved: Vec<Point>,
    radius: f64,
    /// Candidate indices by `radius`-sized cell.
    cells: std::collections::HashMap<(i64, i64), Vec<usize>>,
}

impl MoveHotspots {
    /// The hotspots of everything but `excluded`, less those on a moved
    /// point's own position, for capture within `radius`.
    #[must_use]
    pub fn new(
        sheet: &SchematicSheet,
        moved_points: &[Point],
        excluded: &[uuid::Uuid],
        radius: f64,
    ) -> Self {
        let excluded: std::collections::HashSet<uuid::Uuid> = excluded.iter().copied().collect();
        let mut points = Vec::new();
        let mut consider = |candidate: Point| {
            if !moved_points
                .iter()
                .any(|p| (p.x - candidate.x).abs() < 1e-6 && (p.y - candidate.y).abs() < 1e-6)
            {
                points.push(candidate);
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
        let mut cells: std::collections::HashMap<(i64, i64), Vec<usize>> =
            std::collections::HashMap::new();
        if Self::usable(radius) {
            for (index, point) in points.iter().enumerate() {
                cells
                    .entry(Self::cell(*point, radius))
                    .or_default()
                    .push(index);
            }
        }
        Self {
            points,
            moved: moved_points.to_vec(),
            radius,
            cells,
        }
    }

    fn usable(radius: f64) -> bool {
        radius > 0.0 && radius.is_finite()
    }

    fn cell(point: Point, radius: f64) -> (i64, i64) {
        (
            (point.x / radius).floor() as i64,
            (point.y / radius).floor() as i64,
        )
    }

    /// The hotspot nearest `point` strictly within the radius; the first in
    /// scan order of two equally near ones.
    fn nearest(&self, point: Point) -> Option<(f64, Point)> {
        if !Self::usable(self.radius) || !point.x.is_finite() || !point.y.is_finite() {
            return None;
        }
        let (cx, cy) = Self::cell(point, self.radius);
        let mut best: Option<(f64, usize)> = None;
        for gx in cx.saturating_sub(1)..=cx.saturating_add(1) {
            for gy in cy.saturating_sub(1)..=cy.saturating_add(1) {
                for &index in self.cells.get(&(gx, gy)).into_iter().flatten() {
                    let candidate = self.points[index];
                    let dist = (candidate.x - point.x).hypot(candidate.y - point.y);
                    if dist < self.radius
                        && best.is_none_or(|(d, i)| dist < d || (dist == d && index < i))
                    {
                        best = Some((dist, index));
                    }
                }
            }
        }
        best.map(|(dist, index)| (dist, self.points[index]))
    }

    /// [`snap_move_to_hotspot`] for the moved points this was built with,
    /// moved by `delta`.
    #[must_use]
    pub fn snap(&self, delta: (f64, f64)) -> (f64, f64) {
        let mut best: Option<(f64, (f64, f64))> = None;
        for p in &self.moved {
            let landing = Point::new(p.x + delta.0, p.y + delta.1);
            if let Some((dist, h)) = self.nearest(landing)
                && best.is_none_or(|(d, _)| dist < d)
            {
                best = Some((dist, (h.x - p.x, h.y - p.y)));
            }
        }
        best.map_or(delta, |(_, d)| d)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schematic::{
        Junction, LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, Symbol, Wire,
    };

    /// A sheet with one part, whose lib entry has pins at local (0, ±4):
    /// their tips are hotspots, off the grid the wires use.
    fn add_part_with_pins(sheet: &mut SchematicSheet, at: (f64, f64)) {
        let mut lib: LibSymbol =
            serde_json::from_value(serde_json::json!({ "id": "L:S" })).expect("lib symbol");
        lib.pins = [4.0, -4.0]
            .into_iter()
            .enumerate()
            .map(|(n, y)| LibPin {
                unit: 0,
                body_style: 1,
                pin: Pin {
                    direction: PinDirection::Passive,
                    shape_style: PinShapeStyle::Plain,
                    position: Point::new(0.0, y),
                    rotation: 0.0,
                    length: 1.27,
                    name: String::new(),
                    number: (n + 1).to_string(),
                    visible: true,
                    name_visible: true,
                    number_visible: true,
                },
            })
            .collect();
        sheet.lib_symbols.insert("L:S".into(), lib);
        sheet.symbols.push(Symbol {
            uuid: uuid::Uuid::from_u128(7_000_000),
            lib_id: "L:S".into(),
            position: Point::new(at.0, at.1),
            unit: 1,
            ..Default::default()
        });
    }

    fn sheet_with_wire(start: (f64, f64), end: (f64, f64)) -> SchematicSheet {
        let mut sheet = SchematicSheet::default();
        sheet.wires.push(Wire {
            unknown_columns: Default::default(),
            uuid: uuid::Uuid::now_v7(),
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

    /// The pre-index scan, kept as the oracle for [`MoveHotspots`].
    fn scan_snap(
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

    #[test]
    fn the_index_snaps_exactly_as_the_full_scan() {
        let mut sheet = SchematicSheet::default();
        // Wire ends across all four quadrants, with coincident ends (ties)
        // and junctions, some exactly one capture radius apart.
        for i in -6..6i32 {
            for j in -3..3i32 {
                let x = f64::from(i) * 2.3 + 0.07 * f64::from(j);
                let y = f64::from(j) * 3.1;
                let id = u128::try_from((i + 10) * 100 + j + 10).expect("positive id");
                sheet.wires.push(Wire {
                    unknown_columns: Default::default(),
                    uuid: uuid::Uuid::from_u128(id),
                    start: Point::new(x, y),
                    end: Point::new(x + 1.27, y + 0.635),
                    stroke_width: 0.0,
                    locked: false,
                });
                // A second wire ending where the first starts: a tie.
                sheet.wires.push(Wire {
                    unknown_columns: Default::default(),
                    uuid: uuid::Uuid::from_u128(id + 50_000),
                    start: Point::new(x - 1.27, y),
                    end: Point::new(x, y),
                    stroke_width: 0.0,
                    locked: false,
                });
                sheet.junctions.push(Junction {
                    uuid: uuid::Uuid::from_u128(id + 90_000),
                    position: Point::new(x + 0.635, y - 1.27),
                    ..Default::default()
                });
            }
        }
        // Pin tips are hotspots too, off the grid of the wires.
        add_part_with_pins(&mut sheet, (0.31, 0.17));
        // The first moved point sits on a hotspot of its own: the scan ignores
        // it, so the index must too.
        let moved = [Point::new(-6.0 * 2.3 - 0.21, -9.3), Point::new(1.27, 0.635)];
        let excluded = [uuid::Uuid::from_u128(410)];
        for radius in [0.635, 1.27] {
            let index = MoveHotspots::new(&sheet, &moved, &excluded, radius);
            for step_x in -60..60 {
                for step_y in -30..30 {
                    let delta = (f64::from(step_x) * 0.4, f64::from(step_y) * 0.45);
                    assert_eq!(
                        index.snap(delta),
                        scan_snap(&sheet, &moved, &excluded, delta, radius),
                        "delta {delta:?} radius {radius}"
                    );
                }
            }
            // A non-finite delta snaps nowhere, as the scan.
            assert_eq!(
                index.snap((f64::INFINITY, 0.0)).0.is_infinite(),
                scan_snap(&sheet, &moved, &excluded, (f64::INFINITY, 0.0), radius)
                    .0
                    .is_infinite()
            );
        }
    }

    /// A landing exactly one radius from a pin tip is not captured, as the
    /// full scan; just inside it is.
    #[test]
    fn the_index_and_the_scan_agree_on_a_pin_tip_exactly_at_the_radius() {
        let mut sheet = SchematicSheet::default();
        add_part_with_pins(&mut sheet, (0.5, 0.25));
        let tip = SymbolTransform::from_symbol(&sheet.symbols[0]).apply(Point::new(0.0, 4.0));
        let moved = [Point::new(10.0, 10.0)];
        let radius = 0.5;
        let index = MoveHotspots::new(&sheet, &moved, &[], radius);
        for (offset, captured) in [(0.5, false), (0.25, true)] {
            let delta = (tip.x + offset - 10.0, tip.y - 10.0);
            let want = scan_snap(&sheet, &moved, &[], delta, radius);
            assert_eq!(index.snap(delta), want, "offset {offset}");
            if captured {
                assert_eq!(want, (tip.x - 10.0, tip.y - 10.0), "snapped onto the tip");
            } else {
                assert_eq!(want, delta, "exactly one radius away is out");
            }
        }
    }
}
