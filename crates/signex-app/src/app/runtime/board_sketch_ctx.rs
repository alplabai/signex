//! The Board section of the Properties panel in board sketch mode (#2234):
//! the selected entity's outline and mounting hole attributes.

use signex_sketch::entity::EntityKind;

use crate::app::FootprintDocument;
use crate::panels::{BoardSketchProps, MountingHoleProps};

/// The Board section's values for `editor`, or `None` when it edits a
/// footprint rather than a board sketch.
pub(super) fn board_sketch_props(editor: &FootprintDocument) -> Option<BoardSketchProps> {
    editor.state.board_backdrop.as_ref()?;
    let selected = editor.state.selected_sketch;
    let entity = selected.and_then(|id| {
        editor
            .primitive()
            .sketch
            .as_ref()?
            .entities
            .iter()
            .find(|e| e.id == id)
    });
    let Some(entity) = entity else {
        return Some(BoardSketchProps::default());
    };
    Some(BoardSketchProps {
        selected,
        is_edge: !matches!(entity.kind, EntityKind::Point { .. }),
        is_circle: matches!(entity.kind, EntityKind::Circle { .. }),
        outline: entity.board_outline,
        hole: entity.mounting_hole.as_ref().map(|hole| MountingHoleProps {
            drill: hole.drill_expr.clone().unwrap_or_default(),
            plated: hole.plated,
            net: hole.net.clone().unwrap_or_default(),
        }),
    })
}
