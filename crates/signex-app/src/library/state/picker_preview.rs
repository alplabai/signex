//! The primitive picker's highlighted row and its preview (mockup row 131):
//! a row click previews, the Pick button confirms (decided 2026-11-22).

use std::sync::Arc;

use signex_library::PrimitiveRef;
use signex_types::schematic::LibSymbol;

use crate::library::editor::footprint::canvas::PreviewFootprint;
use crate::library::loose_symbols::LooseSymbol;

/// What a picker row stands for, and what Pick does with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrimitiveChoice {
    /// Bind a primitive of an open library.
    Pick(PrimitiveRef),
    /// Ask to copy (or, for a library that is not open, link) the symbol.
    Loose(LooseSymbol),
}

/// A primitive read for the preview column.
#[derive(Debug, Clone)]
pub enum PrimitiveDrawing {
    Symbol {
        symbol: Box<LibSymbol>,
        pins: usize,
        parts: u8,
    },
    Footprint(Box<PreviewFootprint>),
}

/// The highlighted row and the newest read of its drawing.
#[derive(Debug, Clone, Default)]
pub struct PickerSelection {
    pub highlighted: Option<PrimitiveChoice>,
    /// Tags the newest read, so a slower older one is dropped.
    pub generation: u64,
    pub loading: bool,
    pub drawing: Option<Arc<PrimitiveDrawing>>,
}

impl PickerSelection {
    /// Highlight `choice` and start a new read; returns the read's tag.
    pub fn highlight(&mut self, choice: PrimitiveChoice) -> u64 {
        self.highlighted = Some(choice);
        self.generation += 1;
        self.loading = true;
        self.drawing = None;
        self.generation
    }

    /// A read finished. Only the newest counts; `None` leaves no drawing,
    /// which the column shows as unreadable.
    pub fn finish(&mut self, generation: u64, drawing: Option<Arc<PrimitiveDrawing>>) {
        if generation == self.generation {
            self.loading = false;
            self.drawing = drawing;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn choice(n: u128) -> PrimitiveChoice {
        PrimitiveChoice::Pick(PrimitiveRef::new(
            uuid::Uuid::nil(),
            uuid::Uuid::from_u128(n),
        ))
    }

    fn footprint() -> Arc<PrimitiveDrawing> {
        let model = PreviewFootprint::build(&signex_library::Footprint::empty("R"));
        Arc::new(PrimitiveDrawing::Footprint(Box::new(model)))
    }

    /// A slow read of the previous row never lands on the new one.
    #[test]
    fn only_the_newest_read_is_kept() {
        let mut selection = PickerSelection::default();
        let first = selection.highlight(choice(1));
        let second = selection.highlight(choice(2));
        selection.finish(first, Some(footprint()));
        assert!(selection.loading && selection.drawing.is_none());
        selection.finish(second, Some(footprint()));
        assert!(!selection.loading && selection.drawing.is_some());
    }
}
