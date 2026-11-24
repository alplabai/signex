//! The project's own variant settings over what the sheets say (mockup row
//! 32): a part's fit and parameter values set in the Variant Manager. The
//! BOM and the matrix both apply them here, so the two cannot disagree.

use signex_types::project::{FOOTPRINT_PARAMETER, VALUE_PARAMETER, VariantSettings};

use super::matrix::VariantMatrix;

/// The fit `settings` give `designator` in `variant`, over what its sheets
/// say; `None` leaves the sheets' fit. Base (`None`) has none.
pub fn project_fit(
    settings: &VariantSettings,
    variant: Option<&str>,
    designator: &str,
) -> Option<bool> {
    settings.part(variant, designator)?.fitted
}

/// The value, footprint and fields of one part with `variant`'s parameter
/// values from `settings` put in their place.
pub(crate) fn apply_parameters(
    settings: &VariantSettings,
    variant: Option<&str>,
    designator: &str,
    value: &mut String,
    footprint: &mut String,
    fields: &mut std::collections::BTreeMap<String, String>,
) {
    let Some(part) = settings.part(variant, designator) else {
        return;
    };
    for (parameter, set) in &part.parameters {
        match parameter.as_str() {
            VALUE_PARAMETER => value.clone_from(set),
            FOOTPRINT_PARAMETER => footprint.clone_from(set),
            _ => {
                fields.insert(parameter.clone(), set.clone());
            }
        }
    }
}

impl VariantMatrix {
    /// This matrix, read from the sheets, with the project's `settings`
    /// applied: a variant's own fit replaces the sheets' and a cell with a
    /// parameter value of its own is marked overridden. Base is the sheets.
    pub fn with_settings(&self, settings: &VariantSettings) -> VariantMatrix {
        let mut out = self.clone();
        for row in &mut out.rows {
            for (index, variant) in self.variants.iter().enumerate() {
                let column = index + 1;
                let Some(part) = settings.part(Some(variant), &row.designator) else {
                    continue;
                };
                if let (Some(fitted), Some(cell)) = (part.fitted, row.fitted.get_mut(column)) {
                    *cell = fitted;
                }
                if let Some(cell) = row.overridden.get_mut(column) {
                    *cell = !part.parameters.is_empty();
                }
            }
        }
        out
    }
}
