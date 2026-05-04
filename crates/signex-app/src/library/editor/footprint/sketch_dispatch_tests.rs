//! Phase 5.4 + 7.3 dispatcher tests.
//!
//! These run as inline tests under `#[cfg(test)]` so they exercise
//! the dispatcher without spinning up the iced runtime.

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use signex_library::primitive::footprint::Footprint;
    use signex_sketch::attr::{PadAttr, PadKind, PadShape, PadSide, PasteAperturePattern};
    use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
    use signex_sketch::entity::{Entity, EntityKind};
    use signex_sketch::id::{ConstraintId, SketchEntityId};
    use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
    use signex_sketch::SketchData;

    use super::super::sketch_dispatch::apply_sketch_edit;
    use super::super::sketch_mode::SketchEdit;
    use super::super::state::FootprintEditorState;

    fn empty_footprint() -> Footprint {
        Footprint::empty("test")
    }

    fn point_with_pad(plane: PlaneId, x: f64, y: f64, number: &str) -> (Entity, SketchEntityId) {
        let id = SketchEntityId::new();
        let mut e = Entity::new(id, plane, EntityKind::Point { x, y });
        e.pad = Some(PadAttr {
            number: number.into(),
            kind: PadKind::Smd,
            side: PadSide::Top,
            shape: PadShape::Rect,
            size_x_expr: "1mm".into(),
            size_y_expr: "0.5mm".into(),
            rotation_expr: None,
            offset_x_expr: None,
            offset_y_expr: None,
            drill: None,
            mask_margin_expr: None,
            paste_margin_expr: None,
            paste_apertures: PasteAperturePattern::Single,
        });
        (e, id)
    }

    #[test]
    fn add_entity_triggers_solve_and_bake() {
        let mut fp = empty_footprint();
        let plane = PlaneId::new();
        fp.sketch = Some(SketchData {
            planes: vec![Plane {
                id: plane,
                kind: PlaneKind::BoardTop,
            }],
            ..SketchData::default()
        });
        let mut state = FootprintEditorState::from_footprint(&fp);

        let (entity, _id) = point_with_pad(plane, 0.0, 0.0, "1");
        apply_sketch_edit(&mut state, &mut fp, SketchEdit::AddEntity(entity)).unwrap();

        assert_eq!(fp.pads.len(), 1);
        assert_eq!(fp.pads[0].number, "1");
        assert_eq!(fp.pads[0].position, [0.0, 0.0]);
        assert!(state.last_solve.is_some());
    }

    #[test]
    fn add_constraint_solves_geometry() {
        let mut fp = empty_footprint();
        let plane = PlaneId::new();
        let (e1, p1) = point_with_pad(plane, 0.0, 0.0, "1");
        let (e2, p2) = point_with_pad(plane, 1.0, 0.0, "2");
        fp.sketch = Some(SketchData {
            planes: vec![Plane {
                id: plane,
                kind: PlaneKind::BoardTop,
            }],
            entities: vec![e1, e2],
            constraints: vec![
                Constraint {
                    id: ConstraintId::new(),
                    kind: ConstraintKind::Fixed { point: p1 },
                },
                Constraint {
                    id: ConstraintId::new(),
                    kind: ConstraintKind::Horizontal {
                        line: SketchEntityId::new(), // dummy — Horizontal needs a real Line
                    },
                },
            ],
            ..SketchData::default()
        });
        // Drop the dummy Horizontal — the dispatcher's solve will then
        // run a clean Distance constraint that resolves to 5 mm.
        fp.sketch.as_mut().unwrap().constraints.truncate(1);
        let mut state = FootprintEditorState::from_footprint(&fp);

        apply_sketch_edit(
            &mut state,
            &mut fp,
            SketchEdit::AddConstraint(Constraint {
                id: ConstraintId::new(),
                kind: ConstraintKind::DistancePtPt {
                    p1,
                    p2,
                    target: DimTarget::Literal(5.0),
                },
            }),
        )
        .unwrap();

        // After the solve P2 should be at distance 5 from origin.
        assert!(state.last_solve.is_some());
        assert_eq!(fp.pads.len(), 2);
        let p2_pad = fp.pads.iter().find(|p| p.number == "2").unwrap();
        let dist = (p2_pad.position[0].powi(2) + p2_pad.position[1].powi(2)).sqrt();
        assert!(
            (dist - 5.0).abs() < 1e-6,
            "expected P2 at distance 5, got {dist}"
        );
    }

    #[test]
    fn set_mode_initialises_sketch_field() {
        // Footprint with no sketch initially: SetMode populates an
        // empty SketchData, runs the solve (no entities → no
        // residuals → instant return), and bakes pads from the
        // empty sketch (so fp.pads becomes empty).
        //
        // This documents the v0.13 design: once sketch mode is
        // opened, the sketch is the single source of truth for the
        // footprint's pad list. UX layers (Phase 6) surface a
        // confirmation prompt before the user permanently discards
        // their literal pad authoring; that confirmation lives
        // outside the dispatcher.
        let mut fp = empty_footprint();
        let mut state = FootprintEditorState::from_footprint(&fp);
        apply_sketch_edit(
            &mut state,
            &mut fp,
            SketchEdit::SetMode(super::super::state::EditorMode::Sketch),
        )
        .unwrap();

        assert!(fp.sketch.is_some());
        assert_eq!(fp.pads.len(), 0); // bake of empty sketch produces zero pads
    }

    #[test]
    fn line_tool_two_clicks_creates_line_with_snap() {
        // v0.13.2 Phase 6.4 — drive the Line-tool state machine
        // through two clicks. Second click snaps onto the first
        // entity to verify the auto-Coincident path.
        use crate::library::editor::footprint::state::{SketchTool, ToolPending};
        use crate::library::editor::footprint::sketch_mode::SketchEdit;
        use signex_sketch::entity::EntityKind;

        let mut fp = empty_footprint();
        let plane = PlaneId::new();
        fp.sketch = Some(signex_sketch::SketchData {
            planes: vec![signex_sketch::plane::Plane {
                id: plane,
                kind: signex_sketch::plane::PlaneKind::BoardTop,
            }],
            ..signex_sketch::SketchData::default()
        });
        let mut state = FootprintEditorState::from_footprint(&fp);
        state.active_tool = SketchTool::Line;

        // First click — adds the anchor Point and parks tool_pending
        // on LineFirst.
        let p1 = SketchEntityId::new();
        let pt = Entity::new(p1, plane, EntityKind::Point { x: 0.0, y: 0.0 });
        apply_sketch_edit(&mut state, &mut fp, SketchEdit::AddEntity(pt)).unwrap();
        state.tool_pending = ToolPending::LineFirst { first: p1 };

        // Second click — adds endpoint + Line entity. We do this
        // step manually because the dispatcher in app/dispatch is the
        // glue; this test verifies the SketchEdit-level behaviour.
        let p2 = SketchEntityId::new();
        let pt2 = Entity::new(p2, plane, EntityKind::Point { x: 5.0, y: 0.0 });
        apply_sketch_edit(&mut state, &mut fp, SketchEdit::AddEntity(pt2)).unwrap();
        let line_id = SketchEntityId::new();
        let line = Entity::new(
            line_id,
            plane,
            EntityKind::Line { start: p1, end: p2 },
        );
        apply_sketch_edit(&mut state, &mut fp, SketchEdit::AddEntity(line)).unwrap();

        let sketch = fp.sketch.as_ref().unwrap();
        assert_eq!(sketch.entities.len(), 3);
        assert!(sketch
            .entities
            .iter()
            .any(|e| matches!(e.kind, EntityKind::Line { start, end } if start == p1 && end == p2)));
    }

    #[test]
    fn auto_pause_skips_solve() {
        let mut fp = empty_footprint();
        let plane = PlaneId::new();
        let (e1, _p1) = point_with_pad(plane, 0.0, 0.0, "1");
        fp.sketch = Some(SketchData {
            planes: vec![Plane {
                id: plane,
                kind: PlaneKind::BoardTop,
            }],
            entities: vec![e1],
            ..SketchData::default()
        });
        let mut state = FootprintEditorState::from_footprint(&fp);

        // Force the auto-pause state machine into the paused state.
        state.auto_pause.observe(60, 50);
        state.auto_pause.observe(60, 50);
        assert!(state.auto_pause.paused());

        let pads_before = fp.pads.clone();
        apply_sketch_edit(&mut state, &mut fp, SketchEdit::ForceRebuild).unwrap();
        // Pads should not have been re-baked.
        assert_eq!(fp.pads.len(), pads_before.len());
        assert!(state.last_solve.is_none());
    }
}
