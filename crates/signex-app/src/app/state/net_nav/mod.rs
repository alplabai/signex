//! Cross-sheet net navigation: the sheet history, the persistent net
//! highlight, the hover insight and the cached project net index they read
//! (#1270, #1271, #1272).
//!
//! The Model holds all of it; the connectivity itself is the stitched project
//! netlist's ([`signex_net::NetIndex`]), never derived here.

mod highlight;
mod history;
pub mod hover;
mod index_cache;
mod reported;

pub use highlight::NetHighlight;
pub use history::SheetHistory;
pub use hover::{HoverCard, HoverState};
pub use index_cache::{IndexOutcome, NetIndexCache, ProjectNetData, Stored};
pub use reported::ReportedWarnings;

/// What the user asked for while the index it needs was still being built:
/// run again once the index arrives. The latest request wins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingNetAction {
    JumpToPartner,
    HighlightUnderCursor,
    HighlightRelated,
    StepHighlight { forward: bool },
    HoverDwell,
}

/// Everything cross-sheet navigation keeps between messages.
#[derive(Default)]
pub struct NetNavState {
    pub history: SheetHistory,
    pub highlight: Option<NetHighlight>,
    pub hover: HoverState,
    pub index: NetIndexCache,
    /// The project it was asked in, and what to run when its index lands.
    pub pending: Option<(crate::app::state::ProjectId, PendingNetAction)>,
    /// Stitch warnings already shown, shared with the net-class refresh.
    pub reported: ReportedWarnings,
    /// What the highlight's index refresh last saw, so an update that changed
    /// no connectivity, sheet list or class table does no planning.
    pub refresh_stamp: Option<u64>,
}
