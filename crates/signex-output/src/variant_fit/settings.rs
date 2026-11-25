//! The project's own variant settings over what the sheets say (mockup row
//! 32): a part's fit and parameter values set in the Variant Manager. The
//! BOM and the matrix both apply them here, so the two cannot disagree. An
//! entry finds its part by the placed part ([`part_link`]), so it stays on
//! it through a re-annotation.

use signex_types::pcb::SymbolLink;
use signex_types::project::{FOOTPRINT_PARAMETER, PlacedPart, VALUE_PARAMETER, VariantSettings};
use signex_types::schematic::InstancePath;
use uuid::Uuid;

use super::matrix::VariantMatrix;

/// The placed part made of `units` (each unit's sheet placement and uuid,
/// in placement order) as the settings name it: the first unit's placement
/// and the units placed there. `None` for a part without units.
pub fn part_link<'p>(
    units: impl IntoIterator<Item = (&'p InstancePath, Uuid)>,
) -> Option<SymbolLink> {
    let mut units = units.into_iter().peekable();
    let sheet = units.peek()?.0.clone();
    let on_sheet: Vec<Uuid> = units
        .filter(|(path, _)| **path == sheet)
        .map(|(_, uuid)| uuid)
        .collect();
    Some(SymbolLink::new(sheet, on_sheet))
}

/// The fit `settings` give the part `placed` names in `variant`, over what
/// its sheets say; `None` leaves the sheets' fit. Base (`None`) has none.
pub fn project_fit<'p>(
    settings: &VariantSettings,
    variant: Option<&str>,
    placed: impl Into<PlacedPart<'p>>,
) -> Option<bool> {
    settings.part(variant, placed)?.fitted
}

/// Whether the part `placed` names, made of `units`, is fitted in `variant`:
/// the settings' fit, else what its sheets say. The fit the BOM counts.
pub fn placed_part_fitted<'p>(
    settings: &VariantSettings,
    variant: Option<&str>,
    placed: impl Into<PlacedPart<'p>>,
    units: &[&signex_types::schematic::Symbol],
) -> bool {
    super::part_fit_over(units, variant, project_fit(settings, variant, placed))
}

/// The value, footprint and fields of one part with `variant`'s parameter
/// values from `settings` put in their place.
pub(crate) fn apply_parameters(
    settings: &VariantSettings,
    variant: Option<&str>,
    placed: PlacedPart<'_>,
    value: &mut String,
    footprint: &mut String,
    fields: &mut std::collections::BTreeMap<String, String>,
) {
    let Some(part) = settings.part(variant, placed) else {
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
    /// applied: a variant's own fit replaces the sheets' and a fitted cell
    /// with a parameter value of its own is marked overridden (a part that
    /// is not fitted has no values to show). Base is the sheets.
    pub fn with_settings(&self, settings: &VariantSettings) -> VariantMatrix {
        let mut out = self.clone();
        for row in &mut out.rows {
            let placed = PlacedPart::new(&row.designator, row.link.as_ref());
            for (index, variant) in self.variants.iter().enumerate() {
                let column = index + 1;
                let Some(part) = settings.part(Some(variant), placed) else {
                    continue;
                };
                if let (Some(fitted), Some(cell)) = (part.fitted, row.fitted.get_mut(column)) {
                    *cell = fitted;
                }
                let fitted = row.fitted.get(column).copied().unwrap_or(true);
                if let Some(cell) = row.overridden.get_mut(column) {
                    *cell = fitted && !part.parameters.is_empty();
                }
            }
        }
        out
    }
}
