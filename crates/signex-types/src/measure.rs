//! Point-to-point measurement (#1256): the numbers the Measure tool shows.

use crate::coord::Unit;
use crate::schematic::Point;

/// Millimetres per display unit.
fn mm_per(unit: Unit) -> f64 {
    match unit {
        Unit::Mm => 1.0,
        Unit::Mil => 0.0254,
        Unit::Inch => 25.4,
        Unit::Micrometer => 0.001,
    }
}

/// Decimals a length is shown with in `unit`.
fn decimals(unit: Unit) -> usize {
    match unit {
        Unit::Mm => 3,
        Unit::Mil => 2,
        Unit::Inch => 5,
        Unit::Micrometer => 1,
    }
}

/// A length in millimetres shown in `unit`, e.g. `2.540 mm` or `100.00 mil`.
pub fn format_length_mm(mm: f64, unit: Unit) -> String {
    format!("{:.*} {unit}", decimals(unit), mm / mm_per(unit))
}

/// The bare number of a length in `unit`, as an edit field shows it
/// (`2.540`, no unit suffix).
pub fn length_field_text(mm: f64, unit: Unit) -> String {
    format!("{:.*}", decimals(unit), mm / mm_per(unit))
}

/// The millimetres a length typed in `unit` stands for. `None` when `text`
/// is not a finite number.
pub fn parse_length_field_mm(text: &str, unit: Unit) -> Option<f64> {
    let value: f64 = text.trim().parse().ok()?;
    let mm = value * mm_per(unit);
    mm.is_finite().then_some(mm)
}

/// The offset between two points, in millimetres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Measurement {
    /// `b.x - a.x`.
    pub dx_mm: f64,
    /// `b.y - a.y`.
    pub dy_mm: f64,
}

impl Measurement {
    pub fn between(a: Point, b: Point) -> Self {
        Self {
            dx_mm: b.x - a.x,
            dy_mm: b.y - a.y,
        }
    }

    /// Straight-line distance, in millimetres.
    pub fn distance_mm(&self) -> f64 {
        self.dx_mm.hypot(self.dy_mm)
    }

    /// `dx 2.540 mm  dy 0.000 mm  dist 2.540 mm`, in `unit`.
    pub fn describe(&self, unit: Unit) -> String {
        format!(
            "dx {}  dy {}  dist {}",
            format_length_mm(self.dx_mm, unit),
            format_length_mm(self.dy_mm, unit),
            format_length_mm(self.distance_mm(), unit)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_3_4_5_triangle_measures_5_in_every_unit() {
        let m = Measurement::between(Point::new(1.0, 1.0), Point::new(4.0, 5.0));
        assert_eq!((m.dx_mm, m.dy_mm, m.distance_mm()), (3.0, 4.0, 5.0));
        assert_eq!(
            m.describe(Unit::Mm),
            "dx 3.000 mm  dy 4.000 mm  dist 5.000 mm"
        );
        assert_eq!(
            m.describe(Unit::Micrometer),
            "dx 3000.0 um  dy 4000.0 um  dist 5000.0 um"
        );
    }

    #[test]
    fn a_length_field_round_trips_through_its_unit() {
        assert_eq!(length_field_text(2.54, Unit::Mil), "100.00");
        let mil = parse_length_field_mm(" 100 ", Unit::Mil).unwrap();
        assert!((mil - 2.54).abs() < 1e-12);
        assert_eq!(parse_length_field_mm("1", Unit::Inch), Some(25.4));
        for bad in ["", "abc", "inf", "NaN", "1e999"] {
            assert_eq!(parse_length_field_mm(bad, Unit::Mm), None, "{bad:?}");
        }
    }

    #[test]
    fn mil_and_inch_convert_from_mm() {
        let m = Measurement::between(Point::new(0.0, 0.0), Point::new(2.54, 0.0));
        assert_eq!(
            m.describe(Unit::Mil),
            "dx 100.00 mil  dy 0.00 mil  dist 100.00 mil"
        );
        assert_eq!(
            m.describe(Unit::Inch),
            "dx 0.10000 in  dy 0.00000 in  dist 0.10000 in"
        );
    }

    #[test]
    fn deltas_are_signed_from_a_to_b() {
        let m = Measurement::between(Point::new(5.0, 5.0), Point::new(2.0, 9.0));
        assert_eq!((m.dx_mm, m.dy_mm), (-3.0, 4.0));
        assert_eq!(m.distance_mm(), 5.0);
    }
}
