use std::collections::{BTreeSet, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use signex_engine::{FindHit, FindQuery, MatchOptions, PatternError};
use signex_types::schematic::SelectedItem;

/// One result: where it is (`sheet`, plus the hit's placement and position)
/// and what the scan saw there. The hit's text is for display; a replace
/// re-reads the live document.
#[derive(Debug, Clone)]
pub struct FindMatch {
    /// The sheet file the hit is on.
    pub sheet: Option<PathBuf>,
    pub hit: FindHit,
    /// Which placement of a sheet placed more than once a designator hit was
    /// read at ("placement 2 of 3"); `None` otherwise.
    pub placement_label: Option<String>,
}

/// How far a search reaches.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FindScope {
    /// The active sheet only.
    #[default]
    Sheet,
    /// Every sheet of the project, open or not.
    Project,
}

/// What a project Replace needs from the disk, read off the UI thread: the
/// sheets no engine holds, and a hierarchy built just now, so the placements
/// a designator edit targets are not the Navigator's possibly stale copy.
#[derive(Debug)]
pub struct ReplacePrep {
    pub hierarchy: Option<crate::app::sheet_hierarchy::SheetHierarchy>,
    /// The wanted sheets that had no open engine, parsed from disk.
    pub loaded: std::collections::HashMap<PathBuf, signex_types::schematic::SchematicSheet>,
    pub unreadable: Vec<(PathBuf, String)>,
}

/// Every open sheet's child-sheet placements `(uuid, name, filename)`, in
/// document order. They decide which instance paths a designator is read at,
/// hence its `single` flag; none of them moves `documents_epoch`.
pub type ChildSheetSignature =
    std::collections::BTreeMap<PathBuf, Vec<(uuid::Uuid, String, String)>>;

/// A project Replace waiting for [`ReplacePrep`].
#[derive(Debug, Clone)]
pub struct ReplaceJob {
    /// The scan generation it was started under; a search change cancels it.
    pub generation: u64,
    /// `DocumentState::documents_epoch` when the disk was read. If a document
    /// was opened, saved or closed since, the whole replace is refused: what
    /// was read may no longer be what the sheets hold.
    pub epoch: u64,
    /// The open sheets' [`ChildSheetSignature`] when the hierarchy was
    /// captured. If it differs at apply time the replace is refused: a sheet
    /// symbol was placed, removed, renamed or re-pointed mid-read.
    pub child_sheets: ChildSheetSignature,
    /// The query and its match options as they were when Replace was asked.
    pub find: FindQuery,
    pub replacement: String,
    /// The included results the user was looking at: the only hits it may
    /// replace. A row whose include box was cleared is not in it (#2525).
    pub matches: Vec<FindMatch>,
}

/// Identity of a result across a rescan: its sheet, the item and the
/// placement a designator was read at. The include set is keyed by it, so a
/// row cleared before a Replace Current stays cleared after the refresh.
pub type ResultKey = (Option<PathBuf>, SelectedItem, Option<String>);

impl FindMatch {
    /// See [`ResultKey`].
    pub fn key(&self) -> ResultKey {
        (
            self.sheet.clone(),
            self.hit.item,
            self.hit.placement.as_ref().map(ToString::to_string),
        )
    }
}

/// What a project scan found, built off the UI thread.
#[derive(Debug, Default)]
pub struct ProjectScan {
    /// Hits grouped by sheet, in sorted-path order.
    pub matches: Vec<FindMatch>,
    /// Sheets that exist but could not be read, with the reason.
    pub unreadable: Vec<(PathBuf, String)>,
    /// Sheets written by a newer Signex: held read-only, so not searched.
    pub read_only: Vec<PathBuf>,
}

#[derive(Debug, Clone, Default)]
pub struct FindReplaceState {
    pub open: bool,
    pub replace_mode: bool,
    pub query: String,
    pub replacement: String,
    /// Match case / Whole word / Regex (#2525). Whole word is on by default,
    /// so a net name never matches inside a longer one.
    pub options: MatchOptions,
    /// Results whose include box is cleared: Replace All leaves them alone.
    /// Every result is included by default; a new query, options or scope
    /// clears the set (#2525).
    pub excluded: HashSet<ResultKey>,
    pub matches: Vec<FindMatch>,
    pub selected_index: Option<usize>,
    pub scope: FindScope,
    /// Bumped for every project scan (and every cancelling change); a scan
    /// result carrying an older number is dropped.
    pub scan_generation: u64,
    pub scanning: bool,
    /// Sheets already reported to the Messages panel as skipped, so typing
    /// does not repeat the warning on every keystroke.
    pub skips_reported: BTreeSet<PathBuf>,
    /// One line for the dialog when sheets were skipped.
    pub skipped_note: Option<String>,
    pub replace_job: Option<ReplaceJob>,
    /// The undocked window whose sheet the dialog searches and selects in
    /// (`None`: the main window's). Set when the dialog opens (#2037).
    pub window: Option<iced::window::Id>,
    /// The search ran on a board (#2403): its hits, which take the place of
    /// `matches`; `None` on a sheet.
    pub board: Option<BoardFind>,
}

/// What a search found on a board (#2403).
#[derive(Debug, Clone, Default)]
pub struct BoardFind {
    pub path: PathBuf,
    pub hits: Vec<signex_engine::pcb::find::BoardFindHit>,
}

impl FindReplaceState {
    /// How many results the dialog lists: the board's hits on a board.
    pub fn result_count(&self) -> usize {
        self.board
            .as_ref()
            .map_or(self.matches.len(), |board| board.hits.len())
    }

    /// The query compiled under the options. An invalid regex is the error
    /// the Find field shows; it matches nothing.
    pub fn find_query(&self) -> Result<FindQuery, PatternError> {
        FindQuery::new(&self.query, self.options)
    }

    /// Whether result `index` is included in Replace All. A board's hits
    /// have no include box, so they always are.
    pub fn is_included(&self, index: usize) -> bool {
        self.board.is_some()
            || self
                .matches
                .get(index)
                .is_some_and(|hit| !self.excluded.contains(&hit.key()))
    }

    /// How many results Replace All acts on.
    pub fn included_count(&self) -> usize {
        if self.board.is_some() {
            return self.result_count();
        }
        (0..self.matches.len())
            .filter(|&index| self.is_included(index))
            .count()
    }

    /// The results Replace All acts on, in list order.
    pub fn included_matches(&self) -> Vec<FindMatch> {
        self.matches
            .iter()
            .filter(|hit| !self.excluded.contains(&hit.key()))
            .cloned()
            .collect()
    }

    /// Clear or set the include box of result `index`.
    pub fn toggle_included(&mut self, index: usize) {
        let Some(key) = self.matches.get(index).map(FindMatch::key) else {
            return;
        };
        if !self.excluded.remove(&key) {
            self.excluded.insert(key);
        }
    }
}

#[derive(Debug, Clone)]
pub enum FindReplaceMsg {
    Close,
    QueryChanged(String),
    ReplacementChanged(String),
    SelectResult(usize),
    ReplaceCurrent,
    ReplaceAll,
    ScopeChanged(FindScope),
    /// The header's Find / Replace switch was set (`true`: Replace).
    ModeChanged(bool),
    MatchCaseToggled,
    WholeWordToggled,
    RegexToggled,
    /// The include box of result `index` was clicked.
    ResultIncludeToggled(usize),
    /// Find next (F3): step to the result after the current one.
    NextRequested,
    /// Find previous (Shift+F3).
    PreviousRequested,
    /// The disk half of a project Replace came back; `generation` is the job
    /// it answers. Anything but the live job's generation is ignored.
    ProjectReplacePrepared {
        generation: u64,
        result: Arc<Result<ReplacePrep, String>>,
    },
    /// A project scan came back. `generation` is the scan it answers.
    ProjectScanFinished {
        generation: u64,
        result: Arc<Result<ProjectScan, String>>,
    },
}

impl FindReplaceState {
    /// A project scan or replace is waiting on a background job, so the
    /// results are not yet (or no longer) the ones to act on.
    pub fn is_busy(&self) -> bool {
        self.scanning || self.replace_job.is_some()
    }

    /// Invalidate every job in flight: the next result carrying an older
    /// generation is dropped. Returns the replace that was waiting, if any.
    pub fn supersede(&mut self) -> Option<ReplaceJob> {
        self.scan_generation += 1;
        self.scanning = false;
        self.replace_job.take()
    }
}
