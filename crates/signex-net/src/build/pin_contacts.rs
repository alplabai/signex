//! Where a sheet pin dropped on a parent sheet would silently join a net.

use signex_types::schematic::{Point, SchematicSheet};

use super::{point_on_segment, power_port_pins, pt_key};

/// Every point a sheet pin placed on `sheet` would connect to, derived next to
/// the netlist rules that make those connections so the two cannot drift:
/// wire ends and interiors, label anchors, junctions, no-connects, power-port
/// pin tips (transformed exactly as the power-port merge does) and the pins of
/// the sheet symbols already there (two pins on one point merge their nets).
#[derive(Debug, Default, Clone)]
pub struct SheetPinContacts {
    points: Vec<Point>,
    segments: Vec<(Point, Point)>,
}

impl SheetPinContacts {
    pub fn of_sheet(sheet: &SchematicSheet) -> Self {
        let mut points: Vec<Point> = sheet.labels.iter().map(|l| l.position).collect();
        points.extend(sheet.junctions.iter().map(|j| j.position));
        points.extend(sheet.no_connects.iter().map(|n| n.position));
        points.extend(power_port_pins(sheet).into_iter().map(|(tip, _)| tip));
        points.extend(
            sheet
                .child_sheets
                .iter()
                .flat_map(|cs| cs.pins.iter().map(|p| p.position)),
        );
        let segments = sheet.wires.iter().map(|w| (w.start, w.end)).collect();
        Self { points, segments }
    }

    /// True when a pin at `(x, y)` (mm) would connect to something.
    pub fn hits(&self, x: f64, y: f64) -> bool {
        let p = pt_key(&Point::new(x, y));
        self.points.iter().any(|q| pt_key(q) == p)
            || self
                .segments
                .iter()
                .any(|(a, b)| point_on_segment(p, pt_key(a), pt_key(b)))
    }
}
