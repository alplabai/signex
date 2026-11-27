//! A class template's Components panel configuration: the columns, sort
//! and grouping a category opens with (components-cx decision 3), and the
//! min-to-max parameters it pairs from two stored measurements (decision
//! 8). The template sets the first view; a user's saved setup
//! ([`crate::components_view::ViewSetup`]) overrides it.
//!
//! In a template file:
//!
//! ```toml
//! [[ranges]]
//! name = "supply"
//! min = "supply_min"
//! max = "supply_max"
//!
//! [view]
//! columns = ["row:part_number", "value", "voltage_max", "row:stock"]
//! sort = [{ column = "voltage_max" }]
//! group = [{ column = "package" }, { column = "value" }]
//! pinned = ["value", "package"]
//! ```

use std::fmt;

use serde::{Deserialize, Serialize};
use signex_types::quantity::{QuantityRange, RangeEnd};

use super::ParameterTemplate;
use crate::param::ParamMap;

/// A column of the Components panel: a field every row has, or a
/// parameter (a template slot or a [`RangeSlot`]) by name.
///
/// Written as a string: a row field is `row:<field>` (`row:part_number`),
/// anything else is a parameter name. An unknown `row:` field (from a
/// newer build) is kept as a parameter of that name, so it survives a
/// load and save and simply has no values.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(from = "String", into = "String")]
pub enum ColumnKey {
    /// The library part number (`internal_pn`); always the first column.
    PartNumber,
    Description,
    Manufacturer,
    /// The bound symbol's name.
    LibraryRef,
    /// The bound footprint's name.
    Footprint,
    Stock,
    Price,
    /// A parameter, or a min-to-max pair, by name.
    Param(String),
}

impl ColumnKey {
    /// The row fields, in the order the column picker lists them.
    pub const ROW_FIELDS: [ColumnKey; 7] = [
        ColumnKey::PartNumber,
        ColumnKey::LibraryRef,
        ColumnKey::Description,
        ColumnKey::Manufacturer,
        ColumnKey::Footprint,
        ColumnKey::Stock,
        ColumnKey::Price,
    ];

    pub fn param(name: impl Into<String>) -> ColumnKey {
        ColumnKey::Param(name.into())
    }

    /// The spelling a template or a saved setup stores.
    pub fn as_str(&self) -> &str {
        match self {
            ColumnKey::PartNumber => "row:part_number",
            ColumnKey::Description => "row:description",
            ColumnKey::Manufacturer => "row:manufacturer",
            ColumnKey::LibraryRef => "row:library_ref",
            ColumnKey::Footprint => "row:footprint",
            ColumnKey::Stock => "row:stock",
            ColumnKey::Price => "row:price",
            ColumnKey::Param(name) => name,
        }
    }

    /// The parameter name, for a parameter column.
    pub fn param_name(&self) -> Option<&str> {
        match self {
            ColumnKey::Param(name) => Some(name),
            _ => None,
        }
    }
}

impl From<String> for ColumnKey {
    fn from(text: String) -> ColumnKey {
        ColumnKey::ROW_FIELDS
            .into_iter()
            .find(|k| k.as_str() == text)
            .unwrap_or(ColumnKey::Param(text))
    }
}

impl From<ColumnKey> for String {
    fn from(key: ColumnKey) -> String {
        key.as_str().to_string()
    }
}

impl fmt::Display for ColumnKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One level of a sort or a grouping: a column and its direction.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SortKey {
    pub column: ColumnKey,
    #[serde(default, skip_serializing_if = "super::is_false")]
    pub descending: bool,
}

impl SortKey {
    pub fn ascending(column: ColumnKey) -> SortKey {
        SortKey {
            column,
            descending: false,
        }
    }

    pub fn descending(column: ColumnKey) -> SortKey {
        SortKey {
            column,
            descending: true,
        }
    }
}

/// A min-to-max parameter shown as one cell (Supply voltage, Temperature):
/// rows store its two ends as the measurement parameters `min` and `max`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RangeSlot {
    /// The column name; must not be a slot name.
    pub name: String,
    /// The parameter holding the low end.
    pub min: String,
    /// The parameter holding the high end.
    pub max: String,
    /// The end a sorted column orders by first (Temperature by its max).
    #[serde(default, skip_serializing_if = "is_min")]
    pub sort_by: RangeEnd,
}

fn is_min(end: &RangeEnd) -> bool {
    *end == RangeEnd::Min
}

impl RangeSlot {
    /// The range a row's parameters hold. `None` unless both ends are
    /// measurements in one unit.
    pub fn read(&self, params: &ParamMap) -> Option<QuantityRange> {
        let low = params.get(&self.min)?.as_quantity()?;
        let high = params.get(&self.max)?.as_quantity()?;
        QuantityRange::new(low, high)
    }

    /// Whether `slot` is one of this pair's two ends.
    pub fn holds(&self, slot: &str) -> bool {
        self.min == slot || self.max == slot
    }
}

/// The view a category opens with. Empty lists fall back to the defaults
/// of [`ParameterTemplate::category_view`].
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CategoryView {
    /// Visible columns, in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub columns: Vec<ColumnKey>,
    /// Nested sort, outermost first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sort: Vec<SortKey>,
    /// Nested grouping, outermost first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub group: Vec<SortKey>,
    /// Parameters the Details card lists first (pin to top).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pinned: Vec<String>,
}

impl CategoryView {
    /// The All parts view: only row fields, sorted by part number.
    pub fn all_parts() -> CategoryView {
        CategoryView {
            columns: vec![
                ColumnKey::PartNumber,
                ColumnKey::Description,
                ColumnKey::Manufacturer,
                ColumnKey::Footprint,
                ColumnKey::Stock,
                ColumnKey::Price,
            ],
            sort: vec![SortKey::ascending(ColumnKey::PartNumber)],
            group: Vec::new(),
            pinned: Vec::new(),
        }
    }
}

impl ParameterTemplate {
    /// The [`RangeSlot`] named `name`.
    pub fn range(&self, name: &str) -> Option<&RangeSlot> {
        self.ranges.iter().find(|r| r.name == name)
    }

    /// Every parameter column, in slot order (required, then optional).
    /// A min-to-max pair takes the place of its first end and hides both
    /// ends; a pair whose ends are not slots comes last.
    pub fn parameter_columns(&self) -> Vec<ColumnKey> {
        let mut out: Vec<ColumnKey> = Vec::new();
        let slots = self.required_params.iter().chain(&self.optional_params);
        for slot in slots {
            let column = match self.ranges.iter().find(|r| r.holds(&slot.name)) {
                Some(range) => ColumnKey::param(&range.name),
                None => ColumnKey::param(&slot.name),
            };
            if !out.contains(&column) {
                out.push(column);
            }
        }
        for range in &self.ranges {
            let column = ColumnKey::param(&range.name);
            if !out.contains(&column) {
                out.push(column);
            }
        }
        out
    }

    /// Every column the category offers, for the column picker: part
    /// number, the parameter columns, then the other row fields.
    pub fn all_columns(&self) -> Vec<ColumnKey> {
        let mut out = vec![ColumnKey::PartNumber];
        out.extend(self.parameter_columns());
        out.extend(ColumnKey::ROW_FIELDS.into_iter().skip(1));
        out
    }

    /// The view this category opens with: the template's `[view]`, with
    /// what it leaves empty filled in. No columns: part number, every
    /// parameter column, stock and price. No sort: by part number.
    pub fn category_view(&self) -> CategoryView {
        let mut view = self.view.clone().unwrap_or_default();
        if view.columns.is_empty() {
            view.columns = vec![ColumnKey::PartNumber];
            view.columns.extend(self.parameter_columns());
            view.columns.extend([ColumnKey::Stock, ColumnKey::Price]);
        }
        if view.sort.is_empty() {
            view.sort = vec![SortKey::ascending(ColumnKey::PartNumber)];
        }
        view
    }
}
