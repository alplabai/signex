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

/// The open dialog: the seed's properties, which must match, and where to look.
#[derive(Debug, Clone)]
pub struct FindSimilarState {
    /// `Symbol R1`: what the seed is, for the dialog's title line.
    pub seed_label: String,
    /// The seed's properties, `Kind` first.
    pub values: Vec<SimilarValue>,
    /// The properties set to Same; every other one is Any.
    pub same: BTreeSet<SimilarProperty>,
    pub scope: SimilarScope,
    /// A project search is running; OK waits for it.
    pub running: bool,
    /// The sheet the dialog was opened on; a search answers for it only.
    pub origin: Option<std::path::PathBuf>,
}

impl FindSimilarState {
    /// A fresh dialog for a seed: only `Kind` is Same, as the rest start Any.
    pub fn new(
        seed_label: String,
        values: Vec<SimilarValue>,
        origin: Option<std::path::PathBuf>,
    ) -> Self {
        Self {
            seed_label,
            values,
            same: BTreeSet::from([SimilarProperty::Kind]),
            scope: SimilarScope::Sheet,
            running: false,
            origin,
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
}

/// What a project-wide search found.
#[derive(Debug, Default)]
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
    /// A property's Same / Any choice was flipped.
    /// A property was set to Same (`true`) or Any (`false`). Idempotent:
    /// clicking the choice already made changes nothing.
    PropertySet(SimilarProperty, bool),
    ScopeChosen(SimilarScope),
    /// OK: select every match.
    ApplyPressed,
    Closed,
    /// A project search finished.
    ProjectSearched {
        generation: u64,
        result: Arc<Result<ProjectMatches, String>>,
    },
}
