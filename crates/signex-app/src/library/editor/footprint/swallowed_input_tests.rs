//! GH #599 batch B — numeric inputs whose parse failure used to be
//! replaced with a plausible substitute.
//!
//! Two shapes are covered here:
//!
//!   * a dimensional constraint whose `dimension_input` will not parse
//!     used to end in a bare `if let Some(kind) = new_kind { … }` with
//!     no `else` — no constraint, no message, no highlight, `dirty`
//!     untouched and the canvas cache intact, so even the redraw was
//!     identical;
//!   * the Offset distance and the Fillet radius used to fall back to
//!     0.5 mm on a failed parse, which is worse than a no-op — the
//!     operation SUCCEEDS at a size nobody asked for, and a wrong
//!     fillet radius goes to fabrication.
//!
//! `1,5` (a comma-decimal keyboard) is the realistic trigger for both.

#[cfg(test)]
mod tests {
    use signex_library::primitive::footprint::{Footprint, FootprintFile};
    use signex_sketch::SketchData;
    use signex_sketch::entity::{Entity, EntityKind};
    use signex_sketch::id::SketchEntityId;
    use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
    use std::path::PathBuf;

    use crate::library::editor::footprint::state::SketchTool;
    use crate::library::editor::footprint::updates::apply_footprint_primitive_edit;
    use crate::library::messages::{FootprintEditorMsg, SketchConstraintTag};

    /// Editor holding one horizontal Line from (0, 0) to (10, 0) plus
    /// its two endpoints. Returns the endpoint ids and the Line id.
    fn line_editor() -> (
        crate::app::FootprintEditorState,
        SketchEntityId,
        SketchEntityId,
        SketchEntityId,
    ) {
        let plane = PlaneId::new();
        let start = SketchEntityId::new();
        let end = SketchEntityId::new();
        let line = SketchEntityId::new();
        let mut fp = Footprint::empty("test");
        fp.sketch = Some(SketchData {
            planes: vec![Plane {
                id: plane,
                kind: PlaneKind::BoardTop,
            }],
            entities: vec![
                Entity::new(start, plane, EntityKind::Point { x: 0.0, y: 0.0 }),
                Entity::new(end, plane, EntityKind::Point { x: 10.0, y: 0.0 }),
                Entity::new(line, plane, EntityKind::Line { start, end }),
            ],
            ..SketchData::default()
        });
        let editor = crate::app::FootprintEditorState::new(
            PathBuf::from("swallowed-input.snxfpt"),
            FootprintFile::from_footprint(fp),
        );
        (editor, start, end, line)
    }

    /// Editor holding two Lines meeting at the origin — the shape the
    /// Fillet tool needs, and two Lines for the Angle constraint.
    /// Returns the editor plus the east-going and north-going Line ids.
    fn corner_editor() -> (
        crate::app::FootprintEditorState,
        SketchEntityId,
        SketchEntityId,
    ) {
        let plane = PlaneId::new();
        let corner = SketchEntityId::new();
        let east = SketchEntityId::new();
        let north = SketchEntityId::new();
        let line_east = SketchEntityId::new();
        let line_north = SketchEntityId::new();
        let mut fp = Footprint::empty("test");
        fp.sketch = Some(SketchData {
            planes: vec![Plane {
                id: plane,
                kind: PlaneKind::BoardTop,
            }],
            entities: vec![
                Entity::new(corner, plane, EntityKind::Point { x: 0.0, y: 0.0 }),
                Entity::new(east, plane, EntityKind::Point { x: 10.0, y: 0.0 }),
                Entity::new(north, plane, EntityKind::Point { x: 0.0, y: 10.0 }),
                Entity::new(
                    line_east,
                    plane,
                    EntityKind::Line {
                        start: corner,
                        end: east,
                    },
                ),
                Entity::new(
                    line_north,
                    plane,
                    EntityKind::Line {
                        start: corner,
                        end: north,
                    },
                ),
            ],
            ..SketchData::default()
        });
        let mut editor = crate::app::FootprintEditorState::new(
            PathBuf::from("swallowed-fillet.snxfpt"),
            FootprintFile::from_footprint(fp),
        );
        editor.state.active_tool = SketchTool::Fillet;
        (editor, line_east, line_north)
    }

    fn constraint_count(editor: &crate::app::FootprintEditorState) -> usize {
        editor
            .primitive()
            .sketch
            .as_ref()
            .map_or(0, |s| s.constraints.len())
    }

    fn line_count(editor: &crate::app::FootprintEditorState) -> usize {
        editor.primitive().sketch.as_ref().map_or(0, |s| {
            s.entities
                .iter()
                .filter(|e| matches!(e.kind, EntityKind::Line { .. }))
                .count()
        })
    }

    fn arc_count(editor: &crate::app::FootprintEditorState) -> usize {
        editor.primitive().sketch.as_ref().map_or(0, |s| {
            s.entities
                .iter()
                .filter(|e| matches!(e.kind, EntityKind::Arc { .. }))
                .count()
        })
    }

    // ---------------------------------------------------------------
    // Row 1 — dimensional constraints.
    // ---------------------------------------------------------------

    #[test]
    fn distance_constraint_with_comma_decimal_is_reported_not_swallowed() {
        let (mut editor, start, end, _line) = line_editor();
        editor.state.selected_sketch = Some(start);
        editor.state.selected_sketch_secondary = Some(end);
        editor.state.dimension_input = "1,5".into();
        let before = constraint_count(&editor);

        apply_footprint_primitive_edit(
            &mut editor,
            FootprintEditorMsg::SketchAddConstraintForSelection(SketchConstraintTag::DistancePtPt),
        );

        assert_eq!(
            constraint_count(&editor),
            before,
            "an unreadable dimension must not mint a constraint"
        );
        assert!(
            editor
                .state
                .solve_warnings
                .iter()
                .any(|w| w.contains("1,5")),
            "the refused dimension must be named back to the user, got {:?}",
            editor.state.solve_warnings
        );
    }

    #[test]
    fn angle_constraint_with_unit_suffix_is_reported_not_swallowed() {
        let (mut editor, line_east, line_north) = corner_editor();
        editor.state.selected_sketch = Some(line_east);
        editor.state.selected_sketch_secondary = Some(line_north);
        editor.state.dimension_input = "45deg".into();

        apply_footprint_primitive_edit(
            &mut editor,
            FootprintEditorMsg::SketchAddConstraintForSelection(SketchConstraintTag::Angle),
        );

        assert_eq!(constraint_count(&editor), 0);
        assert!(
            editor
                .state
                .solve_warnings
                .iter()
                .any(|w| w.contains("45deg")),
            "got {:?}",
            editor.state.solve_warnings
        );
    }

    #[test]
    fn distance_constraint_with_a_readable_dimension_still_applies() {
        let (mut editor, start, end, _line) = line_editor();
        editor.state.selected_sketch = Some(start);
        editor.state.selected_sketch_secondary = Some(end);
        editor.state.dimension_input = "1.5".into();

        apply_footprint_primitive_edit(
            &mut editor,
            FootprintEditorMsg::SketchAddConstraintForSelection(SketchConstraintTag::DistancePtPt),
        );

        assert_eq!(
            constraint_count(&editor),
            1,
            "a readable dimension must still add the constraint"
        );
        assert!(editor.dirty, "adding a constraint dirties the editor");
    }

    #[test]
    fn constraint_that_does_not_match_the_selection_is_reported() {
        let (mut editor, start, _end, _line) = line_editor();
        // Parallel needs two Lines; one Point is not a match.
        editor.state.selected_sketch = Some(start);
        editor.state.selected_sketch_secondary = None;

        apply_footprint_primitive_edit(
            &mut editor,
            FootprintEditorMsg::SketchAddConstraintForSelection(SketchConstraintTag::Parallel),
        );

        assert_eq!(constraint_count(&editor), 0);
        assert!(
            !editor.state.solve_warnings.is_empty(),
            "a selection mismatch must not be a silent no-op"
        );
    }

    // ---------------------------------------------------------------
    // Row 2 — Offset distance / Fillet radius.
    // ---------------------------------------------------------------

    #[test]
    fn offset_with_an_unreadable_distance_creates_nothing() {
        let (mut editor, _start, _end, line) = line_editor();
        editor.state.active_tool = SketchTool::Offset;
        editor.state.selected_sketch = Some(line);
        editor.state.dimension_input = "2,5".into();
        let before = line_count(&editor);

        apply_footprint_primitive_edit(
            &mut editor,
            FootprintEditorMsg::SketchToolClick {
                x_mm: 5.0,
                y_mm: 3.0,
                snap_id: None,
            },
        );

        assert_eq!(
            line_count(&editor),
            before,
            "an unreadable offset distance must not mint geometry at the default"
        );
        assert!(
            editor
                .state
                .solve_warnings
                .iter()
                .any(|w| w.contains("2,5")),
            "the refused distance must be named back to the user, got {:?}",
            editor.state.solve_warnings
        );
    }

    #[test]
    fn offset_with_an_empty_distance_still_uses_the_default() {
        let (mut editor, _start, _end, line) = line_editor();
        editor.state.active_tool = SketchTool::Offset;
        editor.state.selected_sketch = Some(line);
        editor.state.dimension_input = String::new();
        let before = line_count(&editor);

        apply_footprint_primitive_edit(
            &mut editor,
            FootprintEditorMsg::SketchToolClick {
                x_mm: 5.0,
                y_mm: 3.0,
                snap_id: None,
            },
        );

        assert_eq!(
            line_count(&editor),
            before + 1,
            "an empty buffer is the one case the default fallback is for"
        );
    }

    #[test]
    fn fillet_with_an_unreadable_radius_creates_nothing() {
        let (mut editor, _east, _north) = corner_editor();
        editor.state.dimension_input = "1,5".into();
        let arcs_before = arc_count(&editor);

        // Click 1 picks the first Line, click 2 completes the gesture.
        for (x_mm, y_mm) in [(5.0, 0.0), (0.0, 5.0)] {
            apply_footprint_primitive_edit(
                &mut editor,
                FootprintEditorMsg::SketchToolClick {
                    x_mm,
                    y_mm,
                    snap_id: None,
                },
            );
        }

        assert_eq!(
            arc_count(&editor),
            arcs_before,
            "an unreadable fillet radius must not mint an arc at the default"
        );
        assert!(
            editor
                .state
                .solve_warnings
                .iter()
                .any(|w| w.contains("1,5")),
            "the refused radius must be named back to the user, got {:?}",
            editor.state.solve_warnings
        );
    }

    #[test]
    fn fillet_with_an_empty_radius_still_uses_the_default() {
        let (mut editor, _east, _north) = corner_editor();
        editor.state.dimension_input = String::new();
        let arcs_before = arc_count(&editor);

        for (x_mm, y_mm) in [(5.0, 0.0), (0.0, 5.0)] {
            apply_footprint_primitive_edit(
                &mut editor,
                FootprintEditorMsg::SketchToolClick {
                    x_mm,
                    y_mm,
                    snap_id: None,
                },
            );
        }

        assert_eq!(
            arc_count(&editor),
            arcs_before + 1,
            "an empty buffer is the one case the default fallback is for"
        );
    }
}
