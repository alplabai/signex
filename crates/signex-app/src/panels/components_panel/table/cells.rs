//! A table cell, group line or Details value as text. A value that is not
//! set is `None`: the caller draws its "Not set" look. A min-to-max pair
//! with one end set shows that end as "≥ low" or "≤ high" (Caner,
//! 2026-11-27); it still sorts, groups and filters as not set.

use signex_i18n::{I18n, ids};
use signex_library::ComponentRow;
use signex_library::components_view::{CellText, CellValue, RowFacts, column_value};
use signex_library::templates::{ColumnKey, ParameterTemplate};
use signex_types::quantity::{ValueStyle, format_quantity};

/// `value` as text; `None` when it is not set.
pub fn shown(value: &CellValue, i18n: &I18n) -> Option<String> {
    match value.display(ValueStyle::Engineering) {
        CellText::Missing => None,
        CellText::Plain(text) => Some(text),
        CellText::Range { low, high } => Some(i18n.message_plain(
            ids::PANELS_COMPONENTS_RANGE,
            &[("low", low.as_str().into()), ("high", high.as_str().into())],
        )),
    }
}

/// The text `row` shows in `column`; `None` when not set.
pub fn cell_text(
    row: &ComponentRow,
    facts: &RowFacts,
    template: Option<&ParameterTemplate>,
    column: &ColumnKey,
    i18n: &I18n,
) -> Option<String> {
    let value = column_value(row, facts, template, column);
    if value.is_missing() {
        return column
            .param_name()
            .and_then(|name| half_set(row, template, name, i18n));
    }
    shown(&value, i18n)
}

/// A min-to-max pair named `name` with exactly one end set, as "≥ low" or
/// "≤ high"; `None` for anything else.
pub fn half_set(
    row: &ComponentRow,
    template: Option<&ParameterTemplate>,
    name: &str,
    i18n: &I18n,
) -> Option<String> {
    let slot = template?.range(name)?;
    let end = |param: &str| row.parameters.get(param).and_then(|v| v.as_quantity());
    let (id, q) = match (end(&slot.min), end(&slot.max)) {
        (Some(low), None) => (ids::PANELS_COMPONENTS_AT_LEAST, low),
        (None, Some(high)) => (ids::PANELS_COMPONENTS_AT_MOST, high),
        _ => return None,
    };
    let value = format_quantity(&q, ValueStyle::Engineering, false);
    Some(i18n.message_plain(id, &[("value", value.as_str().into())]))
}
