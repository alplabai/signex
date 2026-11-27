//! Computed BOM columns (#1988): a column whose cells are an `=` expression
//! over a line's fields and the project variables (`= Quantity * UnitPrice`,
//! `= VCC^2 / Value`), evaluated with the one expression engine
//! (`signex-expr`, through `signex_types::variables`).
//!
//! A name in the expression is looked up, first match wins:
//! 1. the line's own columns: `Quantity` (and `Qty`), `FittedQty`, and, on a
//!    priced line, `UnitPrice`, `BuyQty`, `ExtendedPrice` and `Stock`;
//! 2. the part's fields: its user fields, `Value`, `Footprint`, `Name`,
//!    `Description`, `LibRef` and `Designator`;
//! 3. the variables of the sheet the part is placed on;
//! 4. the project variables.
//!
//! A name matches exactly, then ignoring ASCII case. A grouped line is
//! evaluated once per part: it shows the result when every part gives the
//! same one, else [`ComputedCell::Differs`]. A part whose evaluation fails
//! makes the cell [`ComputedCell::Error`], which shows empty and is reported
//! once ([`computed_issues`]). See `docs/BOM_COLUMNS.md`.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};
use signex_types::variables::{Layered, Resolver, TextTable, display_text};

use crate::{BomComponent, BomRow, BomTable};

/// A computed column: its name (the header) and its `=` expression, checked
/// to parse when made.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ComputedColumn {
    name: String,
    expression: String,
}

/// Why a computed column could not be made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComputedColumnError(pub String);

impl fmt::Display for ComputedColumnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ComputedColumnError {}

impl ComputedColumn {
    /// A computed column named `name` that evaluates `expression`. Refused
    /// for an empty name, or an expression that does not start with `=` or
    /// does not parse (`signex_types::bom_template::check_computed`).
    pub fn new(
        name: impl Into<String>,
        expression: impl Into<String>,
    ) -> Result<Self, ComputedColumnError> {
        let (name, expression) = (name.into(), expression.into());
        signex_types::bom_template::check_computed(&name, &expression)
            .map_err(ComputedColumnError)?;
        Ok(Self { name, expression })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn expression(&self) -> &str {
        &self.expression
    }
}

/// One cell of a computed column.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ComputedCell {
    /// The result, with its derived unit (`2.5mW`).
    Value { text: String },
    /// The parts of a grouped line give different results.
    Differs,
    /// The expression failed: the cell shows empty. `member` names the part
    /// it failed for on a grouped line of several parts.
    Error {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        member: Option<String>,
        error: String,
    },
}

/// The variables a part's expressions read besides its fields: those of the
/// sheet it is placed on, then the project's.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MemberVariables {
    pub sheet: BTreeMap<String, String>,
    pub project: BTreeMap<String, String>,
}

/// Evaluate `columns` on every row of `table` into `BomRow::computed`.
/// `members[i]` lists the parts of row `i` as indices into `components`
/// (`crate::build_table_with_members`), and `variables[j]` holds the
/// variables of `components[j]` (none when it is missing). Run after the
/// prices are applied, so `UnitPrice` reads the cached price.
pub fn apply_computed_columns(
    table: &mut BomTable,
    members: &[Vec<usize>],
    components: &[BomComponent],
    variables: &[MemberVariables],
    columns: &[ComputedColumn],
) {
    let none = MemberVariables::default();
    for (row, parts) in table.rows.iter_mut().zip(members) {
        for column in columns {
            let results = parts.iter().filter_map(|&index| {
                let component = components.get(index)?;
                let vars = variables.get(index).unwrap_or(&none);
                Some((
                    component.reference.as_str(),
                    evaluate(row, component, vars, column.expression()),
                ))
            });
            if let Some(cell) = group_cell(results, parts.len() > 1) {
                row.computed.insert(column.name.clone(), cell);
            }
        }
    }
}

/// What a line shows from its parts' results: the first failure, else the
/// one result they share, else [`ComputedCell::Differs`]. `None` for a line
/// with no part.
fn group_cell<'a>(
    results: impl Iterator<Item = (&'a str, Result<String, String>)>,
    several: bool,
) -> Option<ComputedCell> {
    let mut shared: Option<String> = None;
    let mut differs = false;
    for (reference, result) in results {
        match result {
            Err(error) => {
                return Some(ComputedCell::Error {
                    member: several.then(|| reference.to_string()),
                    error,
                });
            }
            Ok(text) => match &shared {
                None => shared = Some(text),
                Some(first) => differs |= *first != text,
            },
        }
    }
    let text = shared?;
    Some(if differs {
        ComputedCell::Differs
    } else {
        ComputedCell::Value { text }
    })
}

/// `expression` evaluated for one part of `row`, shown with its unit.
fn evaluate(
    row: &BomRow,
    component: &BomComponent,
    variables: &MemberVariables,
    expression: &str,
) -> Result<String, String> {
    let line = line_names(row);
    let fields = part_fields(component);
    let (line, fields) = (TextTable(&line), TextTable(&fields));
    let (sheet, project) = (TextTable(&variables.sheet), TextTable(&variables.project));
    let scope = Layered::new(vec![&line as &dyn Resolver, &fields, &sheet, &project]);
    display_text(expression, &scope).map_err(|error| error.to_string())
}

/// The line's own columns an expression can read.
fn line_names(row: &BomRow) -> BTreeMap<String, String> {
    let mut names = BTreeMap::from([
        ("Quantity".to_string(), row.qty.to_string()),
        ("Qty".to_string(), row.qty.to_string()),
        ("FittedQty".to_string(), row.fitted_qty.to_string()),
    ]);
    if let Some(price) = &row.price {
        names.insert("UnitPrice".into(), price.unit_price.to_string());
        names.insert("BuyQty".into(), price.buy_qty.to_string());
        names.insert("ExtendedPrice".into(), price.extended_price.to_string());
        if let Some(stock) = price.stock {
            names.insert("Stock".into(), stock.to_string());
        }
    }
    names
}

/// The part's fields an expression can read: its user fields, then the
/// standard ones, which win over a user field of the same name.
fn part_fields(component: &BomComponent) -> BTreeMap<String, String> {
    let mut fields = component.custom.clone();
    for (key, value) in [
        ("Value", &component.value),
        ("Footprint", &component.footprint),
        ("Name", &component.name),
        ("Description", &component.description),
        ("LibRef", &component.lib_ref),
        ("Designator", &component.reference),
    ] {
        fields.insert(key.to_string(), value.clone());
    }
    fields
}

/// A computed cell that failed: the line, its designators, the column, the
/// part it failed for (on a grouped line) and the error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ComputedIssue {
    /// 1-based line in table (export) order.
    pub line: usize,
    pub references: Vec<String>,
    pub column: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub member: Option<String>,
    pub error: String,
}

impl fmt::Display for ComputedIssue {
    /// The Messages line: one per failed cell, naming the line by its
    /// designators.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "BOM line {} ({}): column {}",
            self.line,
            self.references.join(", "),
            self.column
        )?;
        if let Some(member) = &self.member {
            write!(f, " for {member}")?;
        }
        write!(f, ": {}", self.error)
    }
}

/// Every failed cell of `columns` in `table`, by line then column order.
pub fn computed_issues(table: &BomTable, columns: &[ComputedColumn]) -> Vec<ComputedIssue> {
    let mut issues = Vec::new();
    for (index, row) in table.rows.iter().enumerate() {
        for column in columns {
            if let Some(ComputedCell::Error { member, error }) = row.computed.get(column.name()) {
                issues.push(ComputedIssue {
                    line: index + 1,
                    references: row.references.clone(),
                    column: column.name.clone(),
                    member: member.clone(),
                    error: error.clone(),
                });
            }
        }
    }
    issues
}

#[cfg(test)]
#[path = "computed_tests.rs"]
mod tests;
