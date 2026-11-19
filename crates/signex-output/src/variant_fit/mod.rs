//! Whether a part is fitted in a variant (#1690, #1293), and the parts by
//! variants matrix the Project tab's Variants section shows. One decision for
//! the BOM and the app: the fit fields are read in [`fields`] and nowhere else.
//!
//! A part's fit in a variant is the variant's own override (a property's
//! `variant_overrides`, then the fields `Fitted@V`, `DNP@V`, `V.fitted`,
//! `variant:V:dnp`), else the base fields (`VariantFitted`, `Fitted`, `DNP`,
//! then the same keys as custom properties), else fitted unless the part is
//! marked DNP. Fitted outranks DNP when both are set; a unit that is not
//! fitted makes its part not fitted. Base is the part without any override.

mod fields;
mod matrix;
#[cfg(test)]
mod tests;

use signex_types::project::is_base_variant;
use signex_types::schematic::Symbol;

#[cfg(test)]
pub(crate) use fields::field_value_ci;
pub(crate) use fields::{
    is_fit_state_field, merge_fit_states, resolve_active_variant_fitted,
    resolve_base_variant_fitted,
};
pub use matrix::{VariantMatrix, VariantRow, variant_matrix};

/// Whether the part made of `units` (its placed units, at least one) is
/// fitted in `variant`; `None` or Base's name is Base. The fit
/// `signex_bom::BomComponent::is_fitted` counts, before the BOM's own
/// in-BOM and on-board gates.
pub fn part_is_fitted(units: &[&Symbol], variant: Option<&str>) -> bool {
    let variant = variant.filter(|name| !is_base_variant(name));
    let own = merge_fit_states(
        units
            .iter()
            .map(|unit| resolve_active_variant_fitted(unit, variant)),
    );
    let base = merge_fit_states(units.iter().map(|unit| resolve_base_variant_fitted(unit)));
    own.or(base).unwrap_or(!units.iter().any(|unit| unit.dnp))
}

/// Whether one placed symbol is fitted in `variant`; `None` is Base.
pub fn is_fitted(symbol: &Symbol, variant: Option<&str>) -> bool {
    part_is_fitted(&[symbol], variant)
}
