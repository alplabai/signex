use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use signex_engine::{SimilarProperty, SimilarValue};

/// How far a Find Similar search reaches.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SimilarScope {
    /// The active sheet only.
    #[default]
    Sheet,
    /// Every sheet of the project, open or not.
    Project,
}

/// The engine's kind name of a placed part; the dialog says [`PART_NOUN`].
const ENGINE_PART_KIND: &str = "Symbol";
/// The one user-facing noun for a placed part.
const PART_NOUN: &str = "Part";

/// The kind name the dialog shows for an engine kind value: a placed part is
/// a Part; every other kind reads as the engine names it. Matching still
/// compares the engine's values.
pub fn shown_kind(kind: &str) -> &str {
    if kind == ENGINE_PART_KIND {
        PART_NOUN
    } else {
        kind
    }
}

/// The seed line for an engine description (`Symbol R1`): its leading kind
/// word as the dialog shows it (`Part R1`).
pub fn shown_seed(described: &str) -> String {
    match described.split_once(' ') {
        Some((kind, rest)) => format!("{} {rest}", shown_kind(kind)),
        None => shown_kind(described).to_owned(),
    }
}

/// The open dialog: the seed's properties, which must match, and where to look.
#[derive(Debug, Clone)]
pub struct FindSimilarState {
    /// `Part R1`: what the seed is, for the header's subtitle.
    pub seed_label: String,
    /// The seed's properties, `Kind` first.
    pub values: Vec<SimilarValue>,
    /// The properties set to Same; every other one is Any.
    pub same: BTreeSet<SimilarProperty>,
    pub scope: SimilarScope,
    /// A project count is running; Select waits for it.
    pub running: bool,
    /// The sheet the dialog was opened on; a search answers for it only.
    pub origin: Option<std::path::PathBuf>,
    /// Opened on a board (#2403): `origin` is the board, and the search
    /// reaches that board only.
    pub board: bool,
    /// How many objects the current criteria match on the dialog's sheet
    /// (or board): what Select selects.
    pub sheet_matches: usize,
    /// The last project count for the current criteria; `None` until one
    /// landed, and again once the criteria or the scope changed.
    pub other_sheets: Option<ProjectMatches>,
}

impl FindSimilarState {
    /// A fresh dialog for a seed: only `Kind` is Same, as the rest start Any.
    /// The seed label is kept with the dialog's kind names.
    pub fn new(
        seed_label: String,
        values: Vec<SimilarValue>,
        origin: Option<std::path::PathBuf>,
    ) -> Self {
        Self {
            seed_label: shown_seed(&seed_label),
            values,
            same: BTreeSet::from([SimilarProperty::Kind]),
            scope: SimilarScope::Sheet,
            running: false,
            origin,
            board: false,
            sheet_matches: 0,
            other_sheets: None,
        }
    }

    /// The dialog opened for a seed on the board at `origin`.
    pub fn on_board(self) -> Self {
        Self {
            board: true,
            ..self
        }
    }

    /// The seed's values for the properties set to Same.
    pub fn criteria(&self) -> Vec<SimilarValue> {
        self.values
            .iter()
            .filter(|v| self.same.contains(&v.property))
            .cloned()
            .collect()
    }

    /// The matches on the project's other sheets, once a count landed.
    pub fn other_sheet_total(&self) -> Option<usize> {
        self.other_sheets
            .as_ref()
            .map(|found| found.elsewhere.iter().map(|(_, n)| n).sum())
    }
}

/// What a project-wide search found.
#[derive(Debug, Default, Clone)]
pub struct ProjectMatches {
    /// Match counts on every other sheet that has any, in project order.
    pub elsewhere: Vec<(PathBuf, usize)>,
    /// Sheets that exist but could not be read, with why.
    pub unreadable: Vec<(PathBuf, String)>,
    /// How many sheets were searched, the open one included.
    pub sheets_searched: usize,
}

#[derive(Debug, Clone)]
pub enum FindSimilarMsg {
    /// Open the dialog for the object at world `at` (the cursor, or where the
    /// context menu was opened), else for the single selected object.
    OpenRequested {
        at: Option<(f64, f64)>,
    },
    /// A property was set to Same (`true`) or Any (`false`). Idempotent:
    /// clicking the choice already made changes nothing.
    PropertySet(SimilarProperty, bool),
    ScopeChosen(SimilarScope),
    /// Select N: select every match on the dialog's sheet.
    ApplyPressed,
    Closed,
    /// A project count finished.
    ProjectSearched {
        generation: u64,
        result: Arc<Result<ProjectMatches, String>>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_placed_part_reads_as_a_part_and_other_kinds_as_named() {
        assert_eq!(shown_kind("Symbol"), "Part");
        assert_eq!(shown_kind("Wire"), "Wire");
        assert_eq!(shown_seed("Symbol R3"), "Part R3");
        assert_eq!(shown_seed("Symbol"), "Part");
        assert_eq!(shown_seed("Label VCC"), "Label VCC");
        assert_eq!(shown_seed("Bus entry"), "Bus entry");
    }

    #[test]
    fn the_other_sheet_total_sums_every_sheet() {
        let mut state = FindSimilarState::new("Wire".into(), Vec::new(), None);
        assert_eq!(state.other_sheet_total(), None);
        state.other_sheets = Some(ProjectMatches {
            elsewhere: vec![(PathBuf::from("a"), 5), (PathBuf::from("b"), 7)],
            ..ProjectMatches::default()
        });
        assert_eq!(state.other_sheet_total(), Some(12));
    }
}
