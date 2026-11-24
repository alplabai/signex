//! Ordering of schematic drawings (#1583).

use super::super::*;
use crate::app::command::{OperandValue, translator};
use signex_engine::DrawingArrange;
use signex_types::schematic::SelectedKind;

/// The catalog command of each order step.
fn arrange_command(arrange: DrawingArrange) -> &'static str {
    match arrange {
        DrawingArrange::Forward => "bring_drawings_forward",
        DrawingArrange::ToFront => "bring_drawings_to_front",
        DrawingArrange::Backward => "send_drawings_backward",
        DrawingArrange::ToBack => "send_drawings_to_back",
    }
}

impl Signex {
    /// Reorder the selected drawings per `arrange`, as one undo step: the
    /// sheet in view and the drawings of its selection run the command's
    /// operand handler (`command::translator`). Symbols, wires and the rest
    /// of a mixed selection are ignored.
    pub(crate) fn handle_drawings_arrange_requested(
        &mut self,
        arrange: DrawingArrange,
    ) -> Task<Message> {
        let ids = self.selected_drawing_ids();
        if ids.is_empty() || !self.has_active_schematic() {
            return Task::none();
        }
        let (Some(path), Some(engine)) = (
            self.document_state.active_path.clone(),
            self.document_state.active_engine(),
        ) else {
            return Task::none();
        };
        // Only drawings the sheet holds: the engine skipped any other, the
        // handler refuses a list naming one.
        let drawings = &engine.document().drawings;
        let ids: Vec<uuid::Uuid> = (ids.into_iter())
            .filter(|id| drawings.iter().any(|d| d.uuid() == *id))
            .collect();
        if ids.is_empty() {
            return Task::none();
        }
        let operands = vec![("ids", OperandValue::Targets(ids))];
        translator::run(self, arrange_command(arrange), &path, operands).1
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
