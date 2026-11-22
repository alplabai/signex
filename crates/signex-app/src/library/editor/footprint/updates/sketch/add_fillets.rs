//! Add fillets (mockup row 179d): round the failing sharp inside corners to
//! the radius the router needs, as one undo step, solved like every sketch
//! edit. The geometry is `signex_bake::board::add_fillets`; a corner it
//! cannot round is left alone and said in Messages. Rules:
//! `docs/PCB_SKETCH.md`, "Outline checks".

use signex_bake::board::{FilletReport, add_fillets, fillet_targets};
use signex_sketch::sketch::SketchData;

use crate::app::FootprintDocument;
use crate::library::editor::footprint::sketch_conflict::mutate_solve_or_revert;
use crate::library::editor::footprint::sketch_dispatch::report_solve_failure;

pub(in crate::library::editor::footprint::updates) fn apply(editor: &mut FootprintDocument) {
    editor.record_edit(None, |editor| {
        let Some((next, report)) = filleted(editor) else {
            return false;
        };
        if report.refused > 0 {
            crate::diagnostics::log_warning(format!(
                "Add fillets: {} corner(s) left as they were; the lines are too short for the radius, or a constraint or pattern depends on the corner",
                report.refused
            ));
        }
        if report.applied == 0 {
            return false;
        }
        let before = editor.primitive().sketch.clone();
        let solved = editor
            .with_parts(|state, fp| mutate_solve_or_revert(state, fp, |fp| fp.sketch = Some(next)));
        if let Err(error) = &solved {
            editor.with_parts(|state, _| report_solve_failure(state, error));
        }
        let changed = editor.primitive().sketch != before;
        if changed {
            editor.mark_dirty();
        }
        changed
    });
    editor.canvas_cache.clear();
}

/// The sketch with its failing corners rounded; `None` when nothing fails
/// that a fillet can fix.
fn filleted(editor: &FootprintDocument) -> Option<(SketchData, FilletReport)> {
    let sketch = editor.primitive().sketch.as_ref()?;
    let solve = editor.state.last_solve.as_ref()?;
    let checks = editor.state.board_checks(Some(sketch))?;
    let targets = fillet_targets(sketch, &checks);
    if targets.is_empty() {
        return None;
    }
    let mut next = sketch.clone();
    let report = add_fillets(&mut next, solve, &targets);
    Some((next, report))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use signex_bake::board::CheckState;
    use signex_library::{Footprint, FootprintFile};
    use signex_sketch::Solver;
    use signex_sketch::entity::{Entity, EntityKind};
    use signex_sketch::id::SketchEntityId;
    use signex_sketch::parameter::resolve;
    use signex_sketch::plane::{Plane, PlaneId, PlaneKind};

    use super::*;
    use crate::library::editor::footprint::state::BoardBackdrop;
    use crate::library::editor::footprint::updates::apply_footprint_primitive_edit;
    use crate::library::messages::FootprintEditorMsg;

    /// A board sketch editor over an L-shaped outline, router tool Ø 2.00.
    fn l_board() -> FootprintDocument {
        let plane = PlaneId::new();
        let mut sketch = SketchData::default();
        sketch.planes.push(Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        });
        let corners = [
            (0.0, 0.0),
            (10.0, 0.0),
            (10.0, 5.0),
            (5.0, 5.0),
            (5.0, 10.0),
            (0.0, 10.0),
        ];
        let points: Vec<SketchEntityId> = corners
            .iter()
            .map(|&(x, y)| {
                let id = SketchEntityId::new();
                sketch
                    .entities
                    .push(Entity::new(id, plane, EntityKind::Point { x, y }));
                id
            })
            .collect();
        for i in 0..points.len() {
            let kind = EntityKind::Line {
                start: points[i],
                end: points[(i + 1) % points.len()],
            };
            let mut line = Entity::new(SketchEntityId::new(), plane, kind);
            line.board_outline = true;
            sketch.entities.push(line);
        }
        let params = resolve(&sketch.parameters).unwrap();
        let solve = Solver::default().solve(&sketch, &params).unwrap();
        let mut fp = Footprint::empty("board");
        fp.sketch = Some(sketch);
        let mut editor =
            FootprintDocument::new(PathBuf::from("t.snxpcb"), FootprintFile::from_footprint(fp));
        editor.state.board_backdrop = Some(Arc::new(BoardBackdrop::default()));
        editor.state.sketch_exit.router_tool_diameter = Some(2.0);
        editor.state.last_solve = Some(solve);
        editor
    }

    /// DECISIONS 2026-11-22, mockup row 179d: Add fillets applies the
    /// required radius as one undo step.
    #[test]
    fn add_fillets_fixes_the_corner_in_one_undo_step() {
        let mut editor = l_board();
        let before = editor.primitive().sketch.clone();
        let marks = editor.state.failure_marks(before.as_ref().unwrap());
        assert_eq!(marks.len(), 1, "{marks:?}");
        assert_eq!(marks[0].at, (5.0, 5.0));
        assert_eq!(marks[0].label, "R 0.00 < R 1.00");

        apply_footprint_primitive_edit(&mut editor, FootprintEditorMsg::SketchAddFilletsRequested);

        let sketch = editor.primitive().sketch.as_ref();
        let checks = editor.state.board_checks(sketch).expect("solved");
        assert_eq!(checks.inside_corners, CheckState::Passed, "{checks:?}");
        assert!(editor.state.failure_marks(sketch.unwrap()).is_empty());
        assert!(editor.undo(), "one undo step");
        assert_eq!(editor.primitive().sketch, before);
        assert!(!editor.history.can_undo(), "and only one");
    }

    #[test]
    fn add_fillets_with_nothing_failing_leaves_no_undo_step() {
        let mut editor = l_board();
        editor.state.sketch_exit.router_tool_diameter = None;

        apply_footprint_primitive_edit(&mut editor, FootprintEditorMsg::SketchAddFilletsRequested);

        assert!(!editor.history.can_undo());
    }
}
