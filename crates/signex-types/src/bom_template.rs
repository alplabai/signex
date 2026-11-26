//! BOM templates (#1988): the columns a BOM lists, in order, computed
//! columns included, kept as a column preset (mockup row 113, "Save as
//! column preset…"). TOML with the token `snxbomt/1`. Signex writes a
//! template one way only ([`encode_bom_template`]), so a template it wrote,
//! read and written again is byte-identical. See `docs/BOM_COLUMNS.md`.

use serde::{Deserialize, Serialize};
use signex_expr::{FieldText, classify, parse};

use crate::format_version::{FormatVersion, FormatVersionError, MissingToken, NewerMinor};

/// The template's format token.
pub const SNXBOMT_FORMAT_V1: &str = "snxbomt/1";

/// `snxbomt` version policy: a newer token or none is refused, so a
/// template this build cannot represent is never rewritten.
pub const SNXBOMT_VERSION: FormatVersion = FormatVersion {
    current: SNXBOMT_FORMAT_V1,
    readable: &[SNXBOMT_FORMAT_V1],
    newer_minor: NewerMinor::Refused,
    missing: MissingToken::Refused,
};

/// One column of a template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateColumn {
    /// A standard column by its key (`designator`, `qty`, `unit_price`);
    /// the BOM exporter names the keys it knows.
    Standard(String),
    /// A part field by name (`MPN`).
    Field(String),
    /// A computed column: its header and its `=` expression.
    Computed { name: String, expression: String },
}

/// A BOM template: its columns, in the order they are listed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BomTemplate {
    pub columns: Vec<TemplateColumn>,
}

/// Why a template could not be read or written.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum BomTemplateError {
    #[error(transparent)]
    Version(#[from] FormatVersionError),
    #[error("the BOM template is not valid: {0}")]
    Parse(String),
    #[error("column {index} of the BOM template: {why}")]
    Column { index: usize, why: String },
}

/// Whether `name` and `expression` make a computed column: a name that is
/// not blank, and an expression that starts with `=` and parses. The one
/// check the template and the BOM engine share.
pub fn check_computed(name: &str, expression: &str) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("a computed column needs a name".into());
    }
    match classify(expression) {
        FieldText::Expression(body) => parse(body).map(|_| ()).map_err(|e| e.to_string()),
        FieldText::Literal(_) => Err(format!(
            "the expression of computed column {name} must start with `=`"
        )),
    }
}

/// A column as its file holds it: exactly one of `column`, `field`, or
/// `name` with `expression`.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ColumnRow {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    column: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    field: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    expression: Option<String>,
}

#[derive(Serialize)]
struct TemplateOut {
    format: &'static str,
    columns: Vec<ColumnRow>,
}

/// The token alone, read before the rest so a newer template is refused
/// for its token, not for a key this build does not know.
#[derive(Deserialize)]
struct TokenIn {
    format: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TemplateIn {
    #[serde(rename = "format")]
    _format: Option<String>,
    #[serde(default)]
    columns: Vec<ColumnRow>,
}

impl ColumnRow {
    fn of(column: &TemplateColumn) -> Self {
        match column {
            TemplateColumn::Standard(key) => Self {
                column: Some(key.clone()),
                ..Self::default()
            },
            TemplateColumn::Field(name) => Self {
                field: Some(name.clone()),
                ..Self::default()
            },
            TemplateColumn::Computed { name, expression } => Self {
                name: Some(name.clone()),
                expression: Some(expression.clone()),
                ..Self::default()
            },
        }
    }

    fn column(self) -> Result<TemplateColumn, String> {
        match (self.column, self.field, self.name, self.expression) {
            (Some(key), None, None, None) => Ok(TemplateColumn::Standard(key)),
            (None, Some(name), None, None) => Ok(TemplateColumn::Field(name)),
            (None, None, Some(name), Some(expression)) => {
                Ok(TemplateColumn::Computed { name, expression })
            }
            _ => Err("a column is one of `column`, `field`, or `name` with `expression`".into()),
        }
    }
}

/// What keeps `template` from being saved or used: a blank key or field
/// name, a computed column that [`check_computed`] refuses, or two computed
/// columns of one name. Columns count from 1.
fn check(template: &BomTemplate) -> Result<(), BomTemplateError> {
    let mut computed_names: Vec<&str> = Vec::new();
    for (index, column) in template.columns.iter().enumerate() {
        let at = |why: String| BomTemplateError::Column {
            index: index + 1,
            why,
        };
        match column {
            TemplateColumn::Standard(key) if key.trim().is_empty() => {
                return Err(at("a standard column needs its key".into()));
            }
            TemplateColumn::Field(name) if name.trim().is_empty() => {
                return Err(at("a field column needs the field's name".into()));
            }
            TemplateColumn::Computed { name, expression } => {
                check_computed(name, expression).map_err(at)?;
                if computed_names.contains(&name.as_str()) {
                    return Err(at(format!("a second computed column named {name}")));
                }
                computed_names.push(name);
            }
            TemplateColumn::Standard(_) | TemplateColumn::Field(_) => {}
        }
    }
    Ok(())
}

/// The template as its file holds it. Refused for a column [`check`]
/// refuses.
pub fn encode_bom_template(template: &BomTemplate) -> Result<String, BomTemplateError> {
    check(template)?;
    let file = TemplateOut {
        format: SNXBOMT_FORMAT_V1,
        columns: template.columns.iter().map(ColumnRow::of).collect(),
    };
    toml::to_string(&file).map_err(|e| BomTemplateError::Parse(e.to_string()))
}

/// Read a template's text. Refused for a missing or newer token, a key
/// this build does not know, and a column [`check`] refuses.
pub fn parse_bom_template(text: &str) -> Result<BomTemplate, BomTemplateError> {
    let token: TokenIn =
        toml::from_str(text).map_err(|e| BomTemplateError::Parse(e.to_string()))?;
    SNXBOMT_VERSION.read(token.format.as_deref())?;
    let file: TemplateIn =
        toml::from_str(text).map_err(|e| BomTemplateError::Parse(e.to_string()))?;
    let columns = file
        .columns
        .into_iter()
        .enumerate()
        .map(|(index, row)| {
            row.column().map_err(|why| BomTemplateError::Column {
                index: index + 1,
                why,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let template = BomTemplate { columns };
    check(&template)?;
    Ok(template)
}

#[cfg(test)]
#[path = "bom_template_tests.rs"]
mod tests;
