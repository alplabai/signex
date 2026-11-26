//! What a generation left out, so the dialog can say it rather than hand
//! over files that silently miss copper.

use pcb_geom::DrawnCopperFault;
use signex_types::pcb_graphic::BoardGraphicKind;
use signex_types::pcb_layer::CopperLayer;
use uuid::Uuid;

/// Something the fabrication files do not hold. The app words it; this
/// crate only counts and names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FabNote {
    /// Copper zones whose fill was not supplied (zone filling, #2216). An
    /// outline is never written in place of a fill: it would short every
    /// other net it crosses.
    ZonesNotFilled { count: usize },
    /// Placed parts the BOM lists that have no designator, so the
    /// pick-and-place file has no row for them.
    PartsWithoutDesignator { count: usize },
    /// Items with a coordinate or size the geometry cannot hold (not
    /// finite, or beyond 1 km), or a graphic missing its points.
    ItemsSkipped { count: usize },
    /// A board drawing or string on copper that the shared conversion
    /// (`pcb_geom::board_graphic_copper` / `board_text_copper`) refuses
    /// (#2883): the design rule check sees no copper there, so the copper
    /// images hold none either.
    DrawnCopperSkipped {
        item: DrawnCopperItem,
        layer: CopperLayer,
        fault: DrawnCopperFault,
    },
}

/// A board drawing or string, by identity, as a note names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawnCopperItem {
    /// A board drawing; `None` for a kind this build does not know.
    Drawing {
        id: Uuid,
        kind: Option<BoardGraphicKind>,
    },
    /// A board string.
    Text { id: Uuid },
}

/// The running counts behind the notes.
#[derive(Debug, Default)]
pub(crate) struct Tally {
    pub unnamed_parts: usize,
    pub skipped: usize,
    /// Every [`FabNote::DrawnCopperSkipped`], in board order, each once.
    pub drawn_copper_skipped: Vec<FabNote>,
}

impl Tally {
    /// The notes, in a fixed order, leaving out every zero count; the
    /// drawn copper left out follows, one note per item.
    pub(crate) fn notes(&self, zones_not_filled: usize) -> Vec<FabNote> {
        [
            FabNote::ZonesNotFilled {
                count: zones_not_filled,
            },
            FabNote::PartsWithoutDesignator {
                count: self.unnamed_parts,
            },
            FabNote::ItemsSkipped {
                count: self.skipped,
            },
        ]
        .into_iter()
        .filter(|note| note.count() > 0)
        .chain(self.drawn_copper_skipped.iter().copied())
        .collect()
    }

    /// Note that `item` on `layer` was left out of the copper for `fault`.
    pub(crate) fn skip_drawn_copper(
        &mut self,
        item: DrawnCopperItem,
        layer: CopperLayer,
        fault: DrawnCopperFault,
    ) {
        self.note_drawn_copper(FabNote::DrawnCopperSkipped { item, layer, fault });
    }

    /// Take the drawn copper notes of another output's tally: the copper
    /// images of every output skip the same items, named once.
    pub(crate) fn merge_drawn_copper(&mut self, other: &Tally) {
        for note in &other.drawn_copper_skipped {
            self.note_drawn_copper(*note);
        }
    }

    fn note_drawn_copper(&mut self, note: FabNote) {
        if !self.drawn_copper_skipped.contains(&note) {
            self.drawn_copper_skipped.push(note);
        }
    }
}

impl FabNote {
    /// How many items the note is about: one for a named item.
    pub fn count(self) -> usize {
        match self {
            FabNote::ZonesNotFilled { count }
            | FabNote::PartsWithoutDesignator { count }
            | FabNote::ItemsSkipped { count } => count,
            FabNote::DrawnCopperSkipped { .. } => 1,
        }
    }
}
