//! The Board sections of the Properties panel in board sketch mode (#2234):
//! the board's name and outline facts, the outline checks of the current
//! solve (mockup row 179d), and the selected entity's outline and mounting hole
//! attributes.

use signex_sketch::entity::EntityKind;

use crate::app::FootprintDocument;
use crate::library::editor::footprint::check_lines;
use crate::library::editor::footprint::sketch_mode::roles::edge_slot_size;
use crate::panels::pcb_board_properties::outline_is_rectangle;
use crate::panels::{BoardSketchProps, MountingHoleProps};
use signex_types::pcb::Point;

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

    /// The Board sections' values for `editor` with what the app holds: the
    /// selected entity's targets, the bake preview and the fab profile.
    pub(super) fn board_sketch_panel_props(
        &self,
        editor: &FootprintDocument,
    ) -> Option<BoardSketchProps> {
        board_sketch_props(editor).map(|board| BoardSketchProps {
            target: self.board_sketch_target(),
            bake: self.board_sketch_bake_preview(),
            fab_profile: self.board_sketch_fab_profile(),
            ..board
        })
    }

    /// The name of the rule profile the board's project was seeded from,
    /// for the Fab profile group (mockup row 179d); `None` when the project
    /// names none.
    fn board_sketch_fab_profile(&self) -> Option<String> {
        let path = &self.board_sketch_in_view()?.board_path;
        let ds = &self.document_state;
        let project = ds
            .document_project(path)
            .or_else(|| ds.project_for_path(path))?;
        let name = &project.data.rules.as_ref()?.profile.as_ref()?.name;
        (!name.is_empty()).then(|| name.clone())
    }
}

/// The Board sections' values for `editor`, or `None` when it edits a
/// footprint rather than a board sketch.
fn board_sketch_props(editor: &FootprintDocument) -> Option<BoardSketchProps> {
    let backdrop = editor.state.board_backdrop.as_ref()?;
    let sketch = editor.primitive().sketch.as_ref();
    let checks = editor.state.board_checks(sketch);
    let fillet_radius = sketch.zip(checks.as_ref()).and_then(|(sketch, checks)| {
        signex_bake::board::fillet_targets(sketch, checks)
            .first()
            .map(|&(_, radius)| radius)
    });
    // The sketch's closed outline is what Bake writes, so the size reads
    // from it; without one, from the board's (mockup row 179e).
    let shape = sketch_shape(editor);
    let backdrop_outline: Vec<Point> = backdrop
        .outline
        .iter()
        .map(|&(x, y)| Point { x, y })
        .collect();
    let board = BoardSketchProps {
        board_name: editor.primitive().name.clone(),
        board_size_mm: shape
            .map(|shape| shape.size)
            .or_else(|| backdrop.outline_size_mm()),
        board_area_mm2: shape
            .map(|shape| shape.area)
            .or_else(|| backdrop.outline_area_mm2()),
        size_from_shape: shape.is_some(),
        outline_is_rectangle: shape.map_or_else(
            || outline_is_rectangle(&backdrop_outline),
            |shape| shape.is_rectangle,
        ),
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

/// The closed outline a board sketch bakes, as the Size and Area rows read it.
#[derive(Debug, Clone, Copy)]
struct SketchShape {
    /// The extent `(width, height)` in mm.
    size: (f64, f64),
    /// The enclosed area in mm².
    area: f64,
    is_rectangle: bool,
}

/// The closed outline the sketch bakes at its last solve; `None` without one.
fn sketch_shape(editor: &FootprintDocument) -> Option<SketchShape> {
    let sketch = editor.primitive().sketch.as_ref()?;
    let solve = editor.state.last_solve.as_ref()?;
    let outline = signex_bake::board::sketch_outline(sketch, solve)?;
    let (first, rest) = outline.split_first()?;
    let (lo, hi) = rest
        .iter()
        .fold(((first.x, first.y), (first.x, first.y)), |(lo, hi), p| {
            (
                (lo.0.min(p.x), lo.1.min(p.y)),
                (hi.0.max(p.x), hi.1.max(p.y)),
            )
        });
    let ring: Vec<signex_sketch::geom::Point2> =
        outline.iter().map(|p| (p.x, p.y).into()).collect();
    Some(SketchShape {
        size: (hi.0 - lo.0, hi.1 - lo.1),
        area: signex_sketch::geom::predicates::signed_area(&ring).abs(),
        is_rectangle: outline_is_rectangle(&outline),
    })
}
