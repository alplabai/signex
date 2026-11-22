//! Renaming a power port renames its rail (mockup row 45): every power port
//! and power label of that name on the project's sheets changes in one undo
//! group, so the net cannot split.
//!
//! A sheet of a loose document is the whole rail and changes at once. In a
//! project the closed sheets are read off the UI thread and stated again, then
//! `write_sheets_as_group` changes every sheet in one `update`, all or nothing.
//! A closed sheet is left unsaved in a tabless engine; nothing is written to
//! disk. A sheet that cannot be read refuses the rename, because it may hold a
//! port of the rail.

mod apply;
#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use signex_engine::{Command, TextTarget};
use signex_i18n::{MessageId, ids};
use signex_types::schematic::{LabelType, SchematicSheet};

use crate::app::Message;
use crate::app::project_sheets::{DiskStamps, ProjectSheetPlan};
use crate::app::sheet_write::SheetWriteRefusal;

/// Reads a rename may start before it gives up on sheets that keep changing.
const MAX_READS: u8 = 3;

/// What finished off the UI thread for the running rename.
#[derive(Debug, Clone)]
pub enum RailRenameMsg {
    /// The project's sheets were read.
    Read {
        job: u64,
        result: Result<Arc<RailRead>, String>,
    },
    /// The closed sheets holding the rail were stated again.
    DiskChecked {
        job: u64,
        result: Result<Vec<PathBuf>, String>,
    },
}

impl From<RailRenameMsg> for Message {
    fn from(msg: RailRenameMsg) -> Self {
        Self::Edit(crate::app::EditMsg::RailRename(msg))
    }
}

/// One rename: the port typed in, and the rail's old and new names.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RailRename {
    pub(crate) sheet: PathBuf,
    pub(crate) port: uuid::Uuid,
    pub(crate) old: String,
    pub(crate) new: String,
    /// Reads started and superseded so far.
    attempts: u8,
}

impl RailRename {
    pub(crate) fn new(sheet: PathBuf, port: uuid::Uuid, old: String, new: String) -> Self {
        Self {
            sheet,
            port,
            old,
            new,
            attempts: 0,
        }
    }
}

/// The project as the read found it.
#[derive(Debug, Default)]
pub struct RailRead {
    /// The project's sheets an engine held when the read started.
    pub(crate) open: Vec<PathBuf>,
    /// The closed sheets holding the rail.
    pub(crate) loaded: HashMap<PathBuf, SchematicSheet>,
    /// The stamp of each `loaded` file, taken just before it was read.
    pub(crate) stamps: DiskStamps,
    /// Sheets that exist but could not be parsed: `(path, why)`.
    pub(crate) unreadable: Vec<(PathBuf, String)>,
}

/// The rename whose read is running.
pub(crate) struct RunningRename {
    job: u64,
    rename: RailRename,
    epoch: u64,
    project: crate::app::state::ProjectId,
    read: Option<Arc<RailRead>>,
}

/// Renames waiting for their project's sheets, one running at a time.
#[derive(Default)]
pub(crate) struct RailRenames {
    pub(crate) queue: VecDeque<RailRename>,
    running: Option<RunningRename>,
    next_job: u64,
}

impl RailRenames {
    fn is_running(&self, job: u64) -> bool {
        self.running.as_ref().is_some_and(|r| r.job == job)
    }

    /// Forget the renames typed on the sheets `matches` picks: a discard
    /// throws them away with the sheet's other edits.
    pub(crate) fn drop_where(&mut self, matches: &dyn Fn(&Path) -> bool) {
        self.queue.retain(|rename| !matches(&rename.sheet));
        if (self.running.as_ref()).is_some_and(|r| matches(&r.rename.sheet)) {
            self.running = None;
        }
    }
}

/// Why a rename changed nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RailProblem {
    EmptyName,
    PortGone,
    Stale,
    ReadFailed(String),
    Unreadable(PathBuf),
    Shared(PathBuf),
    Busy(PathBuf),
    ReadOnly(PathBuf),
    NotLoaded(PathBuf),
    NotChanged(PathBuf),
}

impl From<SheetWriteRefusal> for RailProblem {
    fn from(refusal: SheetWriteRefusal) -> Self {
        match refusal {
            SheetWriteRefusal::Busy(path) => Self::Busy(path),
            SheetWriteRefusal::ReadOnly(path) => Self::ReadOnly(path),
            SheetWriteRefusal::NotLoaded(path) => Self::NotLoaded(path),
            SheetWriteRefusal::NotChanged(path) => Self::NotChanged(path),
        }
    }
}

impl RailProblem {
    /// The catalog line and the text its `$path` shows.
    fn why(&self) -> (MessageId, String) {
        let shown = |path: &PathBuf| path.display().to_string();
        match self {
            Self::EmptyName => (ids::PROPERTIES_RAIL_WHY_EMPTY, String::new()),
            Self::PortGone => (ids::PROPERTIES_RAIL_WHY_PORT_GONE, String::new()),
            Self::Stale => (ids::PROPERTIES_RAIL_WHY_STALE, String::new()),
            Self::ReadFailed(why) => (ids::PROPERTIES_RAIL_WHY_READ_FAILED, why.clone()),
            Self::Unreadable(p) => (ids::PROPERTIES_RAIL_WHY_UNREADABLE, shown(p)),
            Self::Shared(p) => (ids::PROPERTIES_RAIL_WHY_SHARED, shown(p)),
            Self::Busy(p) => (ids::PROPERTIES_RAIL_WHY_BUSY, shown(p)),
            Self::ReadOnly(p) => (ids::PROPERTIES_RAIL_WHY_READ_ONLY, shown(p)),
            Self::NotLoaded(p) => (ids::PROPERTIES_RAIL_WHY_NOT_LOADED, shown(p)),
            Self::NotChanged(p) => (ids::PROPERTIES_RAIL_WHY_NOT_CHANGED, shown(p)),
        }
    }
}

/// The edits that rename the rail `old` on `sheet`. A power flag names no net,
/// so it is not part of a rail.
pub(crate) fn rail_commands(sheet: &SchematicSheet, old: &str, new: &str) -> Vec<Command> {
    let ports = (sheet.symbols.iter())
        .filter(|s| s.is_power && !signex_net::is_power_flag(&s.lib_id) && s.value == old)
        .map(|s| TextTarget::SymbolValue(s.uuid));
    let labels = (sheet.labels.iter())
        .filter(|l| l.label_type == LabelType::Power && l.text == old)
        .map(|l| TextTarget::Label(l.uuid));
    ports
        .chain(labels)
        .map(|target| Command::UpdateText {
            target,
            value: new.to_string(),
        })
        .collect()
}

/// Whether `sheet` holds a port of the rail `old`.
fn holds_rail(sheet: &SchematicSheet, old: &str) -> bool {
    !rail_commands(sheet, old, old).is_empty()
}

/// The disk half: the project's sheets, and copies of the closed ones holding
/// the rail with the stamps they were read at.
fn read_rail(plan: &ProjectSheetPlan, open: &HashSet<PathBuf>, old: &str) -> RailRead {
    let (set, mut read_stamps) = plan.assemble_stamped();
    let mut read = RailRead {
        unreadable: set.unreadable,
        ..RailRead::default()
    };
    for (path, sheet) in set.sheets {
        if open.contains(&path) {
            read.open.push(path);
        } else if holds_rail(&sheet, old) {
            let stamp = read_stamps.remove(&path).flatten();
            read.stamps.insert(path.clone(), stamp);
            read.loaded.insert(path, sheet);
        }
    }
    read
}
