//! The text a symbol's value field is drawn with (#1596, #1986): the one
//! rule the PDF and SVG print it by (`signex_output::shown_value`). The
//! content scene draws it and the hit boxes measure it, so a box fits the
//! evaluated `20kΩ`, not the `= 2 * R1.Value` stored.

use signex_types::designator::DesignatorStandard;
use signex_types::schematic::Symbol;

use crate::canvas::SpecialStrings;

/// The drawn value of `symbol` among `symbols` (its sheet), with the
/// sheet's special strings when the canvas has them.
pub(crate) fn drawn_value(
    symbols: &[Symbol],
    symbol: &Symbol,
    special: Option<&SpecialStrings>,
    standard: Option<DesignatorStandard>,
) -> String {
    let signed = standard.is_some_and(DesignatorStandard::shows_product_aspect_sign);
    match special {
        Some(special) => special.resolve_value(symbols, symbol, signed),
        None => {
            let scope = signex_output::ValueScope {
                signed,
                ..signex_output::ValueScope::of_sheet(symbols, None)
            };
            signex_output::shown_value(&scope, symbol).text
        }
    }
}
