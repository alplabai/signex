//! The text a symbol's value field is drawn with (#1596, #1986): the one
//! rule the PDF and SVG print it by (`signex_output::shown_value`). The
//! content scene draws it and the hit boxes measure it, so a box fits the
//! evaluated `20kΩ`, not the `= 2 * R1.Value` stored. The active variant's
//! own value for a part is drawn in place of the sheet's (#2948), as the
//! PDF and SVG print the printed variant's (`signex_output::drawn_symbol`).
//! Also the designator a canvas draws (#1151, #1416), for the lists that name
//! parts as it does.

use std::borrow::Cow;

use signex_output::DrawnValueMap;
use signex_types::designator::{DesignatorStandard, display_reference};
use signex_types::schematic::{InstancePath, Symbol};

use crate::canvas::SpecialStrings;

/// The drawn value of `symbol` among `symbols` (its sheet), with the
/// sheet's special strings when the canvas has them and the active
/// variant's values (`variant`) when it gives the part one.
pub(crate) fn drawn_value(
    symbols: &[Symbol],
    symbol: &Symbol,
    special: Option<&SpecialStrings>,
    standard: Option<DesignatorStandard>,
    variant: Option<&DrawnValueMap>,
) -> String {
    let symbol = signex_output::drawn_symbol(symbol, variant);
    let signed = standard.is_some_and(DesignatorStandard::shows_product_aspect_sign);
    match special {
        Some(special) => special.resolve_value(symbols, &symbol, signed),
        None => {
            let scope = signex_output::ValueScope {
                signed,
                ..signex_output::ValueScope::of_sheet(symbols, None)
            };
            signex_output::shown_value(&scope, &symbol).text
        }
    }
}

/// Whether `variant` gives `symbol` a value other than its own: the value
/// the canvas then draws in the accent (mockup row 209).
pub(crate) fn is_variant_value(symbol: &Symbol, variant: Option<&DrawnValueMap>) -> bool {
    variant
        .and_then(|values| values.get(&symbol.uuid))
        .is_some_and(|value| *value != symbol.value)
}

/// The designator the canvas draws for `symbol` on a sheet shown at
/// `placement` (its designator there, #1151; `reference` with none), in the
/// project's designator `standard` (#1416).
pub(crate) fn drawn_designator<'s>(
    symbol: &'s Symbol,
    placement: Option<&InstancePath>,
    standard: Option<DesignatorStandard>,
) -> Cow<'s, str> {
    let stored = match placement {
        Some(placement) => symbol.designator_in(placement),
        None => symbol.reference.as_str(),
    };
    display_reference(standard, stored)
}
