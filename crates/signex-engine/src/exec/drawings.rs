//! `Command::ArrangeDrawings` and `Command::SetDrawingPlacement` (#1583).

use crate::*;
use signex_types::schematic::{DrawingPlacement, SchDrawing};
use std::collections::HashSet;
use uuid::Uuid;

/// Reorder `drawings` so the ones in `selected` move per `how`; `true` when
/// the list changed. The selected drawings keep their order among themselves,
/// whatever order `selected` was built in.
pub(crate) fn arrange_drawings(
    drawings: &mut [SchDrawing],
    selected: &HashSet<Uuid>,
    how: DrawingArrange,
) -> bool {
    let is_selected = |d: &SchDrawing| selected.contains(&d.uuid());
    let before: Vec<Uuid> = drawings.iter().map(SchDrawing::uuid).collect();
    match how {
        DrawingArrange::ToFront => drawings.sort_by_key(is_selected),
        DrawingArrange::ToBack => drawings.sort_by_key(|d| !is_selected(d)),
        DrawingArrange::Forward => {
            for i in (0..drawings.len().saturating_sub(1)).rev() {
                if is_selected(&drawings[i]) && !is_selected(&drawings[i + 1]) {
                    drawings.swap(i, i + 1);
                }
            }
        }
        DrawingArrange::Backward => {
            for i in 1..drawings.len() {
                if is_selected(&drawings[i]) && !is_selected(&drawings[i - 1]) {
                    drawings.swap(i, i - 1);
                }
            }
        }
    }
    drawings.iter().map(SchDrawing::uuid).ne(before)
}

impl Engine {
    pub(crate) fn exec_drawings(
        &mut self,
        before: SchematicSheet,
        cmd: Command,
    ) -> Result<CommandResult, EngineError> {
        let changed = match cmd {
            Command::ArrangeDrawings { ids, arrange } => {
                let selected: HashSet<Uuid> = ids.into_iter().collect();
                arrange_drawings(&mut self.document.drawings, &selected, arrange)
            }
            Command::SetDrawingPlacement { ids, placement } => {
                self.set_drawing_placement(&ids, placement)
            }
            _ => unreachable!("exec_drawings routes only the drawing commands"),
        };
        if !changed {
            return Ok(CommandResult::unchanged());
        }
        let patch_pair = PatchPair {
            semantic: SemanticPatch::DrawingMutated,
            document: DocumentPatch::DRAWINGS,
        };
        self.record_history(before, patch_pair);
        Ok(CommandResult::changed(patch_pair))
    }

    /// Set the placement of every existing drawing in `ids`; `true` when one
    /// changed. Behind is the absence of an entry, so it removes it.
    fn set_drawing_placement(&mut self, ids: &[Uuid], placement: DrawingPlacement) -> bool {
        let mut changed = false;
        for id in ids {
            if !self.document.drawings.iter().any(|d| d.uuid() == *id) {
                continue;
            }
            if self.document.drawing_placement(*id) == placement {
                continue;
            }
            match placement {
                DrawingPlacement::Behind => {
                    self.document.drawing_placements.remove(id);
                }
                DrawingPlacement::Above => {
                    self.document.drawing_placements.insert(*id, placement);
                }
            }
            changed = true;
        }
        changed
    }
}
