//! Adding an alternate symbol to the row open in a Component Preview tab
//! (#1230): the picked symbol is checked against the default symbol's pin
//! numbers before it joins the row. The lookups run off the UI thread
//! (`library::alternate_reads`, #1756).

use iced::Task;

use super::super::super::Message;
use super::super::super::state::Signex;
use crate::library::alternate_reads::{AlternateOutcome, alternate_symbol_task};
use crate::library::shared_resolver::OneShotCell;
use crate::library::state::EditorAddress;
use signex_library::primitive::PrimitiveRef;

impl Signex {
    /// Start adding `picked` as an alternate of the previewed row; the row
    /// changes when the answer lands.
    pub(super) fn add_alternate_symbol_from_pick(
        &mut self,
        address: EditorAddress,
        picked: PrimitiveRef,
    ) -> Task<Message> {
        alternate_symbol_task(&mut self.library, &address, picked)
    }

    /// Apply the symbols a worker looked up for an alternate pick.
    pub(super) fn handle_alternate_symbol_resolved(
        &mut self,
        readout: OneShotCell<AlternateOutcome>,
    ) {
        match readout.take() {
            Some(Some(outcome)) => self.library.apply_alternate_symbol(outcome),
            Some(None) => crate::diagnostics::log_warning(
                "[library] looking up the alternate symbol did not finish; pick it again",
            ),
            None => {}
        }
    }
}
