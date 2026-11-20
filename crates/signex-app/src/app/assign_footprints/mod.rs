//! Tools ▸ Assign Footprints (#2248, Graphite #80): a project-wide table of
//! the placed parts and their footprints, like a footprint manager, with a
//! read-only footprint preview.
//!
//! Each part chooses only among the footprints its own library row offers
//! (the default and the alternates, the list Properties shows); its link to
//! the row does not change. Several ticked parts can be set at once. Apply
//! changes every open sheet in memory as one undo step; nothing is written to
//! disk. A sheet that is not open is listed read-only: open it to change its
//! parts. Decision and reasons: `docs/MENU_BAR.md#tools-assign-footprints`.

use std::sync::Arc;

mod apply;
mod filter;
mod preview;
mod read;
mod state;
mod update;

#[cfg(test)]
mod filter_tests;
#[cfg(test)]
mod tests;

pub use filter::{AssignFilter, TickedBar};
pub use preview::{FootprintFacts, PadSize, PinMap, PreviewLoad, pin_map};
pub use read::FootprintRead;
pub use state::{AssignFootprintsState, AssignProblem, FootprintOffer, FootprintPart, PartAccess};

impl From<AssignFootprintsMsg> for crate::app::Message {
    fn from(msg: AssignFootprintsMsg) -> Self {
        Self::Project(crate::app::ProjectMsg::AssignFootprints(msg))
    }
}

/// What happened in the Assign Footprints dialog.
#[derive(Debug, Clone)]
pub enum AssignFootprintsMsg {
    /// The menu row, palette or a key asked for the dialog.
    Requested,
    /// Read `generation` of the project's sheets finished.
    Read {
        generation: u64,
        result: Result<Arc<FootprintRead>, String>,
    },
    /// A footprint was picked for one part.
    FootprintPicked {
        row: usize,
        binding: String,
    },
    /// A footprint was picked for every ticked part.
    TickedPicked(String),
    /// A part's tick box was clicked.
    RowToggled(usize),
    /// The header tick box: tick every listed part, or none (also Clear
    /// ticks).
    AllToggled(bool),
    /// Tick every part of the same library row as this one.
    SameRowTicked(usize),
    /// A filter chip was pressed.
    FilterPicked(AssignFilter),
    /// The filter field was typed in.
    FilterEdited(String),
    /// The pointer entered (`inside`) or left row `row`.
    RowHovered {
        row: usize,
        inside: bool,
    },
    /// A row was clicked: the preview draws its footprint.
    RowPreviewed(usize),
    /// The preview's expand button was pressed.
    PreviewWidthToggled,
    /// A footprint read for the preview of dialog `generation` finished.
    PreviewLoaded {
        generation: u64,
        binding: String,
        result: Result<Arc<FootprintFacts>, String>,
    },
    ApplyRequested,
    CancelRequested,
}
