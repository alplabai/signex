//! The power rail renames waiting and running: plain state, UI-free
//! (ADR-0006). The steps that read and write sheets are in `app::rail_rename`.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use signex_types::schematic::SchematicSheet;

use crate::app::core::file_stamp::DiskStamps;
use crate::app::core::project_id::ProjectId;

/// One rename: the port typed in, and the rail's old and new names.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RailRename {
    pub(crate) sheet: PathBuf,
    pub(crate) port: uuid::Uuid,
    pub(crate) old: String,
    pub(crate) new: String,
    /// Reads started and superseded so far.
    pub(crate) attempts: u8,
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
    pub(crate) job: u64,
    pub(crate) rename: RailRename,
    pub(crate) epoch: u64,
    pub(crate) project: ProjectId,
    pub(crate) read: Option<Arc<RailRead>>,
}

/// Renames waiting for their project's sheets, one running at a time.
#[derive(Default)]
pub struct RailRenames {
    pub(crate) queue: VecDeque<RailRename>,
    pub(crate) running: Option<RunningRename>,
    pub(crate) next_job: u64,
}

impl RailRenames {
    pub(crate) fn is_running(&self, job: u64) -> bool {
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
