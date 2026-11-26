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
//! Above all of these, the project's own variant settings (`.snxprj`
//! `variant_settings`, mockup row 32) decide, applied in [`settings`].

mod channels;
#[cfg(test)]
mod channels_tests;
mod drawn;
#[cfg(test)]
mod drawn_tests;
mod fields;
mod matrix;
mod settings;
#[cfg(test)]
mod settings_tests;
#[cfg(test)]
mod tests;

use signex_types::project::is_base_variant;
use signex_types::schematic::Symbol;

pub use channels::{ChannelFit, MatrixChannels, RowChannel};
pub use drawn::{PlacedFits, not_fitted_symbols};
#[cfg(test)]
pub(crate) use fields::field_value_ci;
pub(crate) use fields::{
    is_fit_state_field, merge_fit_states, resolve_active_variant_fitted,
    resolve_base_variant_fitted,
};
pub use matrix::{VariantMatrix, VariantRow, variant_matrix, variant_matrix_in_channels};
pub(crate) use settings::apply_parameters;
pub use settings::{part_link, placed_part_fitted, project_fit};

/// Whether the part made of `units` (its placed units, at least one) is
/// fitted in `variant`; `None` or Base's name is Base. The fit
/// `signex_bom::BomComponent::is_fitted` counts, before the BOM's own
/// in-BOM and on-board gates.
pub fn part_is_fitted(units: &[&Symbol], variant: Option<&str>) -> bool {
    part_fit_over(units, variant, None)
}

/// [`part_is_fitted`] with `project` (the project's own variant setting,
/// [`project_fit`]) deciding over the variant's fields when set.
fn part_fit_over(units: &[&Symbol], variant: Option<&str>, project: Option<bool>) -> bool {
    let variant = variant.filter(|name| !is_base_variant(name));
    let own = project.or_else(|| {
        merge_fit_states(
            units
                .iter()
                .map(|unit| resolve_active_variant_fitted(unit, variant)),
        )
    });
    let base = merge_fit_states(units.iter().map(|unit| resolve_base_variant_fitted(unit)));
    signex_bom::fit_of(own, base, units.iter().any(|unit| unit.dnp))
}

/// Whether one placed symbol is fitted in `variant`; `None` is Base.
pub fn is_fitted(symbol: &Symbol, variant: Option<&str>) -> bool {
    part_is_fitted(&[symbol], variant)
}
