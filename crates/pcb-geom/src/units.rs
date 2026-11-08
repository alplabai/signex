//! Integer-nanometre values and the conversion from the `f64`-mm model.

use signex_types::format::mm_as_stored_nm;
use signex_types::pcb::Point;

/// Nanometres in one millimetre.
pub const NM_PER_MM: i64 = 1_000_000;

/// Largest coordinate or length the geometry accepts: 1 km. Every squared
/// distance, cross product and R-tree area then fits an `i128`.
pub const MAX_ABS_NM: i64 = 1_000 * 1_000 * NM_PER_MM;

/// A model value the geometry cannot hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeomError {
    /// NaN, an infinity, or a length beyond [`MAX_ABS_NM`]. Carries the
    /// model field's name.
    Unrepresentable(&'static str),
    /// Two items of one board share an identity; the later one is left out.
    DuplicateItem,
}

impl std::fmt::Display for GeomError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GeomError::DuplicateItem => write!(f, "another item on the board has this identity"),
            GeomError::Unrepresentable(field) => {
                write!(
                    f,
                    "{field} is not a finite length within 1 km of the origin"
                )
            }
        }
    }
}

impl std::error::Error for GeomError {}

/// `mm` as integer nm, rounded the way a save rounds it, so geometry built
/// from a freshly loaded board and from the saved file agree to the nm.
pub fn mm_to_nm(mm: f64, field: &'static str) -> Result<i64, GeomError> {
    mm_as_stored_nm(mm)
        .filter(|nm| nm.unsigned_abs() <= MAX_ABS_NM.unsigned_abs())
        .ok_or(GeomError::Unrepresentable(field))
}

/// `nm` as `f64` mm, with the same arithmetic the file loader uses.
pub fn nm_to_mm(nm: i64) -> f64 {
    (nm as f64) * 1.0e-6
}

/// A board point in integer nanometres.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct PointNm {
    pub x: i64,
    pub y: i64,
}

impl PointNm {
    pub const ORIGIN: PointNm = PointNm { x: 0, y: 0 };

    pub const fn new(x: i64, y: i64) -> Self {
        PointNm { x, y }
    }

    /// Convert a model point; `field` names it in the error.
    pub fn from_mm(point: Point, field: &'static str) -> Result<Self, GeomError> {
        Ok(PointNm::new(
            mm_to_nm(point.x, field)?,
            mm_to_nm(point.y, field)?,
        ))
    }

    pub fn to_mm(self) -> Point {
        Point::new(nm_to_mm(self.x), nm_to_mm(self.y))
    }

    pub const fn plus(self, other: PointNm) -> PointNm {
        PointNm::new(self.x + other.x, self.y + other.y)
    }

    pub const fn minus(self, other: PointNm) -> PointNm {
        PointNm::new(self.x - other.x, self.y - other.y)
    }

    /// Squared distance, exact.
    pub fn distance_sq(self, other: PointNm) -> i128 {
        let dx = i128::from(self.x - other.x);
        let dy = i128::from(self.y - other.y);
        dx * dx + dy * dy
    }

    /// Dot product of two vectors, exact.
    pub fn dot(self, other: PointNm) -> i128 {
        i128::from(self.x) * i128::from(other.x) + i128::from(self.y) * i128::from(other.y)
    }

    /// Z of the cross product of two vectors, exact.
    pub fn cross(self, other: PointNm) -> i128 {
        i128::from(self.x) * i128::from(other.y) - i128::from(self.y) * i128::from(other.x)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mm_rounds_to_the_nearest_nm() {
        assert_eq!(mm_to_nm(3.81, "x"), Ok(3_810_000));
        assert_eq!(mm_to_nm(-0.000_000_4, "x"), Ok(0));
        assert_eq!(mm_to_nm(0.000_000_6, "x"), Ok(1));
    }

    #[test]
    fn non_finite_and_out_of_range_are_refused_with_the_field_name() {
        for bad in [
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            1.0e6 + 1.0,
            -1.0e7,
        ] {
            assert_eq!(
                mm_to_nm(bad, "via.position"),
                Err(GeomError::Unrepresentable("via.position"))
            );
        }
        assert_eq!(mm_to_nm(1.0e6, "x"), Ok(MAX_ABS_NM));
        assert_eq!(mm_to_nm(-1.0e6, "x"), Ok(-MAX_ABS_NM));
    }

    #[test]
    fn a_point_round_trips_through_nm() {
        let p = PointNm::from_mm(Point::new(12.5, -7.25), "p").unwrap();
        assert_eq!(p, PointNm::new(12_500_000, -7_250_000));
        assert_eq!(p.to_mm(), Point::new(12.5, -7.25));
        assert!(PointNm::from_mm(Point::new(0.0, f64::NAN), "p").is_err());
    }

    #[test]
    fn products_at_the_range_limit_do_not_overflow() {
        let a = PointNm::new(-MAX_ABS_NM, -MAX_ABS_NM);
        let b = PointNm::new(MAX_ABS_NM, MAX_ABS_NM);
        let span = b.minus(a);
        let max = i128::from(MAX_ABS_NM);
        assert_eq!(a.distance_sq(b), 8 * max * max);
        assert_eq!(span.dot(span), a.distance_sq(b));
        assert_eq!(span.cross(span), 0);
    }

    #[test]
    fn error_message_names_the_field() {
        let msg = GeomError::Unrepresentable("pad.size").to_string();
        assert!(msg.contains("pad.size"));
    }
}
