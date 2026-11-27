//! The live check as a pure, blocking job: the edit scope of each open sheet
//! against its baseline, and the same run `check_erc` makes, raising no
//! Messages line. Runs in `spawn_blocking`, never in `update`.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use signex_erc::live::{EditScope, edit_scope};
use signex_types::schematic::SchematicSheet;

use super::super::erc_job::{ErcRunInputs, PageFrames, run_erc_check_job};
use super::Baseline;

/// What a live check reads.
pub(super) struct LiveErcInputs {
    pub run: ErcRunInputs,
    /// The baseline of each open sheet; an open sheet with none is new to
    /// live ERC.
    pub baselines: HashMap<PathBuf, Baseline>,
}

/// What a live check found.
#[derive(Debug)]
pub struct LiveErcOutcome {
    /// Every finding of the run, by sheet, waived ones left out.
    pub by_path: HashMap<PathBuf, Vec<signex_erc::Violation>>,
    /// What the edit touched on each open sheet.
    pub scopes: HashMap<PathBuf, EditScope>,
    /// The names of every changed net, for the sheets the edit did not change.
    pub changed_names: BTreeSet<String>,
    /// Each open sheet as this check read it: the next baseline.
    pub read: HashMap<PathBuf, Arc<SchematicSheet>>,
    pub page_frames: PageFrames,
}

impl LiveErcOutcome {
    /// What a live check re-checks on the sheet at `path`.
    pub(crate) fn scope_of(&self, path: &Path) -> EditScope {
        self.scopes
            .get(path)
            .cloned()
            .unwrap_or_else(|| EditScope::named(self.changed_names.clone()))
    }
}

pub(super) fn run_live_erc_job(inputs: LiveErcInputs) -> LiveErcOutcome {
    let LiveErcInputs { run, baselines } = inputs;
    // The sheets as the run reads them: the active tab's render snapshot wins.
    let mut read: HashMap<PathBuf, Arc<SchematicSheet>> = run
        .open
        .iter()
        .map(|(path, sheet)| (path.clone(), Arc::new(sheet.clone())))
        .collect();
    if let Some((path, snapshot)) = &run.active {
        read.insert(path.clone(), Arc::new(snapshot.clone()));
    }
    let scopes: HashMap<PathBuf, EditScope> = read
        .iter()
        .map(|(path, sheet)| {
            let before = baselines.get(path).cloned().flatten();
            (path.clone(), edit_scope(before.as_deref(), sheet))
        })
        .collect();
    let changed_names = scopes
        .values()
        .flat_map(|scope| scope.net_names.iter().cloned())
        .collect();
    let (outcome, _net_waived) = run_erc_check_job(run, None);
    LiveErcOutcome {
        by_path: outcome.by_path,
        scopes,
        changed_names,
        read,
        page_frames: outcome.page_frames,
    }
}
