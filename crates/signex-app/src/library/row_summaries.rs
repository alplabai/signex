//! The Properties panel's one-line symbol and footprint summaries of the
//! selected library row, read off the UI thread (#859).
//!
//! The summaries need the bound primitives, which are files. Rebuilding the
//! panel context runs after nearly every message, so it only records what it
//! wants ([`RowSummaryJob::want`]); `Signex::update` starts the read
//! ([`RowSummaryJob::begin`]) and the answer is kept only while it is the
//! one asked for ([`RowSummaryJob::finish`]). Until it lands the panel shows
//! the previous answer for the same bindings, or says it is reading.

use std::path::PathBuf;

use signex_library::{ComponentRow, PrimitiveRef};

use super::shared_resolver::SharedResolver;

/// What the summaries are built from. `data_version` moves whenever the
/// library's rows or caches change, so a saved primitive is read again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RowSummaryKey {
    symbol: Option<PrimitiveRef>,
    footprint: Option<PrimitiveRef>,
    data_version: u64,
}

/// A read to run: the key plus what its reports are worded with.
#[derive(Debug, Clone)]
pub struct RowSummaryRequest {
    key: RowSummaryKey,
    internal_pn: String,
    root: PathBuf,
}

impl RowSummaryRequest {
    /// The request for `row` of the library at `root`. A primitive that is
    /// not bound needs no read.
    pub fn for_row(row: &ComponentRow, root: PathBuf, data_version: u64) -> Self {
        Self {
            key: RowSummaryKey {
                symbol: (!row.is_symbol_unbound()).then_some(row.symbol_ref),
                footprint: row.footprint_ref.filter(|_| !row.is_footprint_unbound()),
                data_version,
            },
            internal_pn: row.internal_pn.as_str().to_string(),
            root,
        }
    }

    /// The library whose write order the read waits behind.
    pub fn root(&self) -> &std::path::Path {
        &self.root
    }

    /// Look the bound primitives up; safe on any thread.
    pub fn read(&self, resolver: &SharedResolver) -> RowSummaries {
        let symbol = self.key.symbol.map(|r| match resolver.resolve_symbol(&r) {
            Ok(Some(s)) => SymbolOutcome::Pins(
                s.pins
                    .iter()
                    .filter(|p| p.body_style != signex_library::BODY_ALTERNATE)
                    .count(),
            ),
            Ok(None) => SymbolOutcome::Absent,
            Err(e) => SymbolOutcome::Failed(e.to_string()),
        });
        let footprint = self
            .key
            .footprint
            .map(|r| match resolver.resolve_footprint(&r) {
                Ok(Some(_)) => FootprintOutcome::Found,
                Ok(None) => FootprintOutcome::Absent,
                Err(e) => FootprintOutcome::Failed(e.to_string()),
            });
        RowSummaries { symbol, footprint }
    }

    /// Report the failed lookups to the Messages panel, once per read.
    pub fn report_failures(&self, summaries: &RowSummaries) {
        if let (Some(SymbolOutcome::Failed(e)), Some(r)) = (&summaries.symbol, self.key.symbol) {
            tracing::error!(
                target: "signex::library",
                error = %e,
                library_id = %r.library_id,
                symbol_uuid = %r.uuid,
                internal_pn = %self.internal_pn,
                "symbol could not be read from its library — the binding is not known to be wrong"
            );
        }
        if let (Some(FootprintOutcome::Failed(e)), Some(r)) =
            (&summaries.footprint, self.key.footprint)
        {
            tracing::error!(
                target: "signex::library",
                error = %e,
                library_id = %r.library_id,
                footprint_uuid = %r.uuid,
                internal_pn = %self.internal_pn,
                "footprint could not be read from its library — the binding is not known to be wrong"
            );
        }
    }
}

/// How the bound symbol resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolOutcome {
    /// Found, with this many pins (the alternate body redraws the same
    /// pins, so it is not counted).
    Pins(usize),
    Absent,
    Failed(String),
}

/// How the bound footprint resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FootprintOutcome {
    Found,
    Absent,
    Failed(String),
}

/// The answer to one [`RowSummaryRequest`]; `None` = nothing bound.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowSummaries {
    pub symbol: Option<SymbolOutcome>,
    pub footprint: Option<FootprintOutcome>,
}

/// How many times a read that died is tried for one key; `update` asks again
/// after every message, so an unbounded retry would spin.
const MAX_ATTEMPTS: u8 = 3;

#[derive(Debug, Default)]
pub struct RowSummaryJob {
    wanted: Option<RowSummaryRequest>,
    in_flight: Option<(u64, RowSummaryKey)>,
    applied: Option<(RowSummaryKey, RowSummaries)>,
    failed: Option<(RowSummaryKey, u8)>,
    generation: u64,
}

impl RowSummaryJob {
    /// Record what the panel needs now; `None` when it needs nothing.
    pub fn want(&mut self, request: Option<RowSummaryRequest>) {
        self.wanted = request;
    }

    /// Claim a read to start: none when the wanted key is already answered,
    /// running, or has died too often.
    pub fn begin(&mut self) -> Option<(u64, RowSummaryRequest)> {
        let request = self.wanted.clone()?;
        let key = request.key;
        let answered = self.applied.as_ref().is_some_and(|(k, _)| *k == key);
        let running = self.in_flight.is_some_and(|(_, k)| k == key);
        let exhausted = self
            .failed
            .is_some_and(|(k, tries)| k == key && tries >= MAX_ATTEMPTS);
        if answered || running || exhausted {
            return None;
        }
        self.generation += 1;
        self.in_flight = Some((self.generation, key));
        Some((self.generation, request))
    }

    /// Account for a finished read. `true` when it is the one in flight and
    /// the answer was kept; a read of an older generation changes nothing.
    pub fn finish(&mut self, generation: u64, summaries: Option<RowSummaries>) -> bool {
        let Some((running, key)) = self.in_flight else {
            return false;
        };
        if running != generation {
            return false;
        }
        self.in_flight = None;
        match summaries {
            Some(summaries) => {
                self.failed = None;
                self.applied = Some((key, summaries));
            }
            None => {
                let tries = match self.failed {
                    Some((k, tries)) if k == key => tries + 1,
                    _ => 1,
                };
                self.failed = Some((key, tries));
            }
        }
        true
    }

    /// The request a read of `generation` was started for.
    pub fn request_of(&self, generation: u64) -> Option<&RowSummaryRequest> {
        let (running, key) = self.in_flight?;
        let wanted = self.wanted.as_ref()?;
        (running == generation && wanted.key == key).then_some(wanted)
    }

    /// The latest answer for `row`'s bindings, even from before the
    /// library's data last changed, so a re-read does not blank the panel.
    pub fn summaries_for(&self, row: &ComponentRow) -> Option<&RowSummaries> {
        let (key, summaries) = self.applied.as_ref()?;
        let symbol = (!row.is_symbol_unbound()).then_some(row.symbol_ref);
        let footprint = row.footprint_ref.filter(|_| !row.is_footprint_unbound());
        (key.symbol == symbol && key.footprint == footprint).then_some(summaries)
    }

    /// The summaries are current and nothing is reading them.
    pub fn is_settled(&self) -> bool {
        self.in_flight.is_none()
            && self
                .wanted
                .as_ref()
                .is_none_or(|w| self.applied.as_ref().is_some_and(|(k, _)| *k == w.key))
    }
}
