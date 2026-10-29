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
mod jump;
mod reported;

pub use highlight::NetHighlight;
pub use history::SheetHistory;
pub use hover::{HoverCard, HoverState};
pub use index_cache::{IndexOutcome, NetIndexCache, ProjectNetData, Stored};
pub use jump::JumpCursor;
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

/// A command asked while its project's index was being built, with what it
/// was asked about: it runs only if that is still what the user is looking at.
#[derive(Debug, Clone)]
pub struct Pending {
    pub project: crate::app::state::ProjectId,
    pub action: PendingNetAction,
    pub sheet: Option<std::path::PathBuf>,
    pub selected: Vec<signex_types::schematic::SelectedItem>,
}

/// Everything cross-sheet navigation keeps between messages.
#[derive(Default)]
pub struct NetNavState {
    pub history: SheetHistory,
    pub highlight: Option<NetHighlight>,
    pub hover: HoverState,
    pub index: NetIndexCache,
    /// A command waiting for its project's index.
    pub pending: Option<Pending>,
    /// Where the run of Go to net partner is in its ring.
    pub jump: Option<JumpCursor>,
    /// Stitch warnings already shown, shared with the net-class refresh.
    pub reported: ReportedWarnings,
    /// What the highlight's index refresh last saw, so an update that changed
    /// no connectivity, sheet list or class table does no planning.
    pub refresh_stamp: Option<u64>,
}
