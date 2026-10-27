//! Request counters for the off-thread read behind a placed symbol's switch
//! to another drawing (#1230): a result that is not the latest request for
//! its symbol is stale and must not be applied.

use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Default)]
pub struct SymbolChoiceRequests {
    latest: HashMap<Uuid, u64>,
    issued: u64,
}

impl SymbolChoiceRequests {
    /// Start a request for `symbol`, superseding any earlier one.
    pub fn begin(&mut self, symbol: Uuid) -> u64 {
        self.issued += 1;
        self.latest.insert(symbol, self.issued);
        self.issued
    }

    /// Whether `request` is still the latest one for `symbol`.
    pub fn is_latest(&self, symbol: Uuid, request: u64) -> bool {
        self.latest.get(&symbol) == Some(&request)
    }
}
