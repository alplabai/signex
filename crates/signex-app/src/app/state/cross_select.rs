//! Cross Select Mode and a pending Cross Probe (#1312).
//!
//! Cross select compares a fingerprint of each window's selection after every
//! update. A surface whose selection changed on the same document since the
//! last look is the one the user acted on; its counterpart is selected on the
//! other side, and the fingerprints are taken again so that write is never
//! read back as a new change (no echo).

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::PathBuf;

use signex_net::cross_probe::PartOccurrence;

/// One selection surface: a schematic or board canvas of a window (`None`:
/// the main window).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SelectSurface {
    Schematic(Option<iced::window::Id>),
    Board(Option<iced::window::Id>),
}

impl SelectSurface {
    pub fn is_schematic(self) -> bool {
        matches!(self, Self::Schematic(_))
    }
}

/// What a surface shows and a fingerprint of its selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurfaceStamp {
    pub document: PathBuf,
    pub selection: u64,
    pub is_empty: bool,
}

impl SurfaceStamp {
    pub fn new<T: Hash>(document: PathBuf, items: impl IntoIterator<Item = T>) -> Self {
        let mut hasher = DefaultHasher::new();
        let mut count = 0usize;
        for item in items {
            item.hash(&mut hasher);
            count += 1;
        }
        count.hash(&mut hasher);
        Self {
            document,
            selection: hasher.finish(),
            is_empty: count == 0,
        }
    }
}

/// What a selection stands for on the other side: parts by sheet occurrence
/// and unit, nets by name.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CrossSelection {
    pub parts: Vec<PartOccurrence>,
    pub nets: Vec<String>,
}

impl CrossSelection {
    pub fn is_empty(&self) -> bool {
        self.parts.is_empty() && self.nets.is_empty()
    }
}

/// A Cross Probe waiting for its target document to be shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingProbe {
    pub what: CrossSelection,
    /// The probe goes to the board (from a sheet) or to a sheet (from a board).
    pub to_board: bool,
    /// The document the probe started from; its project names the target.
    pub from: PathBuf,
    /// The document being opened for it, once an open was started.
    pub opening: Option<PathBuf>,
}

/// Cross Select Mode, the fingerprints it compares, and a pending probe.
#[derive(Debug)]
pub struct CrossSelectState {
    /// On by default (maintainer decision, `docs/DECISIONS.md`).
    pub enabled: bool,
    seen: HashMap<SelectSurface, SurfaceStamp>,
    pub pending: Option<PendingProbe>,
}

impl Default for CrossSelectState {
    fn default() -> Self {
        Self {
            enabled: true,
            seen: HashMap::new(),
            pending: None,
        }
    }
}

impl CrossSelectState {
    /// The first surface (in `now` order) whose selection changed on the same
    /// document since the last [`Self::record`]. A surface that now shows
    /// another document, or that was not seen before, did not change.
    pub fn changed(&self, now: &[(SelectSurface, SurfaceStamp)]) -> Option<SelectSurface> {
        now.iter()
            .find(|(surface, stamp)| {
                self.seen.get(surface).is_some_and(|before| {
                    before.document == stamp.document && before.selection != stamp.selection
                })
            })
            .map(|(surface, _)| *surface)
    }

    /// Remember `now` as what every surface shows; a surface not in it is gone.
    pub fn record(&mut self, now: Vec<(SelectSurface, SurfaceStamp)>) {
        self.seen = now.into_iter().collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stamp(doc: &str, items: &[u32]) -> SurfaceStamp {
        SurfaceStamp::new(PathBuf::from(doc), items.iter().copied())
    }

    #[test]
    fn a_new_selection_on_the_same_document_is_a_change() {
        let mut state = CrossSelectState::default();
        let sheet = SelectSurface::Schematic(None);
        state.record(vec![(sheet, stamp("a", &[]))]);
        assert_eq!(state.changed(&[(sheet, stamp("a", &[1]))]), Some(sheet));
    }

    #[test]
    fn switching_documents_or_a_new_surface_is_not_a_change() {
        let mut state = CrossSelectState::default();
        let sheet = SelectSurface::Schematic(None);
        let board = SelectSurface::Board(None);
        state.record(vec![(sheet, stamp("a", &[1]))]);
        assert_eq!(state.changed(&[(sheet, stamp("b", &[2]))]), None);
        assert_eq!(state.changed(&[(board, stamp("p", &[2]))]), None);
    }

    #[test]
    fn what_was_recorded_is_not_a_change_again() {
        let mut state = CrossSelectState::default();
        let board = SelectSurface::Board(None);
        state.record(vec![(board, stamp("p", &[1]))]);
        state.record(vec![(board, stamp("p", &[2]))]);
        assert_eq!(state.changed(&[(board, stamp("p", &[2]))]), None);
    }

    #[test]
    fn cross_select_is_on_by_default() {
        assert!(CrossSelectState::default().enabled);
    }
}
