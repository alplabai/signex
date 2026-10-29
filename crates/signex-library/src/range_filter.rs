//! Range filter over a measurement column: the one predicate the library
//! browser and the Place Component picker share (#1572, phase 4).
//!
//! A filter is an operator (`>=`, `<=`, `=`, between) and one or two
//! [`Quantity`] bounds in a fixed [`Unit`], the column's. Every comparison
//! is in base units through [`ParamValue::as_quantity`] and
//! [`compare_for_sort`], so a legacy `{4.7, "kΩ"}` and a canonical
//! `{4700, "ohm"}` are the same value to it.
//!
//! A row whose cell is not a measurement in the filter's unit (empty, text,
//! a plain number, or another unit family) has no value to compare. It is
//! [`RangeVerdict::Excluded`], not silently "outside": callers drop it
//! while the filter is active and must tell the user how many were dropped.
//!
//! Bounds are inclusive. `=` uses [`EQUALS_RELATIVE_TOLERANCE`].

use std::cmp::Ordering;

use signex_types::quantity::{Quantity, Unit, compare_for_sort};

use crate::param::ParamValue;

/// `=` matches when the two base values differ by at most this fraction of
/// the larger magnitude: 0.5 %.
///
/// E-series values are stored rounded (4.99 k, 33.2 k) and typed rounded,
/// and the decimal-string join in `quantity_from_parts` can still land a
/// ulp apart from a value derived another way. 0.5 % absorbs both while
/// staying under half the spacing of the finest series in use, E192
/// (10^(1/192) - 1 = 1.2 %), so two neighbouring E192 values never match
/// each other. Zero equals only zero.
pub const EQUALS_RELATIVE_TOLERANCE: f64 = 0.005;

/// The comparison a [`RangeFilter`] makes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RangeOp {
    /// value >= bound
    AtLeast,
    /// value <= bound
    AtMost,
    /// value = bound, within [`EQUALS_RELATIVE_TOLERANCE`]
    Equals,
    /// low <= value <= high
    Between,
}

impl RangeOp {
    /// Picker order.
    pub const ALL: [RangeOp; 4] = [
        RangeOp::AtLeast,
        RangeOp::AtMost,
        RangeOp::Equals,
        RangeOp::Between,
    ];

    /// Whether the operator takes a second bound.
    pub fn is_between(self) -> bool {
        self == RangeOp::Between
    }
}

impl std::fmt::Display for RangeOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            RangeOp::AtLeast => "≥",
            RangeOp::AtMost => "≤",
            RangeOp::Equals => "=",
            RangeOp::Between => "between",
        })
    }
}

/// What a filter makes of one cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RangeVerdict {
    /// A measurement in the filter's unit, inside the range.
    Matches,
    /// A measurement in the filter's unit, outside the range.
    Outside,
    /// No comparable value: not a measurement in the filter's unit.
    Excluded,
}

/// A validated range over one unit. Build with [`RangeFilter::new`].
#[derive(Clone, Debug, PartialEq)]
pub struct RangeFilter {
    op: RangeOp,
    low: Quantity,
    high: Quantity,
}

impl RangeFilter {
    /// `high` is required for [`RangeOp::Between`] and ignored otherwise.
    /// `None` if a bound is missing or the two bounds are in different
    /// units. A reversed pair is put in order.
    pub fn new(op: RangeOp, low: Quantity, high: Option<Quantity>) -> Option<RangeFilter> {
        if !op.is_between() {
            return Some(RangeFilter {
                op,
                high: low.clone(),
                low,
            });
        }
        let high = high?;
        if low.unit() != high.unit() {
            return None;
        }
        let (low, high) = if compare_for_sort(&low, &high) == Ordering::Greater {
            (high, low)
        } else {
            (low, high)
        };
        Some(RangeFilter { op, low, high })
    }

    pub fn op(&self) -> RangeOp {
        self.op
    }

    /// The unit every compared value must be in.
    pub fn unit(&self) -> &Unit {
        self.low.unit()
    }

    /// Judge `value`, a row's cell (`None` when the row has no such key).
    pub fn verdict(&self, value: Option<&ParamValue>) -> RangeVerdict {
        let Some(q) = value.and_then(ParamValue::as_quantity) else {
            return RangeVerdict::Excluded;
        };
        if q.unit() != self.unit() {
            return RangeVerdict::Excluded;
        }
        if self.accepts(&q) {
            RangeVerdict::Matches
        } else {
            RangeVerdict::Outside
        }
    }

    fn accepts(&self, q: &Quantity) -> bool {
        let at_or_above_low = compare_for_sort(q, &self.low) != Ordering::Less;
        let at_or_below_high = compare_for_sort(q, &self.high) != Ordering::Greater;
        match self.op {
            RangeOp::AtLeast => at_or_above_low,
            RangeOp::AtMost => at_or_below_high,
            RangeOp::Between => at_or_above_low && at_or_below_high,
            RangeOp::Equals => approx_equal(q.base(), self.low.base()),
        }
    }
}

fn approx_equal(a: f64, b: f64) -> bool {
    if a == b {
        return true;
    }
    (a - b).abs() <= EQUALS_RELATIVE_TOLERANCE * a.abs().max(b.abs())
}

/// The unit a column's range filter works in: the unit most of its
/// measurements are in (ties go to the earlier [`Unit::STANDARD`] entry,
/// then the unit token). `None` when no cell is a measurement, so there is
/// nothing to filter.
pub fn column_unit<'a>(values: impl IntoIterator<Item = &'a ParamValue>) -> Option<Unit> {
    let mut counts: Vec<(Unit, usize)> = Vec::new();
    for q in values.into_iter().filter_map(ParamValue::as_quantity) {
        match counts.iter_mut().find(|(u, _)| u == q.unit()) {
            Some((_, n)) => *n += 1,
            None => counts.push((q.unit().clone(), 1)),
        }
    }
    let rank = |u: &Unit| {
        Unit::STANDARD
            .iter()
            .position(|s| s == u)
            .unwrap_or(Unit::STANDARD.len())
    };
    counts
        .into_iter()
        .min_by(|(ua, na), (ub, nb)| {
            nb.cmp(na)
                .then_with(|| rank(ua).cmp(&rank(ub)))
                .then_with(|| ua.token().cmp(ub.token()))
        })
        .map(|(unit, _)| unit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::quantity::SiPrefix;

    fn m(value: f64, unit: &str) -> ParamValue {
        ParamValue::Measurement {
            value,
            unit: unit.into(),
        }
    }

    fn q(mantissa: &str, prefix: SiPrefix, unit: Unit) -> Quantity {
        signex_types::quantity::quantity_from_parts(mantissa, prefix, unit).unwrap()
    }

    fn at_least_1k() -> RangeFilter {
        RangeFilter::new(RangeOp::AtLeast, q("1", SiPrefix::Kilo, Unit::Ohm), None).unwrap()
    }

    #[test]
    fn at_least_one_kilohm_keeps_canonical_and_legacy_and_drops_100_ohm() {
        let f = at_least_1k();
        for kept in [
            m(1000.0, "ohm"),
            m(4700.0, "ohm"),
            m(4.7, "kΩ"),
            m(1.0, "kΩ"),
        ] {
            assert_eq!(f.verdict(Some(&kept)), RangeVerdict::Matches, "{kept:?}");
        }
        assert_eq!(f.verdict(Some(&m(100.0, "ohm"))), RangeVerdict::Outside);
    }

    #[test]
    fn between_is_inclusive_and_orders_a_reversed_pair() {
        let (lo, hi) = (
            q("10", SiPrefix::Nano, Unit::Farad),
            q("1", SiPrefix::Micro, Unit::Farad),
        );
        for f in [
            RangeFilter::new(RangeOp::Between, lo.clone(), Some(hi.clone())).unwrap(),
            RangeFilter::new(RangeOp::Between, hi, Some(lo)).unwrap(),
        ] {
            assert_eq!(f.verdict(Some(&m(10e-9, "F"))), RangeVerdict::Matches);
            assert_eq!(f.verdict(Some(&m(100.0, "nF"))), RangeVerdict::Matches);
            assert_eq!(f.verdict(Some(&m(1e-6, "F"))), RangeVerdict::Matches);
            assert_eq!(f.verdict(Some(&m(1.0, "pF"))), RangeVerdict::Outside);
            assert_eq!(f.verdict(Some(&m(10.0, "µF"))), RangeVerdict::Outside);
        }
    }

    #[test]
    fn between_needs_a_high_bound_in_the_same_unit() {
        let lo = q("1", SiPrefix::Kilo, Unit::Ohm);
        assert!(RangeFilter::new(RangeOp::Between, lo.clone(), None).is_none());
        let hi = q("1", SiPrefix::Micro, Unit::Farad);
        assert!(RangeFilter::new(RangeOp::Between, lo, Some(hi)).is_none());
    }

    #[test]
    fn equals_matches_legacy_and_canonical_forms_within_tolerance() {
        let f =
            RangeFilter::new(RangeOp::Equals, q("4.7", SiPrefix::Kilo, Unit::Ohm), None).unwrap();
        assert_eq!(f.verdict(Some(&m(4700.0, "ohm"))), RangeVerdict::Matches);
        assert_eq!(f.verdict(Some(&m(4.7, "kΩ"))), RangeVerdict::Matches);
        // Inside 0.5 %, outside it, and the next E96 value.
        assert_eq!(f.verdict(Some(&m(4720.0, "ohm"))), RangeVerdict::Matches);
        assert_eq!(f.verdict(Some(&m(4750.0, "ohm"))), RangeVerdict::Outside);
        assert_eq!(f.verdict(Some(&m(4.87, "kΩ"))), RangeVerdict::Outside);
    }

    #[test]
    fn zero_equals_only_zero() {
        let f = RangeFilter::new(RangeOp::Equals, q("0", SiPrefix::One, Unit::Ohm), None).unwrap();
        assert_eq!(f.verdict(Some(&m(0.0, "ohm"))), RangeVerdict::Matches);
        assert_eq!(f.verdict(Some(&m(0.001, "ohm"))), RangeVerdict::Outside);
    }

    #[test]
    fn cells_without_a_comparable_value_are_excluded() {
        let f = at_least_1k();
        for cell in [
            Some(ParamValue::Text("10k".into())),
            Some(ParamValue::Number(5000.0)),
            Some(ParamValue::Bool(true)),
            Some(m(10.0, "nF")),
            None,
        ] {
            assert_eq!(f.verdict(cell.as_ref()), RangeVerdict::Excluded, "{cell:?}");
        }
    }

    #[test]
    fn the_column_unit_is_the_commonest_measurement_unit() {
        let cells = [
            m(1.0, "kΩ"),
            m(2.0, "ohm"),
            m(1.0, "nF"),
            ParamValue::Number(1.0),
        ];
        assert_eq!(column_unit(cells.iter()), Some(Unit::Ohm));
        assert_eq!(column_unit([ParamValue::Text("x".into())].iter()), None);
    }
}
