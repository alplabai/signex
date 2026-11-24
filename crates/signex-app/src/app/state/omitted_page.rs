//! The rows of the netlist-incomplete prompt: the pages a netlist export
//! leaves out and why.

use super::UnreadableCause;

/// One page a netlist export leaves out, as the netlist-incomplete prompt
/// lists it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OmittedPage {
    /// The page's file as the user knows it: its file name, or the reference
    /// the placing sheet symbol carries.
    pub file: String,
    pub reason: OmittedReason,
}

/// Why a page is not in the netlist. Every reason is structured, so the
/// prompt words it from the catalog (mockup row 28).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum OmittedReason {
    /// No file at its path. `placed_by` names the sheet symbol that places
    /// it, when the stitcher knows it.
    FileMissing { placed_by: Option<String> },
    /// The file exists but could not be read. `cause` is the sheet reader's,
    /// `None` when the set that listed the sheet did not carry one.
    Unreadable { cause: Option<UnreadableCause> },
    /// The stitcher's sheet cycle: `parent` re-enters this sheet.
    SheetCycle { parent: String },
    /// The stitcher's key collision: this sheet and `kept` are one path on
    /// this file system, and only `kept` is in the netlist.
    KeyCollision { kept: String },
}
