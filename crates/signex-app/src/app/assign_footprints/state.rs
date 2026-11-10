//! The dialog's draft: the placed parts as read and the footprints picked for
//! them. Pure data; which footprints a part may take comes from its own
//! library row (`footprint_choices_for`, the list Properties shows).

use std::collections::BTreeSet;
use std::path::PathBuf;

use uuid::Uuid;

/// One footprint a part's library row offers: the default or an alternate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FootprintOffer {
    /// `"<library uuid>:<footprint uuid>"`, as Properties sends it.
    pub binding: String,
    /// The footprint's name.
    pub label: String,
}

/// Whether a part's footprint can be set here, and if not, why.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartAccess {
    /// On an open sheet and its row offers more than one footprint.
    Editable,
    /// Its sheet is not open: listed for the overview, changed by opening it.
    SheetClosed,
    /// Its sheet was written by a newer Signex and opens read-only.
    ReadOnly,
    /// Its library row is not open, or it has no row (a hand-made part).
    NoRow,
    /// Its row offers one footprint only.
    NoAlternates,
}

/// One placed part, as the table lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FootprintPart {
    pub sheet: PathBuf,
    /// The sheet as the user knows it (relative to the project folder).
    pub sheet_label: String,
    pub symbol_id: Uuid,
    pub designator: String,
    pub value: String,
    /// The library row's part number; `None` with no open row.
    pub row_label: Option<String>,
    /// `(library id, row id)`, so rows of one library row can be selected
    /// together.
    pub row_key: Option<(Uuid, Uuid)>,
    /// The footprint text the part holds today.
    pub current: String,
    /// The offer the part is built with today; `None` when its footprint
    /// text was typed over or its binding is no longer offered.
    pub current_binding: Option<String>,
    pub offers: Vec<FootprintOffer>,
    pub access: PartAccess,
}

/// The Assign Footprints dialog, while open. Nothing changes until Apply,
/// and Apply changes the open sheets in memory only.
#[derive(Debug, Clone)]
pub struct AssignFootprintsState {
    /// The read this dialog waits for or shows.
    pub(crate) generation: u64,
    /// `documents_epoch` when the read started: an engine opened or dropped
    /// since then makes the read stale.
    pub(crate) epoch: u64,
    /// `None` while the read runs.
    pub(crate) parts: Option<Vec<FootprintPart>>,
    /// The footprint picked for each part, by index; `None` keeps it.
    pub(crate) picks: Vec<Option<String>>,
    /// The ticked rows, for setting many at once.
    pub(crate) selected: BTreeSet<usize>,
    /// Why the read failed or Apply refused, shown in the dialog.
    pub(crate) error: Option<AssignProblem>,
}

/// Why the read failed or Apply refused. The dialog words it through the
/// catalog; the Messages panel line is [`Self::english`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssignProblem {
    /// The read worker itself failed; the text is the system's reason.
    ReadFailed(String),
    /// A sheet with a pick was closed after the read.
    SheetClosed(PathBuf),
    /// A sheet with a pick opens read-only (written by a newer Signex).
    ReadOnly(PathBuf),
    /// A part can no longer take its pick (gone, or its row changed); the
    /// part's designator and the reason in English.
    Refused { reference: String, why: String },
    /// A sheet refused the change; the sheets already changed were put back.
    NotChanged(PathBuf),
}

impl AssignProblem {
    /// The reason in English, for the Messages panel.
    pub(crate) fn english(&self) -> String {
        match self {
            Self::ReadFailed(why) => format!("the sheets could not be read: {why}"),
            Self::SheetClosed(path) => format!(
                "{} was closed after the dialog read it; nothing was changed",
                path.display()
            ),
            Self::ReadOnly(path) => format!(
                "{} is read-only (written by a newer Signex); nothing was changed",
                path.display()
            ),
            Self::Refused { reference, why } => {
                format!("{reference} could not take the footprint, {why}; nothing was changed")
            }
            Self::NotChanged(path) => format!(
                "{} refused the change; the sheets already changed were put back",
                path.display()
            ),
        }
    }
}

impl AssignFootprintsState {
    pub(crate) fn reading(generation: u64, epoch: u64) -> Self {
        Self {
            generation,
            epoch,
            parts: None,
            picks: Vec::new(),
            selected: BTreeSet::new(),
            error: None,
        }
    }

    /// The read arrived: every part starts at the footprint it has today.
    pub(crate) fn show(&mut self, parts: Vec<FootprintPart>) {
        self.picks = vec![None; parts.len()];
        self.parts = Some(parts);
        self.selected.clear();
        self.error = None;
    }

    pub fn parts(&self) -> &[FootprintPart] {
        self.parts.as_deref().unwrap_or(&[])
    }

    /// Why the read failed or Apply refused, while the dialog shows it.
    pub fn problem(&self) -> Option<&AssignProblem> {
        self.error.as_ref()
    }

    /// The footprint row `index` will have after Apply: the pick, else what
    /// it is built with today.
    pub fn shown_binding(&self, index: usize) -> Option<&str> {
        self.picks
            .get(index)
            .and_then(Option::as_deref)
            .or_else(|| self.parts().get(index)?.current_binding.as_deref())
    }

    /// Pick `binding` for row `index`. Only an editable part takes it, and
    /// only a footprint its own row offers; picking what it has today drops
    /// the pick.
    pub(crate) fn pick(&mut self, index: usize, binding: &str) {
        let Some(part) = self.parts().get(index) else {
            return;
        };
        if part.access != PartAccess::Editable || !part.offers.iter().any(|o| o.binding == binding)
        {
            return;
        }
        let keep = part.current_binding.as_deref() == Some(binding);
        if let Some(slot) = self.picks.get_mut(index) {
            *slot = (!keep).then(|| binding.to_string());
        }
    }

    /// Pick `binding` for every ticked row whose row offers it.
    pub(crate) fn pick_selected(&mut self, binding: &str) {
        let rows: Vec<usize> = self.selected.iter().copied().collect();
        for index in rows {
            self.pick(index, binding);
        }
    }

    /// Tick or untick row `index`; only an editable part can be ticked.
    pub(crate) fn toggle(&mut self, index: usize) {
        if self.parts().get(index).map(|p| p.access) != Some(PartAccess::Editable) {
            return;
        }
        if !self.selected.remove(&index) {
            self.selected.insert(index);
        }
    }

    /// Tick every editable part, or none.
    pub(crate) fn select_all(&mut self, on: bool) {
        self.selected = if on {
            self.editable_rows().collect()
        } else {
            BTreeSet::new()
        };
    }

    /// Tick every editable part of the same library row as row `index`.
    pub(crate) fn select_same_row(&mut self, index: usize) {
        let Some(key) = self.parts().get(index).and_then(|p| p.row_key) else {
            return;
        };
        let same: Vec<usize> = self
            .editable_rows()
            .filter(|&i| self.parts()[i].row_key == Some(key))
            .collect();
        self.selected.extend(same);
    }

    fn editable_rows(&self) -> impl Iterator<Item = usize> + '_ {
        self.parts()
            .iter()
            .enumerate()
            .filter(|(_, p)| p.access == PartAccess::Editable)
            .map(|(i, _)| i)
    }

    /// Whether every editable part is ticked (and there is one).
    pub fn all_selected(&self) -> bool {
        let editable: Vec<usize> = self.editable_rows().collect();
        !editable.is_empty() && editable.iter().all(|i| self.selected.contains(i))
    }

    /// The footprints every ticked part's row offers, in the first ticked
    /// part's order: what "Set ticked to" can pick. Empty with nothing ticked.
    pub fn shared_offers(&self) -> Vec<FootprintOffer> {
        let mut ticked = self.selected.iter().filter_map(|&i| self.parts().get(i));
        let Some(first) = ticked.next() else {
            return Vec::new();
        };
        let rest: Vec<&FootprintPart> = ticked.collect();
        first
            .offers
            .iter()
            .filter(|offer| {
                rest.iter()
                    .all(|p| p.offers.iter().any(|o| o.binding == offer.binding))
            })
            .cloned()
            .collect()
    }

    /// The parts Apply changes and the footprint each gets.
    pub fn changes(&self) -> Vec<(usize, String)> {
        self.picks
            .iter()
            .enumerate()
            .filter_map(|(i, pick)| Some((i, pick.clone()?)))
            .collect()
    }

    /// Apply is offered: the read is in and a footprint changes.
    pub fn can_apply(&self) -> bool {
        self.parts.is_some() && self.picks.iter().any(Option::is_some)
    }
}
