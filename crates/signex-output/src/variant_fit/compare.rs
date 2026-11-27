//! Compare variants (#2949, mockup row 59, view d): two variants side by
//! side, only the parts they differ on. A part differs when one variant fits
//! it and the other does not, or when both fit it with another value,
//! footprint or field. The sides are read from the parts matrix with the
//! project's variant settings laid over it, as the Variant Manager and the
//! BOM read them, so the comparison never disagrees with either.

use std::collections::BTreeMap;

use serde::Serialize;
use signex_types::project::{
    FOOTPRINT_PARAMETER, PlacedPart, VALUE_PARAMETER, VariantSettings, is_base_variant,
};

use super::matrix::{VariantMatrix, VariantRow};

/// One part in one variant, as the comparison shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PartInVariant {
    pub fitted: bool,
    /// The value, footprint and fields, by parameter name ([`VALUE_PARAMETER`],
    /// [`FOOTPRINT_PARAMETER`] or a field name), the variant's own over the
    /// sheets'. Empty values are left out.
    pub parameters: BTreeMap<String, String>,
}

/// What differs between the two sides of one part.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "parameter", rename_all = "snake_case")]
pub enum PartChange {
    /// One side fits the part and the other does not.
    Fit,
    /// Both fit it, with another value for this parameter.
    Parameter(String),
}

/// One part the two variants differ on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ComparedPart {
    pub designator: String,
    pub from: PartInVariant,
    pub to: PartInVariant,
    pub changes: Vec<PartChange>,
}

/// Two variants side by side: the parts they differ on, in designator order,
/// and how many parts each fits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VariantComparison {
    /// The left variant's name as read; Base's for Base.
    pub from: String,
    /// The right variant's name as read.
    pub to: String,
    pub parts: Vec<ComparedPart>,
    pub from_fitted: usize,
    pub to_fitted: usize,
}

/// Why two variants cannot be compared.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompareProblem {
    /// The matrix was not read for this variant.
    Unknown(String),
}

impl std::fmt::Display for CompareProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unknown(name) => write!(f, "the project has no variant named `{name}`"),
        }
    }
}

impl VariantMatrix {
    /// `from` and `to` (`None` or Base's name is Base) side by side, read from
    /// this matrix, as read from the sheets, with `settings` laid over it.
    pub fn compare(
        &self,
        settings: &VariantSettings,
        from: Option<&str>,
        to: Option<&str>,
    ) -> Result<VariantComparison, CompareProblem> {
        let column = |variant: Option<&str>| {
            self.column(variant)
                .ok_or_else(|| CompareProblem::Unknown(variant.unwrap_or_default().to_string()))
        };
        let (from_column, to_column) = (column(from)?, column(to)?);
        let shown = self.with_settings(settings);
        let name = |column: usize| match column {
            0 => signex_types::project::BASE_VARIANT.to_string(),
            n => self.variants[n - 1].clone(),
        };
        let (from_name, to_name) = (name(from_column), name(to_column));
        let side = |row: &VariantRow, column: usize, variant: &str| PartInVariant {
            fitted: row.fitted.get(column).copied().unwrap_or(true),
            parameters: parameters_in(row, settings, variant),
        };
        let parts = shown
            .rows
            .iter()
            .filter_map(|row| {
                let from = side(row, from_column, &from_name);
                let to = side(row, to_column, &to_name);
                let changes = changes_between(&from, &to);
                (!changes.is_empty()).then(|| ComparedPart {
                    designator: row.designator.clone(),
                    from,
                    to,
                    changes,
                })
            })
            .collect();
        Ok(VariantComparison {
            from_fitted: shown.fitted_count(from_column),
            to_fitted: shown.fitted_count(to_column),
            from: from_name,
            to: to_name,
            parts,
        })
    }
}

/// The value, footprint and fields of `row`'s part in `variant`: the sheets'
/// with the variant's own values in their place.
fn parameters_in(
    row: &VariantRow,
    settings: &VariantSettings,
    variant: &str,
) -> BTreeMap<String, String> {
    let mut out: BTreeMap<String, String> = row.parameters.iter().cloned().collect();
    out.insert(VALUE_PARAMETER.to_string(), row.value.clone());
    out.insert(FOOTPRINT_PARAMETER.to_string(), row.footprint.clone());
    let variant = Some(variant).filter(|name| !is_base_variant(name));
    let placed = PlacedPart::new(&row.designator, row.link.as_ref());
    if let Some(own) = settings.part(variant, placed) {
        for (name, value) in &own.parameters {
            out.insert(name.clone(), value.clone());
        }
    }
    out.retain(|_, value| !value.trim().is_empty());
    out
}

/// What differs from `from` to `to`: the fit, else (both fitted) each
/// parameter with another value, in name order.
fn changes_between(from: &PartInVariant, to: &PartInVariant) -> Vec<PartChange> {
    if from.fitted != to.fitted {
        return vec![PartChange::Fit];
    }
    if !from.fitted {
        return Vec::new();
    }
    let names: std::collections::BTreeSet<&String> =
        from.parameters.keys().chain(to.parameters.keys()).collect();
    names
        .into_iter()
        .filter(|name| from.parameters.get(*name) != to.parameters.get(*name))
        .map(|name| PartChange::Parameter(name.clone()))
        .collect()
}
