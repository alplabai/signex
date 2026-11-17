//! The fabrication frame and the number formats every fabrication file
//! shares. Frame and units: see `docs/FABRICATION_OUTPUTS.md`, "Frame".

use pcb_geom::{GeomError, PointNm};
use signex_types::pcb::{PcbBoard, Point};

/// Millionths in one unit: nm per mm, micro-degrees per degree.
const MILLION: i64 = 1_000_000;

/// The fabrication frame of one board: x to the right and y up, in nm from
/// the board origin (#2384), the coordinates the board shows. The board
/// model's y grows down the screen, so the frame mirrors it about the
/// origin; this is the one place that rule lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FabFrame {
    pub(crate) origin: PointNm,
}

impl FabFrame {
    /// The frame of `board`. An origin the geometry cannot hold (not
    /// finite, or beyond 1 km) counts from the model's zero, as a board
    /// saved without one does.
    pub(crate) fn of(board: &PcbBoard) -> Self {
        let origin =
            PointNm::from_mm(board.options.origin.at, "board.origin").unwrap_or(PointNm::new(0, 0));
        FabFrame { origin }
    }

    /// A model point in the frame.
    pub(crate) fn point(self, model: PointNm) -> PointNm {
        PointNm::new(model.x - self.origin.x, self.origin.y - model.y)
    }

    /// A model point in mm, in the frame.
    pub(crate) fn of_mm(self, model: Point, what: &'static str) -> Result<PointNm, GeomError> {
        Ok(self.point(PointNm::from_mm(model, what)?))
    }
}

/// A model angle (degrees, counter-clockwise in the model's axes) as the
/// counter-clockwise angle in the fabrication frame, in micro-degrees in
/// `0..360_000_000`. Negating y reverses the sense of turn.
pub(crate) fn fab_angle_udeg(model_deg: f64) -> i64 {
    let turned = (-model_deg).rem_euclid(360.0);
    let udeg = (turned * MILLION as f64).round() as i64;
    udeg.rem_euclid(360 * MILLION)
}

/// Whether a fabrication-frame angle is a whole quarter turn.
pub(crate) fn is_quarter_turn(udeg: i64) -> bool {
    udeg % (90 * MILLION) == 0
}

/// Whether a quarter-turn angle swaps a shape's width and height.
pub(crate) fn swaps_axes(udeg: i64) -> bool {
    udeg == 90 * MILLION || udeg == 270 * MILLION
}

/// A value in millionths as a decimal with no trailing zeros:
/// `1_250_000` is `1.25`, `-500` is `-0.0005`, `2_000_000` is `2`.
pub(crate) fn millionths(value: i64) -> String {
    let sign = if value < 0 { "-" } else { "" };
    let abs = value.unsigned_abs();
    let whole = abs / MILLION as u64;
    let frac = abs % MILLION as u64;
    if frac == 0 {
        return format!("{sign}{whole}");
    }
    let digits = format!("{frac:06}");
    format!("{sign}{whole}.{}", digits.trim_end_matches('0'))
}

/// `value / divisor` rounded half away from zero.
pub(crate) fn div_round(value: i64, divisor: i64) -> i64 {
    let quotient = value / divisor;
    let rest = value % divisor;
    if 2 * rest.abs() >= divisor {
        quotient + value.signum()
    } else {
        quotient
    }
}

/// A length in nm as mm rounded to the micrometre, three decimals always:
/// `1_250_000` is `1.250`, `-4_999_600` is `-5.000`.
pub(crate) fn mm3(nm: i64) -> String {
    thousandths(div_round(nm, 1_000))
}

/// A fabrication-frame angle in micro-degrees as degrees with three
/// decimals: `90_000_000` is `90.000`.
pub(crate) fn deg3(udeg: i64) -> String {
    thousandths(div_round(udeg, 1_000).rem_euclid(360_000))
}

fn thousandths(value: i64) -> String {
    let sign = if value < 0 { "-" } else { "" };
    let abs = value.unsigned_abs();
    format!("{sign}{}.{:03}", abs / 1_000, abs % 1_000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_frame_turns_y_up_and_reverses_the_turn() {
        let at_zero = FabFrame {
            origin: PointNm::new(0, 0),
        };
        assert_eq!(at_zero.point(PointNm::new(3, 4)), PointNm::new(3, -4));
        // From an origin at (0, 80) mm, the model point (10, 70) mm is 10 mm
        // right of it and 10 mm up.
        let bottom_left = FabFrame {
            origin: PointNm::new(0, 80_000_000),
        };
        assert_eq!(
            bottom_left.point(PointNm::new(10_000_000, 70_000_000)),
            PointNm::new(10_000_000, 10_000_000)
        );
        assert_eq!(fab_angle_udeg(0.0), 0);
        assert_eq!(fab_angle_udeg(-0.0), 0);
        assert_eq!(fab_angle_udeg(90.0), 270_000_000);
        assert_eq!(fab_angle_udeg(-45.0), 45_000_000);
        assert_eq!(fab_angle_udeg(360.0), 0);
    }

    #[test]
    fn numbers_print_without_locale_or_float_noise() {
        assert_eq!(millionths(1_250_000), "1.25");
        assert_eq!(millionths(-500), "-0.0005");
        assert_eq!(millionths(2_000_000), "2");
        assert_eq!(millionths(0), "0");
        assert_eq!(mm3(1_250_000), "1.250");
        assert_eq!(mm3(-4_999_600), "-5.000");
        assert_eq!(mm3(-400), "0.000");
        assert_eq!(deg3(270_000_000), "270.000");
        assert_eq!(deg3(359_999_999), "0.000");
    }

    #[test]
    fn quarter_turns_are_recognised() {
        assert!(is_quarter_turn(0));
        assert!(is_quarter_turn(270_000_000));
        assert!(!is_quarter_turn(45_000_000));
        assert!(swaps_axes(90_000_000));
        assert!(!swaps_axes(180_000_000));
    }
}
