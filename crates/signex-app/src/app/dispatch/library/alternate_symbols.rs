//! Adding an alternate symbol to the row open in a Component Preview tab
//! (#1230): the picked symbol is checked against the default symbol's pin
//! numbers before it joins the row.

use super::super::super::state::Signex;
use crate::library::state::EditorAddress;
use signex_library::primitive::{PrimitiveRef, Symbol};

impl Signex {
    /// Add `picked` as an alternate of the previewed row. A refusal (an
    /// unresolved symbol, a pin-number mismatch, a repeat) is shown in the
    /// editor and logged to the Messages panel; the row is left as it was.
    pub(super) fn add_alternate_symbol_from_pick(
        &mut self,
        address: EditorAddress,
        picked: PrimitiveRef,
    ) {
        let Some(state) = self.library.editors.get_mut(&address) else {
            return;
        };
        let pn = state.row.internal_pn.to_string();
        let default = resolve(
            &self.library.set,
            &state.row.symbol_ref,
            "the default symbol",
        );
        let alternate = resolve(&self.library.set, &picked, "the picked symbol");
        let outcome = match (default, alternate) {
            (Ok(default), Ok(alternate)) => {
                signex_library::add_alternate_symbol(&mut state.row, &default, picked, &alternate)
                    .map_err(|e| e.to_string())
            }
            (Err(why), _) | (_, Err(why)) => Err(why),
        };
        match outcome {
            Ok(()) => {
                state.alternate_error = None;
                state.mark_dirty();
            }
            Err(why) => {
                crate::diagnostics::log_warning(format!(
                    "Alternate symbol not added to {pn}: {why}."
                ));
                state.alternate_error = Some(why);
            }
        }
    }
}

/// The symbol `reference` names, or why it could not be read.
fn resolve(
    set: &signex_library::LibrarySet,
    reference: &PrimitiveRef,
    what: &str,
) -> Result<Symbol, String> {
    match set.resolve_symbol(reference) {
        Ok(Some(symbol)) => Ok(symbol),
        Ok(None) => Err(format!(
            "{what} is not in any open library, so pin compatibility cannot be checked"
        )),
        Err(e) => Err(format!("{what} could not be read ({e})")),
    }
}
