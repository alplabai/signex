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
//! Bounds are inclusive. `=` uses [`EQUALS_RELATIVE_TOLERANCE`]. A bound is a
//! base value or, for "Any prefix", the number a value displays
//! ([`FilterBound`]); a min-to-max value is judged on its whole range, its
//! low or high end, or whether it covers the bound ([`RangeOn`]).

use std::cmp::Ordering;

use signex_types::quantity::{Quantity, QuantityRange, Unit, compare_for_sort};

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

/// One end of a filter.
#[derive(Clone, Debug, PartialEq)]
pub enum FilterBound {
    /// A picked prefix: a true base-value comparison.
    Base(Quantity),
    /// "Any prefix": the number a value displays, in whatever prefix it
    /// displays in, so `100` takes 100 pF, 100 nF and 100 µF
    /// (mockup row 52).
    Displayed(f64),
}

/// Which part of a min-to-max value a filter tests (mockup row 52).
/// A plain measurement is always judged on its value.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum RangeOn {
    /// The whole range: `between` needs both ends inside, `≥` tests the
    /// low end and `≤` the high end.
    #[default]
    Value,
    /// The range contains the low bound ("runs at 3.3 V"); the operator is
    /// not used.
    Covers,
    /// The operator over a min-to-max value's low end.
    Min,
    /// The operator over a min-to-max value's high end.
    Max,
}

/// A validated range over one unit. Build with [`RangeFilter::new`] or
/// [`RangeFilter::with_bounds`].
#[derive(Clone, Debug, PartialEq)]
pub struct RangeFilter {
    op: RangeOp,
    unit: Unit,
    low: FilterBound,
    /// Equal to `low` unless the operator is [`RangeOp::Between`].
    high: FilterBound,
}

impl RangeFilter {
    /// `high` is required for [`RangeOp::Between`] and ignored otherwise.
    /// `None` if a bound is missing or the two bounds are in different
    /// units. A reversed pair is put in order.
    pub fn new(op: RangeOp, low: Quantity, high: Option<Quantity>) -> Option<RangeFilter> {
        let unit = low.unit().clone();
        RangeFilter::with_bounds(
            op,
            unit,
            FilterBound::Base(low),
            high.map(FilterBound::Base),
        )
    }

    /// [`RangeFilter::new`] with either kind of bound, in `unit`. `None` if
    /// a needed bound is missing, a base bound is in another unit, or a
    /// displayed number is not finite. Two bounds of the same kind in
    /// reverse order are put in order; a base and a displayed bound cannot
    /// be ordered and are kept as given.
    pub fn with_bounds(
        op: RangeOp,
        unit: Unit,
        low: FilterBound,
        high: Option<FilterBound>,
    ) -> Option<RangeFilter> {
        let fits = |b: &FilterBound| match b {
            FilterBound::Base(q) => q.unit() == &unit,
            FilterBound::Displayed(n) => n.is_finite(),
        };
        if !fits(&low) {
            return None;
        }
        if !op.is_between() {
            return Some(RangeFilter {
                op,
                unit,
                high: low.clone(),
                low,
            });
        }
        let high = high.filter(fits)?;
        let reversed = match (&low, &high) {
            (FilterBound::Base(a), FilterBound::Base(b)) => {
                compare_for_sort(a, b) == Ordering::Greater
            }
            (FilterBound::Displayed(a), FilterBound::Displayed(b)) => a > b,
            _ => false,
        };
        let (low, high) = if reversed { (high, low) } else { (low, high) };
        Some(RangeFilter {
            op,
            unit,
            low,
            high,
        })
    }

    pub fn op(&self) -> RangeOp {
        self.op
    }

    /// The unit every compared value must be in.
    pub fn unit(&self) -> &Unit {
        &self.unit
    }

    pub fn low(&self) -> &FilterBound {
        &self.low
    }

    /// The upper bound; the same as [`RangeFilter::low`] unless the
    /// operator is between.
    pub fn high(&self) -> &FilterBound {
        &self.high
    }

    /// Judge `value`, a row's cell (`None` when the row has no such key).
    pub fn verdict(&self, value: Option<&ParamValue>) -> RangeVerdict {
        match value.and_then(ParamValue::as_quantity) {
            Some(q) => self.verdict_quantity(&q),
            None => RangeVerdict::Excluded,
        }
    }

    /// Judge one measurement.
    pub fn verdict_quantity(&self, q: &Quantity) -> RangeVerdict {
        if q.unit() != self.unit() {
            return RangeVerdict::Excluded;
        }
        verdict_of(self.accepts(q))
    }

    /// Judge a min-to-max value, testing what `on` names.
    pub fn verdict_range(&self, range: &QuantityRange, on: RangeOn) -> RangeVerdict {
        if range.unit() != self.unit() {
            return RangeVerdict::Excluded;
        }
        verdict_of(match on {
            RangeOn::Min => self.accepts(range.low()),
            RangeOn::Max => self.accepts(range.high()),
            RangeOn::Covers => covers(range, &self.low),
            RangeOn::Value => self.accepts_whole(range),
        })
    }

    fn accepts(&self, q: &Quantity) -> bool {
        let at_or_above_low = order(q, &self.low) != Ordering::Less;
        let at_or_below_high = order(q, &self.high) != Ordering::Greater;
        match self.op {
            RangeOp::AtLeast => at_or_above_low,
            RangeOp::AtMost => at_or_below_high,
            RangeOp::Between => at_or_above_low && at_or_below_high,
            RangeOp::Equals => equals(q, &self.low),
        }
    }

    fn accepts_whole(&self, range: &QuantityRange) -> bool {
        let low_in = order(range.low(), &self.low) != Ordering::Less;
        let high_in = order(range.high(), &self.high) != Ordering::Greater;
        match self.op {
            RangeOp::AtLeast => low_in,
            RangeOp::AtMost => high_in,
            RangeOp::Between => low_in && high_in,
            RangeOp::Equals => equals(range.low(), &self.low) && equals(range.high(), &self.low),
        }
    }
}

fn verdict_of(accepted: bool) -> RangeVerdict {
    if accepted {
        RangeVerdict::Matches
    } else {
        RangeVerdict::Outside
    }
}

/// Where `q` stands against `bound`.
fn order(q: &Quantity, bound: &FilterBound) -> Ordering {
    match bound {
        FilterBound::Base(b) => compare_for_sort(q, b),
        FilterBound::Displayed(n) => q.displayed_number().total_cmp(n),
    }
}

fn equals(q: &Quantity, bound: &FilterBound) -> bool {
    match bound {
        FilterBound::Base(b) => approx_equal(q.base(), b.base()),
        FilterBound::Displayed(n) => approx_equal(q.displayed_number(), *n),
    }
}

/// Covers with a displayed number reads both ends in the high end's
/// display prefix, as the cell shows them (`3 to 3.6 V` covers `3.3`).
fn covers(range: &QuantityRange, bound: &FilterBound) -> bool {
    match bound {
        FilterBound::Base(b) => range.covers(b),
        FilterBound::Displayed(n) => {
            let prefix = range.high().eng_parts().1;
            range.low().number_in(prefix) <= *n && *n <= range.high().number_in(prefix)
        }
    }
}

/// `=` within [`EQUALS_RELATIVE_TOLERANCE`]; zero equals only zero. The one
/// tolerance rule for range filters and value-aware search.
pub fn approx_equal(a: f64, b: f64) -> bool {
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

    fn base(b: f64, unit: Unit) -> Quantity {
        Quantity::new(b, unit).unwrap()
    }

    fn span(lo: f64, hi: f64, unit: Unit) -> QuantityRange {
        QuantityRange::new(base(lo, unit.clone()), base(hi, unit)).unwrap()
    }

    fn any(op: RangeOp, unit: Unit, lo: f64, hi: Option<f64>) -> RangeFilter {
        RangeFilter::with_bounds(
            op,
            unit,
            FilterBound::Displayed(lo),
            hi.map(FilterBound::Displayed),
        )
        .unwrap()
    }

    /// mockup row 52: on Any prefix, 100 finds 100 pF, 100 nF
    /// and 100 µF.
    #[test]
    fn any_prefix_compares_the_displayed_number() {
        let f = any(RangeOp::Equals, Unit::Farad, 100.0, None);
        for kept in [m(100e-12, "F"), m(100.0, "nF"), m(100e-6, "F")] {
            assert_eq!(f.verdict(Some(&kept)), RangeVerdict::Matches, "{kept:?}");
        }
        assert_eq!(f.verdict(Some(&m(10e-9, "F"))), RangeVerdict::Outside);
        let at_least = any(RangeOp::AtLeast, Unit::Farad, 100.0, None);
        assert_eq!(
            at_least.verdict(Some(&m(470e-9, "F"))),
            RangeVerdict::Matches
        );
        assert_eq!(at_least.verdict(Some(&m(1e-6, "F"))), RangeVerdict::Outside);
        let between = any(RangeOp::Between, Unit::Farad, 470.0, Some(100.0));
        assert_eq!(between.low(), &FilterBound::Displayed(100.0));
        assert_eq!(
            between.verdict(Some(&m(220e-12, "F"))),
            RangeVerdict::Matches
        );
        assert_eq!(between.verdict(Some(&m(1e-6, "F"))), RangeVerdict::Outside);
    }

    #[test]
    fn with_bounds_refuses_a_bound_that_cannot_compare() {
        let ohm = base(1.0, Unit::Ohm);
        assert!(
            RangeFilter::with_bounds(RangeOp::AtLeast, Unit::Farad, FilterBound::Base(ohm), None)
                .is_none()
        );
        let nan = FilterBound::Displayed(f64::NAN);
        assert!(RangeFilter::with_bounds(RangeOp::AtLeast, Unit::Farad, nan, None).is_none());
        let one = FilterBound::Displayed(1.0);
        assert!(RangeFilter::with_bounds(RangeOp::Between, Unit::Farad, one, None).is_none());
        // A base and a displayed bound are kept as given.
        let mixed = RangeFilter::with_bounds(
            RangeOp::Between,
            Unit::Volt,
            FilterBound::Displayed(5.0),
            Some(FilterBound::Base(base(1.0, Unit::Volt))),
        )
        .unwrap();
        assert_eq!(mixed.low(), &FilterBound::Displayed(5.0));
    }

    /// mockup row 52: Supply voltage filters on Covers, for
    /// example parts that run at 3.3 V.
    #[test]
    fn covers_keeps_ranges_that_contain_the_bound() {
        let f = RangeFilter::new(RangeOp::Equals, base(3.3, Unit::Volt), None).unwrap();
        let on = RangeOn::Covers;
        let v = Unit::Volt;
        assert_eq!(
            f.verdict_range(&span(3.0, 3.6, v.clone()), on),
            RangeVerdict::Matches
        );
        assert_eq!(
            f.verdict_range(&span(2.3, 5.5, v.clone()), on),
            RangeVerdict::Matches
        );
        assert_eq!(
            f.verdict_range(&span(4.5, 5.5, v.clone()), on),
            RangeVerdict::Outside
        );
        assert_eq!(
            f.verdict_range(&span(-40.0, 85.0, Unit::DegreeCelsius), on),
            RangeVerdict::Excluded
        );
        // Any prefix reads both ends in the high end's prefix.
        let shown = any(RangeOp::Equals, v.clone(), 3.3, None);
        assert_eq!(
            shown.verdict_range(&span(0.5, 5.5, v.clone()), on),
            RangeVerdict::Matches
        );
        let mv = any(RangeOp::Equals, v.clone(), 600.0, None);
        assert_eq!(
            mv.verdict_range(&span(0.5, 5.5, v), on),
            RangeVerdict::Outside
        );
    }

    #[test]
    fn min_and_max_test_one_end() {
        let c = Unit::DegreeCelsius;
        let f = RangeFilter::new(RangeOp::AtLeast, base(105.0, c.clone()), None).unwrap();
        let industrial = span(-40.0, 85.0, c.clone());
        let automotive = span(-40.0, 125.0, c.clone());
        assert_eq!(
            f.verdict_range(&industrial, RangeOn::Max),
            RangeVerdict::Outside
        );
        assert_eq!(
            f.verdict_range(&automotive, RangeOn::Max),
            RangeVerdict::Matches
        );
        assert_eq!(
            f.verdict_range(&automotive, RangeOn::Min),
            RangeVerdict::Outside
        );
        let cold = RangeFilter::new(RangeOp::AtMost, base(-40.0, c), None).unwrap();
        assert_eq!(
            cold.verdict_range(&industrial, RangeOn::Min),
            RangeVerdict::Matches
        );
    }

    #[test]
    fn value_on_a_range_needs_the_whole_range_inside() {
        let c = Unit::DegreeCelsius;
        let f = RangeFilter::new(
            RangeOp::Between,
            base(-40.0, c.clone()),
            Some(base(125.0, c.clone())),
        )
        .unwrap();
        let on = RangeOn::Value;
        assert_eq!(
            f.verdict_range(&span(-40.0, 85.0, c.clone()), on),
            RangeVerdict::Matches
        );
        assert_eq!(
            f.verdict_range(&span(-55.0, 125.0, c.clone()), on),
            RangeVerdict::Outside
        );
        let at_least = RangeFilter::new(RangeOp::AtLeast, base(-40.0, c.clone()), None).unwrap();
        assert_eq!(
            at_least.verdict_range(&span(-40.0, 85.0, c.clone()), on),
            RangeVerdict::Matches
        );
        assert_eq!(
            at_least.verdict_range(&span(-55.0, 85.0, c), on),
            RangeVerdict::Outside
        );
    }

    #[test]
    fn approx_equal_is_the_half_percent_rule() {
        assert!(approx_equal(4700.0, 4720.0));
        assert!(!approx_equal(4700.0, 4750.0));
        assert!(approx_equal(0.0, 0.0));
        assert!(!approx_equal(0.0, 1e-300));
    }
}
