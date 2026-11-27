//! A min-to-max value: two measurements in one unit (components-cx
//! decision 8). Rows store the two ends as two ordinary measurement
//! parameters; this type only pairs them for display, sorting and
//! filtering.

use std::cmp::Ordering;

use serde::{Deserialize, Serialize};

use super::format::{ValueStyle, format_quantity, layout};
use super::sort::compare_for_sort;
use super::unit::Unit;
use super::value::Quantity;

/// Which end of a [`QuantityRange`] leads, for sorting or filtering.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RangeEnd {
    /// The low end (supply voltage sorts by its minimum).
    #[default]
    Min,
    /// The high end (temperature sorts by its maximum).
    Max,
}

/// An inclusive range `low..=high` of one unit, `low <= high`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct QuantityRange {
    low: Quantity,
    high: Quantity,
}

impl QuantityRange {
    /// `None` when the ends are in different units. A reversed pair is put
    /// in order, so a row that stores min and max swapped still reads.
    pub fn new(a: Quantity, b: Quantity) -> Option<QuantityRange> {
        if a.unit() != b.unit() {
            return None;
        }
        let (low, high) = if compare_for_sort(&a, &b) == Ordering::Greater {
            (b, a)
        } else {
            (a, b)
        };
        Some(QuantityRange { low, high })
    }

    pub fn low(&self) -> &Quantity {
        &self.low
    }

    pub fn high(&self) -> &Quantity {
        &self.high
    }

    pub fn end(&self, end: RangeEnd) -> &Quantity {
        match end {
            RangeEnd::Min => &self.low,
            RangeEnd::Max => &self.high,
        }
    }

    pub fn unit(&self) -> &Unit {
        self.low.unit()
    }

    /// Whether `q` (same unit) lies inside, ends included: the Covers test,
    /// "parts that run at 3.3 V".
    pub fn covers(&self, q: &Quantity) -> bool {
        q.unit() == self.unit()
            && compare_for_sort(q, &self.low) != Ordering::Less
            && compare_for_sort(q, &self.high) != Ordering::Greater
    }

    /// Whether the whole range lies inside `low..=high` (same unit).
    pub fn is_within(&self, low: &Quantity, high: &Quantity) -> bool {
        self.unit() == low.unit()
            && self.unit() == high.unit()
            && compare_for_sort(&self.low, low) != Ordering::Less
            && compare_for_sort(&self.high, high) != Ordering::Greater
    }

    /// The two ends as display text, for a cell that joins them with the
    /// catalog's "to" pattern. When both ends show in the same prefix the
    /// low end is the bare number (`("3", "3.6 V")`, `("−40", "125 °C")`);
    /// otherwise each carries its own (`("500 mV", "5.5 V")`).
    pub fn display_ends(&self, style: ValueStyle) -> (String, String) {
        let high = format_quantity(&self.high, style, false);
        let low = format_quantity(&self.low, style, false);
        let (lo, hi) = (
            layout(self.low.base(), self.unit(), 15),
            layout(self.high.base(), self.unit(), 15),
        );
        let shared = lo.prefix == hi.prefix && !lo.scientific && !hi.scientific;
        if !shared || style == ValueStyle::Rkm {
            return (low, high);
        }
        // Same prefix: drop the low end's suffix (" kΩ", " °C", " %").
        let suffix_len = high.len() - number_len(&high);
        let low_number = low[..low.len() - suffix_len].to_string();
        (low_number, high)
    }
}

/// Length of the leading number (sign included) of a formatted value.
fn number_len(text: &str) -> usize {
    text.find(' ').unwrap_or(text.len())
}

/// Order of two ranges for a sorted column: unit family, then the `first`
/// end, then the other end.
pub fn compare_ranges(a: &QuantityRange, b: &QuantityRange, first: RangeEnd) -> Ordering {
    let other = match first {
        RangeEnd::Min => RangeEnd::Max,
        RangeEnd::Max => RangeEnd::Min,
    };
    compare_for_sort(a.end(first), b.end(first))
        .then_with(|| compare_for_sort(a.end(other), b.end(other)))
}
