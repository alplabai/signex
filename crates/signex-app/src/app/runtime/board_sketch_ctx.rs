//! The Board sections of the Properties panel in board sketch mode (#2234):
//! the board's name and outline facts, the outline checks of the current
//! solve (mockup row 179d), and the selected entity's outline and mounting hole
//! attributes.

use signex_sketch::entity::EntityKind;

use crate::app::FootprintDocument;
use crate::library::editor::footprint::check_lines;
use crate::library::editor::footprint::sketch_mode::roles::edge_slot_size;
use crate::panels::{BoardSketchProps, MountingHoleProps};

/// The role rows and centerline count of a sketch in Sketch mode: the Roles
/// section of a footprint (mockup row 190), the Bake as list of a board's
/// Bake panel (mockup row 179); nothing outside the mode.
pub(super) fn sketch_role_rows(
    editor: &FootprintDocument,
) -> (
    Vec<crate::library::editor::footprint::sketch_mode::roles::ProfileRoleRow>,
    usize,
) {
    use crate::library::editor::footprint::EditorMode;
    use crate::library::editor::footprint::sketch_mode::roles::{
        board_profile_rows, profile_role_rows,
    };
    use signex_types::sketch::SketchHost;
    let host = editor.state.sketch_host();
    let sketch = editor.primitive().sketch.as_ref();
    let centerlines =
        |s: &signex_sketch::SketchData| s.entities.iter().filter(|e| e.centerline).count();
    match sketch {
        Some(s) if editor.state.mode == EditorMode::Sketch && host == SketchHost::Footprint => {
            (profile_role_rows(s, host), centerlines(s))
        }
        Some(s) if editor.state.mode == EditorMode::Sketch && host == SketchHost::Board => (
            board_profile_rows(s, editor.state.last_solve.as_ref()),
            centerlines(s),
        ),
        _ => (Vec::new(), 0),
    }
}

impl crate::app::Signex {
    /// The open bake preview of the board sketch in view, for the
    /// Properties Bake panel (mockup row 179); `None` while it is closed.
    pub(crate) fn board_sketch_bake_preview(&self) -> Option<crate::panels::BoardBakePreview> {
        self.board_sketch_in_view()?.preview.clone()
    }
}

/// The Board sections' values for `editor`, or `None` when it edits a
/// footprint rather than a board sketch.
pub(super) fn board_sketch_props(editor: &FootprintDocument) -> Option<BoardSketchProps> {
    let backdrop = editor.state.board_backdrop.as_ref()?;
    let sketch = editor.primitive().sketch.as_ref();
    let checks = editor.state.board_checks(sketch);
    let fillet_radius = sketch.zip(checks.as_ref()).and_then(|(sketch, checks)| {
        signex_bake::board::fillet_targets(sketch, checks)
            .first()
            .map(|&(_, radius)| radius)
    });
    let board = BoardSketchProps {
        board_name: editor.primitive().name.clone(),
        board_size_mm: backdrop.outline_size_mm(),
        board_area_mm2: backdrop.outline_area_mm2(),
        // Read here, where every path that solves ends up, so they are live.
        outline_checks: check_lines(checks.as_ref()),
        router_tool_diameter: editor.state.sketch_exit.router_tool_diameter,
        fillet_radius,
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
        selected_slot: sketch
            .and_then(|s| edge_slot_size(s, editor.state.last_solve.as_ref(), entity.id)),
        outline: entity.board_outline,
        hole: entity.mounting_hole.as_ref().map(|hole| MountingHoleProps {
            drill: hole.drill_expr.clone().unwrap_or_default(),
            plated: hole.plated,
            net: hole.net.clone().unwrap_or_default(),
        }),
        ..board
    })
}
