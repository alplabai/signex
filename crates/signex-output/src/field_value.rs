//! A part's value as it is shown and exported (#1596, #1986): the one rule
//! the canvas, the PDF, SVG and print preview, the BOM and the netlist call.
//!
//! - An `=` value is evaluated (`signex_types::part_fields`): the part's
//!   own fields, other parts of the sheet, then the sheet's and project's
//!   variables. One that does not evaluate keeps its source text and
//!   carries the error, which the app reports in Messages.
//! - Any other value has its markup (`@{name}`, `${name}`, `${refdes:...}`)
//!   read from the part's fields, then the variables, and then its
//!   `${TOKEN}` special strings resolved.
//! - A power port's value is a net name: it reads the part's own fields
//!   only and is never evaluated nor resolved against the sheet.

use std::collections::HashMap;

use signex_types::markup::{ExpressionEvalContext, evaluate_expressions};
use signex_types::part_fields::part_field_value;
use signex_types::schematic::Symbol;
use signex_types::variables::{ExprError, FieldText, Layered, Resolver, TextTable, classify};

use crate::{ProjectMetadata, SubstitutionContext};

/// Where a value is read: its sheet's parts, the special strings and
/// variables, and the page's markup tables.
#[derive(Clone, Copy)]
pub struct ValueScope<'a> {
    /// The parts of the symbol's sheet.
    pub symbols: &'a [Symbol],
    /// The sheet's special strings and variables; `None` resolves none.
    pub special: Option<&'a SubstitutionContext<'a>>,
    /// Designators are shown with the product-aspect sign (#1416); an
    /// export copy that stores `-QAC1` needs no flag.
    pub signed: bool,
    /// Designator lookups for `${refdes:...}`; the part's own when `None`.
    pub refdes: Option<&'a dyn Resolver>,
    /// The page number `CELL()` reads.
    pub cell: Option<&'a str>,
}

impl<'a> ValueScope<'a> {
    /// The scope of one sheet with its special strings and variables.
    pub fn of_sheet(symbols: &'a [Symbol], special: Option<&'a SubstitutionContext<'a>>) -> Self {
        Self {
            symbols,
            special,
            signed: false,
            refdes: None,
            cell: None,
        }
    }
}

/// A value as shown, and why it shows its source when it does.
#[derive(Debug, Clone, PartialEq)]
pub struct ShownValue {
    pub text: String,
    pub error: Option<ExprError>,
}

/// The fields `@{name}` and `${name}` read on a part, before the variables.
pub(crate) fn symbol_eval_variables(sym: &Symbol) -> HashMap<String, String> {
    let mut vars = sym.fields.clone();
    for prop in &sym.custom_properties {
        if !prop.key.is_empty() {
            vars.insert(prop.key.clone(), prop.value.clone());
        }
    }
    vars.entry("refdes".to_string())
        .or_insert_with(|| sym.reference.clone());
    vars.entry("reference".to_string())
        .or_insert_with(|| sym.reference.clone());
    vars.entry("value".to_string())
        .or_insert_with(|| sym.value.clone());
    vars
}

/// What `symbol`'s value shows in `scope`.
pub fn shown_value(scope: &ValueScope<'_>, symbol: &Symbol) -> ShownValue {
    let tables = scope.special.map(SubstitutionContext::variable_tables);
    let variables: Vec<&dyn Resolver> = tables
        .iter()
        .flatten()
        .map(|table| table as &dyn Resolver)
        .collect();
    if !symbol.is_power && matches!(classify(&symbol.value), FieldText::Expression(_)) {
        let evaluated = part_field_value(scope.symbols, symbol, "Value", &variables, scope.signed);
        return match evaluated {
            Some(Ok(text)) => ShownValue { text, error: None },
            Some(Err(error)) => ShownValue {
                text: symbol.value.clone(),
                error: Some(error),
            },
            None => ShownValue {
                text: symbol.value.clone(),
                error: None,
            },
        };
    }
    let part_fields = symbol_eval_variables(symbol);
    let part_table = TextTable(&part_fields);
    let mut layers: Vec<&dyn Resolver> = vec![&part_table];
    if !symbol.is_power {
        layers.extend(variables);
    }
    let model = Layered::new(layers);
    let mut own = HashMap::new();
    if !symbol.uuid.is_nil() && !symbol.reference.is_empty() {
        own.insert(symbol.uuid.to_string(), symbol.reference.clone());
    }
    let own_table = TextTable(&own);
    let ctx = ExpressionEvalContext {
        current_refdes: (!symbol.reference.is_empty()).then_some(symbol.reference.as_str()),
        current_value: (!symbol.value.is_empty()).then_some(symbol.value.as_str()),
        cell: scope.cell,
        at_variables: Some(&model),
        refdes_variables: Some(scope.refdes.unwrap_or(&own_table as &dyn Resolver)),
        ..ExpressionEvalContext::default()
    };
    let marked = evaluate_expressions(&symbol.value, &ctx);
    let text = match scope.special {
        Some(special) if !symbol.is_power => crate::resolve(&marked, special),
        _ => marked,
    };
    ShownValue { text, error: None }
}

/// The special strings of a sheet for a list that has no page (the BOM,
/// the netlist): the project's and the sheet's values, no page number.
pub fn sheet_context(metadata: &ProjectMetadata) -> SubstitutionContext<'_> {
    SubstitutionContext {
        metadata,
        document_path: std::path::PathBuf::new(),
        filename: String::new(),
        sheet_name: String::new(),
        sheet_number: 0,
        sheet_count: 0,
        signex_version: env!("CARGO_PKG_VERSION"),
        variant: None,
        physical_structure: false,
        physical_sheet_number: false,
        physical_document_number: true,
    }
}

#[cfg(test)]
#[path = "field_value_tests.rs"]
mod tests;
