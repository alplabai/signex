//! The parameter list of the Components panel's Details card: every
//! parameter with its value, pinned ones first.

use super::cell::{CellValue, RowFacts, column_value};
use super::setup::offered_columns;
use crate::component::ComponentRow;
use crate::templates::{ColumnKey, ParameterTemplate};

/// One line of the Details parameter list.
#[derive(Clone, Debug, PartialEq)]
pub struct DetailEntry {
    /// The parameter (or min-to-max pair) name.
    pub name: String,
    /// `Missing` for a template slot the row leaves empty.
    pub value: CellValue,
    pub pinned: bool,
}

/// `row`'s parameters for Details: the `pinned` ones first, in pin order,
/// then the template's parameter columns (a min-to-max pair as one line,
/// empty slots included), then the other parameters the row holds, by
/// name. A pin the row's category does not offer is skipped.
pub fn detail_parameters(
    row: &ComponentRow,
    template: Option<&ParameterTemplate>,
    pinned: &[String],
) -> Vec<DetailEntry> {
    let facts = RowFacts::default();
    // Without a template every parameter the row holds is a free one.
    let empty = ParameterTemplate::default();
    let names: Vec<String> = offered_columns(Some(template.unwrap_or(&empty)), [row])
        .into_iter()
        .filter_map(|c| c.param_name().map(str::to_string))
        .collect();
    let entry = |name: &str, pinned: bool| DetailEntry {
        name: name.to_string(),
        value: column_value(row, &facts, template, &ColumnKey::param(name)),
        pinned,
    };
    let mut out: Vec<DetailEntry> = pinned
        .iter()
        .filter(|p| names.contains(*p))
        .map(|p| entry(p, true))
        .collect();
    out.extend(
        names
            .iter()
            .filter(|n| !pinned.contains(*n))
            .map(|n| entry(n, false)),
    );
    out
}
