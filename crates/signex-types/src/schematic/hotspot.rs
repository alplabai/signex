//! Electrical hotspots: the points a wire can connect to without a grid
//! point lying under them.
//!
//! A pin tip, a wire end or a junction is where connectivity happens, and it
//! need not sit on the snap grid (a part drawn on another grid, or after a
//! grid change). [`snap_to_hotspot`] finds the nearest one to a cursor, so a
//! wire click lands exactly on it instead of on the nearest grid dot beside it.

use super::{Point, SchematicSheet, SymbolTransform, instance_pins};

/// Default capture radius (mm): one 50 mil step around a hotspot.
pub const SCHEMATIC_HOTSPOT_RADIUS_MM: f64 = 1.27;

/// Slack (mm) on the radius, so a point exactly one radius away still
/// captures despite decimal rounding.
const HOTSPOT_RADIUS_TOLERANCE_MM: f64 = 1e-6;

/// The electrical hotspot nearest to `point` within `radius` (mm), if any.
///
/// Candidates are every placed symbol pin tip (of the instance's unit), both
/// ends of every wire and every junction. The first of two equally near
/// hotspots wins, so the result is deterministic.
pub fn snap_to_hotspot(sheet: &SchematicSheet, point: Point, radius: f64) -> Option<Point> {
    let mut best: Option<(f64, Point)> = None;
    let mut consider = |candidate: Point| {
        let dist = (candidate.x - point.x).hypot(candidate.y - point.y);
        if dist <= radius + HOTSPOT_RADIUS_TOLERANCE_MM && best.is_none_or(|(d, _)| dist < d) {
            best = Some((dist, candidate));
        }
    };
    for sym in &sheet.symbols {
        let Some(lib) = sheet.lib_symbols.get(&sym.lib_id) else {
            continue;
        };
        let xform = SymbolTransform::from_symbol(sym);
        for lp in instance_pins(lib, sym) {
            consider(xform.apply(lp.pin.position));
        }
    }
    for wire in &sheet.wires {
        consider(wire.start);
        consider(wire.end);
    }
    for junction in &sheet.junctions {
        consider(junction.position);
    }
    best.map(|(_, p)| p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schematic::Wire;

    fn sheet_with_wire(start: (f64, f64), end: (f64, f64)) -> SchematicSheet {
        let mut sheet = SchematicSheet::default();
        sheet.wires.push(Wire {
            unknown_columns: Default::default(),
            uuid: uuid::Uuid::new_v4(),
            start: Point::new(start.0, start.1),
            end: Point::new(end.0, end.1),
            stroke_width: 0.0,
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
}
