//! Update PCB from schematic (#1312): the dialog that lists the change list
//! (`signex_engine::pcb::eco`), its entry points and New PCB in a project.
//! Behaviour and the decisions behind it: `docs/UPDATE_PCB.md`.
//!
//! The design read and the diff run off the update thread (`design.rs`);
//! Execute runs the ticked rows as one labelled board step at the list's
//! revision, so it is one undo step and refused once the board moved on.

mod command;
pub(crate) mod design;
mod execute;
mod fix;
mod new_pcb;
mod state;
pub(crate) mod table;
mod target;
mod update;

pub(crate) use command::update_pcb_from_schematic;
pub(crate) use design::DesignSnapshot;
pub(crate) use new_pcb::new_board_path;
pub use state::UpdatePcbState;
pub(crate) use state::{BoardStatus, Phase, file_name};
pub use table::{RowAction, RowFilter, TableGroup};
pub(crate) use target::PcbSyncLabel;

#[cfg(test)]
mod tests;

use std::path::PathBuf;
use std::sync::Arc;

use signex_engine::pcb::eco::{EcoChangeList, EcoKey};

use crate::app::state::ProjectId;

impl From<UpdatePcbMsg> for crate::app::Message {
    fn from(msg: UpdatePcbMsg) -> Self {
        Self::UpdatePcb(msg)
    }
}

/// What the read on the worker produced: the design as read and its change
/// list.
pub struct Computed {
    pub(crate) snapshot: Arc<DesignSnapshot>,
    pub(crate) list: EcoChangeList,
}

/// Terse: every message is formatted for the frame probe, and the design is
/// large.
impl std::fmt::Debug for Computed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Computed")
            .field("revision", &self.list.revision)
            .field("rows", &self.list.rows.len())
            .finish_non_exhaustive()
    }
}

/// What happened in Update PCB and New PCB, namespaced under
/// `Message::UpdatePcb` (ADR-0001 D3) and routed to
/// `dispatch_update_pcb_message`.
#[derive(Debug, Clone)]
pub enum UpdatePcbMsg {
    /// `update_pcb_from_schematic` ran for the document in view. `apply`
    /// (headless `apply=true`) executes the rows ticked by default once the
    /// list is ready, without a dialog.
    Requested {
        apply: bool,
    },
    /// Projects tree, project row: Update PCB Document for that project.
    RequestedForProject(ProjectId),
    /// The design read and the change list of job `job` landed.
    Computed {
        job: u64,
        result: Result<Arc<Computed>, String>,
    },
    /// The net rows of job `job` were recomputed after a footprint tick.
    NetsRefreshed {
        job: u64,
        list: Arc<EcoChangeList>,
    },
    RowToggled {
        key: EcoKey,
        on: bool,
    },
    /// A group's tri-state box: tick or untick its open rows.
    GroupToggled {
        group: TableGroup,
        on: bool,
    },
    /// A group's heading was clicked: fold or open it.
    GroupFolded {
        group: TableGroup,
        folded: bool,
    },
    /// A summary tile was clicked: list only its rows, or every row again
    /// when it was the tile in force.
    FilterChipClicked(RowFilter),
    /// The Problems tile set (`true`) or cleared as a toggle: the
    /// headless and test form of a Problems tile click.
    ProblemsOnlyToggled(bool),
    /// The filter field's text changed: list only the rows it names.
    FilterQueryChanged(String),
    /// Clear filter: no tile and no filter text.
    FilterCleared,
    /// The Notes group's heading was clicked: fold or open it.
    NotesFolded(bool),
    /// A row was clicked: Space now ticks it.
    RowFocused(EcoKey),
    /// Space: tick or untick the focused row.
    FocusedRowToggled,
    /// A problem row's Choose footprint...: the footprint picker opens for
    /// its part.
    ChooseFootprintRequested(EcoKey),
    /// The footprint picker picked `footprint` for the part with anchor unit
    /// `part` on placement `sheet`.
    FootprintChosen {
        sheet: signex_types::schematic::InstancePath,
        part: uuid::Uuid,
        footprint: signex_library::PrimitiveRef,
    },
    RefreshRequested,
    ValidateRequested,
    ExecuteRequested,
    ReportRequested,
    /// The Report... save dialog closed (`None`: cancelled).
    ReportPathPicked(Option<PathBuf>),
    ReportWritten(Result<PathBuf, String>),
    /// A blocked row's Annotate... link.
    AnnotateRequested,
    /// A blocked row's Cross Probe link: show the part on its sheet.
    CrossProbeRequested(EcoKey),
    /// The parked board's Reopen button.
    ReopenRequested,
    /// The no-board banner's Create PCB button.
    CreatePcbRequested,
    CloseRequested,
    /// `new_pcb_document`: open the New PCB setup dialog for the project in
    /// view (`None`), or for the project the Projects tree names.
    NewPcbRequested(Option<ProjectId>),
    /// The disk check of the new board's path finished: `Ok(true)` (a file
    /// is there) and `Err` (the check failed) refuse it.
    NewPcbPathChecked {
        project: ProjectId,
        path: PathBuf,
        exists: Result<bool, String>,
        /// The rules the setup dialog or the command chose; `None`: the
        /// project defaults.
        rules: Option<signex_types::project::PcbDefaults>,
        /// The fab tier picked with them, whose `fab_*` rules the project
        /// takes when the board is made (#2311).
        fab: Option<signex_types::project::FabTemplate>,
    },
}
