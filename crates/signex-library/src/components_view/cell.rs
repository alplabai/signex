//! What a row shows in one Components panel column.

use signex_types::quantity::{Quantity, QuantityRange, RangeEnd, ValueStyle, format_quantity};

use crate::component::ComponentRow;
use crate::param::ParamValue;
use crate::range_filter::{RangeFilter, RangeOn, RangeVerdict};
use crate::templates::{ColumnKey, ParameterTemplate};

/// One cell. A measurement keeps its base value, so sort, group, range
/// filter and search read the same number the cell shows (mockup row 52).
#[derive(Clone, Debug, PartialEq)]
pub enum CellValue {
    /// No value: an absent parameter, empty text, a non-finite number, or a
    /// min-to-max pair missing an end. Shown as "Not set".
    Missing,
    Text(String),
    Number(f64),
    Bool(bool),
    Quantity {
        value: Quantity,
        /// Shown with a leading `±` (the slot's `display_plus_minus`).
        plus_minus: bool,
    },
    Range {
        range: QuantityRange,
        /// The end a sorted column orders by first.
        sort_by: RangeEnd,
    },
}

/// A cell as display text. The panel renders `Missing` as its "Not set"
/// string and joins a `Range` with its "to" pattern, both from the
/// catalog; `Plain` is shown as is. Stock and price arrive as `Plain`
/// numbers and are usually re-formatted by the panel (grouping, currency).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CellText {
    Missing,
    Plain(String),
    Range { low: String, high: String },
}

/// What a row's own fields do not hold, resolved by the panel: the bound
/// symbol and footprint names and the cached supply figures.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RowFacts {
    pub library_ref: Option<String>,
    pub footprint: Option<String>,
    pub stock: Option<u64>,
    pub price: Option<f64>,
}

impl CellValue {
    pub fn is_missing(&self) -> bool {
        matches!(self, CellValue::Missing)
    }

    fn text(s: &str) -> CellValue {
        if s.trim().is_empty() {
            CellValue::Missing
        } else {
            CellValue::Text(s.to_string())
        }
    }

    fn number(n: f64) -> CellValue {
        if n.is_finite() {
            CellValue::Number(n)
        } else {
            CellValue::Missing
        }
    }

    /// The cell as display text in `style`.
    pub fn display(&self, style: ValueStyle) -> CellText {
        match self {
            CellValue::Missing => CellText::Missing,
            CellValue::Text(s) => CellText::Plain(s.clone()),
            CellValue::Number(n) => CellText::Plain(n.to_string()),
            CellValue::Bool(b) => CellText::Plain(b.to_string()),
            CellValue::Quantity { value, plus_minus } => {
                CellText::Plain(format_quantity(value, style, *plus_minus))
            }
            CellValue::Range { range, .. } => {
                let (low, high) = range.display_ends(style);
                CellText::Range { low, high }
            }
        }
    }

    /// Every measurement the cell holds (both ends of a range), for
    /// value-aware search (mockup row 52).
    pub fn quantities(&self) -> Vec<&Quantity> {
        match self {
            CellValue::Quantity { value, .. } => vec![value],
            CellValue::Range { range, .. } => vec![range.low(), range.high()],
            _ => Vec::new(),
        }
    }

    /// What `filter` makes of this cell; `on` matters for a min-to-max
    /// value only. A cell that is not a measurement is excluded.
    pub fn range_verdict(&self, filter: &RangeFilter, on: RangeOn) -> RangeVerdict {
        match self {
            CellValue::Quantity { value, .. } => filter.verdict_quantity(value),
            CellValue::Range { range, .. } => filter.verdict_range(range, on),
            _ => RangeVerdict::Excluded,
        }
    }
}

/// The value `row` shows in `column`. A parameter column reads the
/// template's min-to-max pair of that name first, then the parameter.
pub fn column_value(
    row: &ComponentRow,
    facts: &RowFacts,
    template: Option<&ParameterTemplate>,
    column: &ColumnKey,
) -> CellValue {
    match column {
        ColumnKey::PartNumber => CellValue::text(row.internal_pn.as_str()),
        ColumnKey::Description => CellValue::text(&row.description),
        ColumnKey::Manufacturer => CellValue::text(&row.primary_mpn.manufacturer),
        ColumnKey::LibraryRef => facts
            .library_ref
            .as_deref()
            .map_or(CellValue::Missing, CellValue::text),
        ColumnKey::Footprint => facts
            .footprint
            .as_deref()
            .map_or(CellValue::Missing, CellValue::text),
        ColumnKey::Stock => facts
            .stock
            .map_or(CellValue::Missing, |n| CellValue::Number(n as f64)),
        ColumnKey::Price => facts.price.map_or(CellValue::Missing, CellValue::number),
        ColumnKey::Param(name) => param_value(row, template, name),
    }
}

fn param_value(row: &ComponentRow, template: Option<&ParameterTemplate>, name: &str) -> CellValue {
    if let Some(slot) = template.and_then(|t| t.range(name)) {
        return match slot.read(&row.parameters) {
            Some(range) => CellValue::Range {
                range,
                sort_by: slot.sort_by,
            },
            None => CellValue::Missing,
        };
    }
    let Some(value) = row.parameters.get(name) else {
        return CellValue::Missing;
    };
    match value {
        ParamValue::Text(s) => CellValue::text(s),
        ParamValue::Number(n) => CellValue::number(*n),
        ParamValue::Bool(b) => CellValue::Bool(*b),
        ParamValue::Measurement { .. } => match value.as_quantity() {
            Some(q) => CellValue::Quantity {
                value: q,
                plus_minus: template
                    .and_then(|t| t.slot(name))
                    .is_some_and(|s| s.display_plus_minus),
            },
            None => CellValue::Missing,
        },
    }
}

/// Identity of a group (one nested group level): cells that show the same
/// value share it. Text groups case-insensitively; a measurement by its
/// base value and unit, so `{4.7, "kΩ"}` and `{4700, "ohm"}` are one group.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum GroupKey {
    Missing,
    Text(String),
    Number(u64),
    Bool(bool),
    Quantity { base: u64, unit: String },
    Range { low: u64, high: u64, unit: String },
}

impl CellValue {
    pub fn group_key(&self) -> GroupKey {
        match self {
            CellValue::Missing => GroupKey::Missing,
            CellValue::Text(s) => GroupKey::Text(s.to_lowercase()),
            CellValue::Number(n) => GroupKey::Number(if *n == 0.0 { 0 } else { n.to_bits() }),
            CellValue::Bool(b) => GroupKey::Bool(*b),
            CellValue::Quantity { value, .. } => GroupKey::Quantity {
                base: value.to_bits(),
                unit: value.unit().token().to_string(),
            },
            CellValue::Range { range, .. } => GroupKey::Range {
                low: range.low().to_bits(),
                high: range.high().to_bits(),
                unit: range.unit().token().to_string(),
            },
        }
    }
}
