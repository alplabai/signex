//! Compare Board with Schematic (#2462): every difference between the
//! project's schematic and its board, both ways, in one modal report. Each
//! row has its own direction (Update PCB, Update schematic or Skip);
//! Apply runs the PCB side as one labelled board step through Update PCB's
//! Execute path, then the schematic side as one Back Annotate undo group.
//!
//! The report is `signex_engine::pcb::eco::compare` over the two change
//! lists Update PCB (`app/update_pcb/design.rs`) and Back Annotate
//! (`app/back_annotate/read.rs`) read; one worker task runs both reads
//! (`read.rs`). Entry points: Project ▸ Compare Board with Schematic…, the
//! board right-click menu, the palette. Behaviour and decisions:
//! `docs/UPDATE_PCB.md`, "Compare Board with Schematic".

mod apply;
mod read;
mod state;
mod update;

#[cfg(test)]
mod tests;

use std::path::PathBuf;
use std::sync::Arc;

use signex_engine::pcb::eco::{CompareJump, CompareKind, CompareReport, CompareRowId, Direction};

pub use read::CompareRead;
pub use state::CompareBoardState;

impl From<CompareBoardMsg> for crate::app::Message {
    fn from(msg: CompareBoardMsg) -> Self {
        Self::Pcb(crate::app::PcbMsg::CompareBoard(msg))
    }
}

/// Which editor a jump link shows the object in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JumpSide {
    Schematic,
    Board,
}

/// What happened in the Compare Board with Schematic dialog, namespaced
/// under `PcbMsg::CompareBoard` and routed to
/// `dispatch_compare_board_message`.
#[derive(Debug, Clone, PartialEq)]
pub enum CompareBoardMsg {
    /// `compare_board_with_schematic` ran for the document in view.
    Requested,
    /// Both reads of job `job` landed.
    Read {
        job: u64,
        result: Result<Arc<CompareRead>, String>,
    },
    /// The net rows of job `job` were recomputed for the footprint
    /// directions chosen.
    NetsRefreshed {
        job: u64,
        report: Arc<CompareReport>,
    },
    /// A kind chip was clicked; `None`: All.
    KindFilterPicked(Option<CompareKind>),
    RowDirectionPicked {
        row: CompareRowId,
        direction: Direction,
    },
    /// "Set shown rows": every shown row that allows it takes `direction`.
    ShownRowsDirectionPicked(Direction),
    CompareAgainRequested,
    ExportRequested,
    /// The Export report… save dialog closed (`None`: cancelled).
    ExportPathPicked(Option<PathBuf>),
    Exported(Result<PathBuf, String>),
    ApplyRequested,
    /// The confirmation listing the board deletions was answered.
    ApplyConfirmed,
    ApplyCancelled,
    /// The closed sheets of job `job` were stated again before Apply
    /// writes: the files that changed on disk since the read, or why the
    /// check failed.
    DiskChecked {
        job: u64,
        result: Result<Vec<PathBuf>, String>,
    },
    /// A jump link: show the row's object in one editor; the dialog stays.
    JumpRequested {
        side: JumpSide,
        target: CompareJump,
    },
    CloseRequested,
}
