use std::path::PathBuf;

use signex_engine::FindHit;

/// One result: where it is (`sheet`, plus the hit's placement and position)
/// and what the scan saw there. The hit's text is for display; a replace
/// re-reads the live document.
#[derive(Debug, Clone)]
pub struct FindMatch {
    /// The sheet file the hit is on.
    pub sheet: Option<PathBuf>,
    pub hit: FindHit,
}

#[derive(Debug, Clone, Default)]
pub struct FindReplaceState {
    pub open: bool,
    pub replace_mode: bool,
    pub query: String,
    pub replacement: String,
    pub matches: Vec<FindMatch>,
    pub selected_index: Option<usize>,
}

#[derive(Debug, Clone)]
pub enum FindReplaceMsg {
    Close,
    QueryChanged(String),
    ReplacementChanged(String),
    SelectResult(usize),
    ReplaceCurrent,
    ReplaceAll,
}
