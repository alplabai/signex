//! Messages of the actions on a placed part that lead into the library:
//! Open Datasheet (#1301), Edit Symbol (#1262), Change Component (#1260).

use std::path::PathBuf;

use uuid::Uuid;

use super::ResolvedPlacement;
use crate::library::state::{ChangeFilter, ChangeScope};

/// What the user asked of the selected placed part, from the canvas context
/// menu, the Properties panel or a command id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacedPartAction {
    OpenDatasheet,
    EditSymbol,
    ChangeComponent,
}

/// The placed symbol a Change Component started on. A completion that no
/// longer finds this sheet, engine instance and symbol is dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedPartTarget {
    pub sheet: PathBuf,
    /// The engine that held the sheet then; a re-opened sheet is another one.
    pub engine_instance: u64,
    pub symbol_id: Uuid,
}

#[derive(Debug, Clone)]
pub enum PlacedPartMsg {
    /// An action was asked of the selected placed part.
    ActionRequested(PlacedPartAction),
    /// A document was chosen in the datasheet list (index into it).
    DatasheetPicked(usize),
    /// The platform opener could not show the datasheet.
    DatasheetOpenFailed(String),
    /// A library part was picked as the replacement; its read is next.
    ChangePartPicked,
    /// The off-thread read of the picked part finished.
    ChangePartResolved {
        target: PlacedPartTarget,
        /// The row that was read; a result for a row that is no longer the
        /// picked one is dropped.
        row_id: Uuid,
        result: Result<Box<ResolvedPlacement>, String>,
    },
    /// The preview was left to pick another part.
    ChangePreviewBackedOut,
    /// The preview was confirmed: apply the change.
    ChangeConfirmed,
    /// A row of Change Component's list was clicked: pick it and read it.
    ChangeRowClicked(Uuid),
    /// A filter chip of Change Component's list was pressed.
    ChangeFilterToggled(ChangeFilter),
    /// A user parameter's Keep now box was toggled.
    ChangeKeepToggled(String),
    /// An Apply to choice was picked.
    ChangeScopePicked(ChangeScope),
    /// The old pin `old` was mapped by hand to the new pin `new`, or to no
    /// pin (#2954).
    ChangePinRemapped { old: String, new: Option<String> },
    /// The off-thread read of the listed rows' pin counts finished; `Err`
    /// carries the runtime's text when the read's thread panicked.
    ChangePinCountsRead {
        target: PlacedPartTarget,
        counts: Result<Vec<(Uuid, usize)>, String>,
    },
}
