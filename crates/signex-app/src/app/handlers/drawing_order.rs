//! Ordering of schematic drawings (#1583).

use super::super::*;
use signex_types::schematic::SelectedKind;

impl Signex {
    /// Reorder the selected drawings per `arrange`, as one undo step. Only
    /// drawings in the selection move; symbols, wires and the rest of a mixed
    /// selection are ignored.
    pub(crate) fn handle_drawings_arrange_requested(
        &mut self,
        arrange: signex_engine::DrawingArrange,
    ) {
        let ids = self.selected_drawing_ids();
        if ids.is_empty() || !self.has_active_schematic() {
            return;
        }
        self.apply_engine_command(
            signex_engine::Command::ArrangeDrawings { ids, arrange },
            false,
            true,
        );
    }

    /// The uuids of the drawings in the active canvas selection.
    pub(crate) fn selected_drawing_ids(&self) -> Vec<uuid::Uuid> {
        self.interaction_state
            .active_canvas()
            .selected
            .iter()
            .filter(|item| item.kind == SelectedKind::Drawing)
            .map(|item| item.uuid)
            .collect()
    }
}
