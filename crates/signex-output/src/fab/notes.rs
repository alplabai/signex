//! What a generation left out, so the dialog can say it rather than hand
//! over files that silently miss copper.

/// Something the fabrication files do not hold. The app words it; this
/// crate only counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FabNote {
    /// Copper zones whose fill was not supplied (zone filling, #2216). An
    /// outline is never written in place of a fill: it would short every
    /// other net it crosses.
    ZonesNotFilled { count: usize },
    /// Texts on a fabrication layer, left for the stroke font of board text
    /// drawing (#2387).
    TextsNotDrawn { count: usize },
    /// V-score lines, which no file written here carries.
    VScoresNotWritten { count: usize },
    /// Items with a coordinate or size the geometry cannot hold (not
    /// finite, or beyond 1 km), or a graphic missing its points.
    ItemsSkipped { count: usize },
}

/// The running counts behind the notes.
#[derive(Debug, Default)]
pub(crate) struct Tally {
    pub texts: usize,
    pub skipped: usize,
}

impl Tally {
    /// The notes, in a fixed order, leaving out every zero count.
    pub(crate) fn notes(&self, zones_not_filled: usize, v_scores: usize) -> Vec<FabNote> {
        [
            FabNote::ZonesNotFilled {
                count: zones_not_filled,
            },
            FabNote::TextsNotDrawn { count: self.texts },
            FabNote::VScoresNotWritten { count: v_scores },
            FabNote::ItemsSkipped {
                count: self.skipped,
            },
        ]
        .into_iter()
        .filter(|note| note.count() > 0)
        .collect()
    }
}

impl FabNote {
    pub fn count(self) -> usize {
        match self {
            FabNote::ZonesNotFilled { count }
            | FabNote::TextsNotDrawn { count }
            | FabNote::VScoresNotWritten { count }
            | FabNote::ItemsSkipped { count } => count,
        }
    }
}
