//! The rows of the netlist-incomplete prompt: the pages a netlist export
//! leaves out and why.

/// One page a netlist export leaves out, as the netlist-incomplete prompt
/// lists it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OmittedPage {
    /// The page's file as the user knows it: its file name, or the reference
    /// the placing sheet symbol carries.
    pub file: String,
    pub reason: OmittedReason,
}

/// Why a page is not in the netlist.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum OmittedReason {
    /// No file at its path. `placed_by` names the sheet symbol that places
    /// it, when the stitcher knows it.
    FileMissing { placed_by: Option<String> },
    /// The file exists but could not be read: the reader's own English text.
    Unreadable(String),
    /// The stitcher's English line: a sheet cycle, or two sheets on one key.
    Stitch(String),
}
