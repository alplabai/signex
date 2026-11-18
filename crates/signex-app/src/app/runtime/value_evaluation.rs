//! The Properties rows of a part whose value is an `=` expression (#1986):
//! what it evaluates to (`Value Shows`), or why it does not (`Value Error`).
//! The Value field itself keeps showing the source.

use std::collections::BTreeMap;

use signex_types::part_fields::part_field_value;
use signex_types::schematic::SchematicSheet;
use signex_types::variables::{Resolver, TextTable};

/// The rows for `symbol` on `sheet`, with the project's variables; none for
/// a literal value or a power port.
pub(super) fn value_evaluation_rows(
    sheet: &SchematicSheet,
    symbol: uuid::Uuid,
    project: &BTreeMap<String, String>,
) -> Vec<(String, String)> {
    let Some(symbol) = sheet
        .symbols
        .iter()
        .find(|s| s.uuid == symbol && !s.is_power)
    else {
        return Vec::new();
    };
    let (sheet_table, project_table) = (TextTable(&sheet.variables), TextTable(project));
    let variables: [&dyn Resolver; 2] = [&sheet_table, &project_table];
    match part_field_value(&sheet.symbols, symbol, "Value", &variables) {
        Some(Ok(value)) => vec![("Value Shows".to_string(), value)],
        Some(Err(error)) => vec![("Value Error".to_string(), error.to_string())],
        None => Vec::new(),
    }
}
