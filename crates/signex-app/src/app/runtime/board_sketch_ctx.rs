//! The Board sections of the Properties panel in board sketch mode (#2234):
//! the board's name and outline facts, the outline checks of the current
//! solve (mockup 179d), and the selected entity's outline and mounting hole
//! attributes.

use signex_sketch::entity::EntityKind;

use crate::app::FootprintDocument;
use crate::library::editor::footprint::state::check_lines;
use crate::panels::{BoardSketchProps, MountingHoleProps};

/// The Board sections' values for `editor`, or `None` when it edits a
/// footprint rather than a board sketch.
pub(super) fn board_sketch_props(editor: &FootprintDocument) -> Option<BoardSketchProps> {
    let backdrop = editor.state.board_backdrop.as_ref()?;
    let sketch = editor.primitive().sketch.as_ref();
    let board = BoardSketchProps {
        board_name: editor.primitive().name.clone(),
        board_size_mm: backdrop.outline_size_mm(),
        board_area_mm2: backdrop.outline_area_mm2(),
        // Read here, where every path that solves ends up, so they are live.
        outline_checks: check_lines(editor.state.board_checks(sketch).as_ref()),
        router_tool_diameter: editor.state.sketch_exit.router_tool_diameter,
        ..BoardSketchProps::default()
    };
    let selected = editor.state.selected_sketch;
    let entity = selected.and_then(|id| sketch?.entities.iter().find(|e| e.id == id));
    let Some(entity) = entity else {
        return Some(board);
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
        ..board
    })
}
