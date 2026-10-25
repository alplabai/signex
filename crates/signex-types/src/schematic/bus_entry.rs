//! Bus-entry geometry: the orientation stored in [`BusEntry::size`] and the
//! snap of a placement point onto a bus.
//!
//! A bus entry runs from [`BusEntry::position`] (the end that sits on the bus)
//! to `position + size`. The signs of `size` are the orientation, so a rotation
//! or a mirror only rewrites `size` and the file format gains no field: an
//! untouched old file keeps its bytes, and the old default `(2.54, 2.54)`
//! simply reads as "down and to the right". The entry always turns about its
//! bus end, which is what keeps it attached to the bus.

use super::{Bus, BusEntry, Point};

/// Default leg of a freshly placed bus entry on each axis (mm), one 100 mil
/// step. The signs of the leg are chosen by rotating or mirroring.
pub const SCHEMATIC_BUS_ENTRY_SIZE_MM: f64 = 2.54;

/// Default orientation and size of a new bus entry: down and to the right.
pub const SCHEMATIC_BUS_ENTRY_DEFAULT_SIZE: (f64, f64) =
    (SCHEMATIC_BUS_ENTRY_SIZE_MM, SCHEMATIC_BUS_ENTRY_SIZE_MM);

impl BusEntry {
    /// The end that does not sit on the bus.
    pub fn end(&self) -> Point {
        Point::new(self.position.x + self.size.0, self.position.y + self.size.1)
    }

    /// `size` after `quarter_turns` 90 degree turns counter-clockwise on
    /// screen (negative turns go clockwise). The schematic's y axis points
    /// down, so one turn maps `(dx, dy)` to `(dy, -dx)`.
    pub fn rotated_size(size: (f64, f64), quarter_turns: i32) -> (f64, f64) {
        let mut out = size;
        for _ in 0..quarter_turns.rem_euclid(4) {
            out = (out.1, 0.0 - out.0);
        }
        out
    }

    /// `size` flipped left-right (the `X` key): the entry leans the other way
    /// in x while its bus end stays put.
    pub fn flipped_horizontal(size: (f64, f64)) -> (f64, f64) {
        (0.0 - size.0, size.1)
    }

    /// `size` flipped top-bottom (the `Y` key).
    pub fn flipped_vertical(size: (f64, f64)) -> (f64, f64) {
        (size.0, 0.0 - size.1)
    }
}

/// The point on the nearest bus segment to `point`, if one lies within
/// `radius` (mm). The first of two equally near buses wins.
pub fn snap_to_bus(buses: &[Bus], point: Point, radius: f64) -> Option<Point> {
    let mut best: Option<(f64, Point)> = None;
    for bus in buses {
        let on_bus = closest_on_segment(bus.start, bus.end, point);
        let dist = (on_bus.x - point.x).hypot(on_bus.y - point.y);
        if dist <= radius && best.is_none_or(|(d, _)| dist < d) {
            best = Some((dist, on_bus));
        }
    }
    best.map(|(_, p)| p)
}

fn closest_on_segment(a: Point, b: Point, p: Point) -> Point {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len_sq = dx * dx + dy * dy;
    if len_sq <= 0.0 {
        return a;
    }
    let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / len_sq).clamp(0.0, 1.0);
    Point::new(a.x + t * dx, a.y + t * dy)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn bus(sx: f64, sy: f64, ex: f64, ey: f64) -> Bus {
        Bus {
            uuid: Uuid::new_v4(),
            start: Point::new(sx, sy),
            end: Point::new(ex, ey),
        }
    }

    #[test]
    fn snaps_onto_a_horizontal_bus() {
        let buses = [bus(0.0, 10.0, 50.0, 10.0)];
        let p = snap_to_bus(&buses, Point::new(20.0, 11.27), 2.54).expect("within radius");
        assert_eq!((p.x, p.y), (20.0, 10.0));
    }

    #[test]
    fn snaps_onto_a_vertical_bus() {
        let buses = [bus(5.0, 0.0, 5.0, 40.0)];
        let p = snap_to_bus(&buses, Point::new(3.0, 15.0), 2.54).expect("within radius");
        assert_eq!((p.x, p.y), (5.0, 15.0));
    }

    #[test]
    fn does_not_snap_beyond_the_radius_and_clamps_to_the_ends() {
        let buses = [bus(0.0, 10.0, 50.0, 10.0)];
        assert!(snap_to_bus(&buses, Point::new(20.0, 14.0), 2.54).is_none());
        let p = snap_to_bus(&buses, Point::new(51.0, 10.5), 2.54).expect("near the end");
        assert_eq!((p.x, p.y), (50.0, 10.0));
    }

    #[test]
    fn picks_the_nearer_of_two_buses() {
        let buses = [bus(0.0, 10.0, 50.0, 10.0), bus(0.0, 12.0, 50.0, 12.0)];
        let p = snap_to_bus(&buses, Point::new(20.0, 11.5), 2.54).expect("within radius");
        assert_eq!(p.y, 12.0);
    }

    #[test]
    fn four_quarter_turns_visit_every_diagonal_and_return() {
        let s = SCHEMATIC_BUS_ENTRY_DEFAULT_SIZE;
        let k = SCHEMATIC_BUS_ENTRY_SIZE_MM;
        assert_eq!(BusEntry::rotated_size(s, 1), (k, -k));
        assert_eq!(BusEntry::rotated_size(s, 2), (-k, -k));
        assert_eq!(BusEntry::rotated_size(s, 3), (-k, k));
        assert_eq!(BusEntry::rotated_size(s, 4), s);
        assert_eq!(BusEntry::rotated_size(s, -1), (-k, k));
    }

    #[test]
    fn flips_negate_one_axis_and_never_make_negative_zero() {
        let k = SCHEMATIC_BUS_ENTRY_SIZE_MM;
        assert_eq!(BusEntry::flipped_horizontal((k, k)), (-k, k));
        assert_eq!(BusEntry::flipped_vertical((k, k)), (k, -k));
        let flat = BusEntry::flipped_vertical((k, 0.0));
        assert!(flat.1.is_sign_positive(), "0.0 must not become -0.0");
    }
}
