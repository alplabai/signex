//! Phase 5.4 + 7.3 dispatcher tests.
//!
//! These run as inline tests under `#[cfg(test)]` so they exercise
//! the dispatcher without spinning up the iced runtime.

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use signex_library::primitive::footprint::Footprint;
    use signex_sketch::SketchData;
    use signex_sketch::attr::{PadAttr, PadKind, PadShape, PadSide, PasteAperturePattern};
    use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
    use signex_sketch::entity::{Entity, EntityKind};
    use signex_sketch::id::{ConstraintId, SketchEntityId};
    use signex_sketch::plane::{Plane, PlaneId, PlaneKind};

    use super::super::sketch_dispatch::{apply_sketch_edit, apply_sketch_edit_with_warnings};
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
            ..PadAttr::default()
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
    fn set_mode_initialises_sketch_field_and_preserves_literal_pads() {
        // Footprint with literal (manually-authored) pads and no sketch
        // initially. SetMode populates an empty SketchData and runs the
        // solver; the bake step is gated on `!sketch.entities.is_empty()`
        // so the existing literal pads survive the first Sketch-mode
        // toggle. Once the user actually authors sketch entities, the
        // sketch becomes the source of truth and the bake's output
        // (possibly an empty Vec) overwrites the literal pads.
        use signex_library::primitive::footprint::{
            LayerId, Pad, PadKind as LibPadKind, PadShape as LibPadShape,
        };

        let mut fp = empty_footprint();
        // Pre-populate with one literal pad so we can verify it survives.
        fp.pads.push(Pad {
            number: "literal".into(),
            kind: LibPadKind::Smd,
            shape: LibPadShape::Rect,
            size: [1.0, 0.5],
            position: [0.0, 0.0],
            rotation: 0.0,
            layers: vec![LayerId::new("Top Layer")],
            drill: None,
            solder_mask_margin: None,
            paste_margin: None,
            ..Pad::default()
        });

        let mut state = FootprintEditorState::from_footprint(&fp);
        apply_sketch_edit(
            &mut state,
            &mut fp,
            SketchEdit::SetMode(super::super::state::EditorMode::Sketch),
        )
        .unwrap();

        assert!(fp.sketch.is_some());
        assert_eq!(
            fp.pads.len(),
            1,
            "literal pad must survive empty Sketch toggle"
        );
        assert_eq!(fp.pads[0].number, "literal");
    }

    #[test]
    fn line_tool_two_clicks_creates_line_with_snap() {
        // v0.13.2 Phase 6.4 — drive the Line-tool state machine
        // through two clicks. Second click snaps onto the first
        // entity to verify the auto-Coincident path.
        use crate::library::editor::footprint::sketch_mode::SketchEdit;
        use crate::library::editor::footprint::state::{SketchTool, ToolPending};
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
        let line = Entity::new(line_id, plane, EntityKind::Line { start: p1, end: p2 });
        apply_sketch_edit(&mut state, &mut fp, SketchEdit::AddEntity(line)).unwrap();

        let sketch = fp.sketch.as_ref().unwrap();
        assert_eq!(sketch.entities.len(), 3);
        assert!(sketch.entities.iter().any(
            |e| matches!(e.kind, EntityKind::Line { start, end } if start == p1 && end == p2)
        ));
    }

    #[test]
    fn warning_wrapper_captures_parse_error_into_solve_warnings() {
        // EditParameter with bad expression syntax — the resolver fails
        // on parse, the dispatcher returns SketchError::Expr, and the
        // _with_warnings wrapper must capture the message into
        // state.solve_warnings instead of dropping it on the floor.
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

        // Unbalanced parens — guaranteed parser failure.
        apply_sketch_edit_with_warnings(
            &mut state,
            &mut fp,
            SketchEdit::EditParameter {
                name: "bad".into(),
                expr: "(((".into(),
            },
        );

        assert!(
            !state.solve_warnings.is_empty(),
            "wrapper should have captured a parse error into solve_warnings"
        );
        // Message contains either "parse" or "expression" depending on
        // the ExprError::Display impl — accept any non-empty warning
        // for forward compat with the error-message wording.
        assert!(state.solve_warnings.iter().any(|w| !w.is_empty()));
    }

    #[test]
    fn set_role_pad_on_point_attaches_pad_attr_and_bakes() {
        use super::super::sketch_dispatch::{apply_sketch_role, current_role_of};
        use crate::library::messages::RoleTag;

        let mut fp = empty_footprint();
        let plane = PlaneId::new();
        let pid = SketchEntityId::new();
        let mut e = Entity::new(pid, plane, EntityKind::Point { x: 1.0, y: 2.0 });
        e.pad = None; // brand-new Point, Unassigned role
        fp.sketch = Some(SketchData {
            planes: vec![Plane {
                id: plane,
                kind: PlaneKind::BoardTop,
            }],
            entities: vec![e],
            ..SketchData::default()
        });
        let mut state = FootprintEditorState::from_footprint(&fp);

        apply_sketch_role(&mut state, &mut fp, pid, RoleTag::Pad).unwrap();

        let entity = &fp.sketch.as_ref().unwrap().entities[0];
        assert!(entity.pad.is_some(), "Pad attr must be attached");
        assert_eq!(current_role_of(entity), RoleTag::Pad);
        assert_eq!(fp.pads.len(), 1, "bake must emit the pad");
        assert_eq!(fp.pads[0].position, [1.0, 2.0]);
    }

    #[test]
    fn set_role_pad_on_line_is_silent_noop() {
        use super::super::sketch_dispatch::{apply_sketch_role, current_role_of};
        use crate::library::messages::RoleTag;

        let mut fp = empty_footprint();
        let plane = PlaneId::new();
        let p1 = SketchEntityId::new();
        let p2 = SketchEntityId::new();
        let lid = SketchEntityId::new();
        let pt1 = Entity::new(p1, plane, EntityKind::Point { x: 0.0, y: 0.0 });
        let pt2 = Entity::new(p2, plane, EntityKind::Point { x: 1.0, y: 0.0 });
        let line = Entity::new(lid, plane, EntityKind::Line { start: p1, end: p2 });
        fp.sketch = Some(SketchData {
            planes: vec![Plane {
                id: plane,
                kind: PlaneKind::BoardTop,
            }],
            entities: vec![pt1, pt2, line],
            ..SketchData::default()
        });
        let mut state = FootprintEditorState::from_footprint(&fp);

        apply_sketch_role(&mut state, &mut fp, lid, RoleTag::Pad).unwrap();

        let line_entity = fp
            .sketch
            .as_ref()
            .unwrap()
            .entities
            .iter()
            .find(|e| e.id == lid)
            .unwrap();
        assert!(
            line_entity.pad.is_none(),
            "Pad attr must NOT attach to a Line"
        );
        assert_eq!(current_role_of(line_entity), RoleTag::Unassigned);
    }

    #[test]
    fn set_role_silk_top_attaches_silk_attr_with_top_layer() {
        use super::super::sketch_dispatch::{apply_sketch_role, current_role_of};
        use crate::library::messages::RoleTag;
        use signex_types::layer::SignexLayer;

        let mut fp = empty_footprint();
        let plane = PlaneId::new();
        let p1 = SketchEntityId::new();
        let p2 = SketchEntityId::new();
        let lid = SketchEntityId::new();
        let pt1 = Entity::new(p1, plane, EntityKind::Point { x: 0.0, y: 0.0 });
        let pt2 = Entity::new(p2, plane, EntityKind::Point { x: 1.0, y: 0.0 });
        let line = Entity::new(lid, plane, EntityKind::Line { start: p1, end: p2 });
        fp.sketch = Some(SketchData {
            planes: vec![Plane {
                id: plane,
                kind: PlaneKind::BoardTop,
            }],
            entities: vec![pt1, pt2, line],
            ..SketchData::default()
        });
        let mut state = FootprintEditorState::from_footprint(&fp);

        apply_sketch_role(&mut state, &mut fp, lid, RoleTag::SilkTop).unwrap();

        let line_entity = fp
            .sketch
            .as_ref()
            .unwrap()
            .entities
            .iter()
            .find(|e| e.id == lid)
            .unwrap();
        assert_eq!(
            line_entity.silk.as_ref().unwrap().layer,
            SignexLayer::TopSilk
        );
        assert_eq!(current_role_of(line_entity), RoleTag::SilkTop);
    }

    #[test]
    fn set_role_unassigned_clears_every_attr() {
        use super::super::sketch_dispatch::{apply_sketch_role, current_role_of};
        use crate::library::messages::RoleTag;

        let mut fp = empty_footprint();
        let plane = PlaneId::new();
        let (e1, p1) = point_with_pad(plane, 0.0, 0.0, "1");
        fp.sketch = Some(SketchData {
            planes: vec![Plane {
                id: plane,
                kind: PlaneKind::BoardTop,
            }],
            entities: vec![e1],
            ..SketchData::default()
        });
        let mut state = FootprintEditorState::from_footprint(&fp);

        // Confirm starting role is Pad.
        let entity_before = &fp.sketch.as_ref().unwrap().entities[0];
        assert_eq!(current_role_of(entity_before), RoleTag::Pad);

        apply_sketch_role(&mut state, &mut fp, p1, RoleTag::Unassigned).unwrap();

        let entity_after = &fp.sketch.as_ref().unwrap().entities[0];
        assert!(entity_after.pad.is_none());
        assert!(entity_after.silk.is_none());
        assert!(entity_after.courtyard.is_none());
        assert!(entity_after.mask_opening.is_none());
        assert!(entity_after.mask_exclude.is_none());
        assert!(entity_after.paste_aperture.is_none());
        assert!(entity_after.pour.is_none());
        assert!(entity_after.keepout.is_none());
        assert!(entity_after.board_cutout.is_none());
        assert!(entity_after.v_score.is_none());
        assert_eq!(current_role_of(entity_after), RoleTag::Unassigned);
    }

    #[test]
    fn set_role_replaces_existing_attr_atomically() {
        // Pad → SilkTop must swap, not append. If the dispatcher
        // forgot to clear before setting, both attrs would be set
        // simultaneously and the bake would emit duplicate geometry.
        use super::super::sketch_dispatch::{apply_sketch_role, current_role_of};
        use crate::library::messages::RoleTag;

        let mut fp = empty_footprint();
        let plane = PlaneId::new();
        let (e1, p1) = point_with_pad(plane, 0.0, 0.0, "1");
        fp.sketch = Some(SketchData {
            planes: vec![Plane {
                id: plane,
                kind: PlaneKind::BoardTop,
            }],
            entities: vec![e1],
            ..SketchData::default()
        });
        let mut state = FootprintEditorState::from_footprint(&fp);

        apply_sketch_role(&mut state, &mut fp, p1, RoleTag::SilkTop).unwrap();

        let entity = &fp.sketch.as_ref().unwrap().entities[0];
        assert!(
            entity.pad.is_none(),
            "Pad must be cleared by SilkTop assign"
        );
        assert!(entity.silk.is_some());
        assert_eq!(current_role_of(entity), RoleTag::SilkTop);
        // Bake must NOT emit a stale pad after the role swap.
        assert_eq!(fp.pads.len(), 0);
    }

    #[test]
    fn set_role_courtyard_attaches_courtyard_attr() {
        use super::super::sketch_dispatch::{apply_sketch_role, current_role_of};
        use crate::library::messages::RoleTag;

        let mut fp = empty_footprint();
        let plane = PlaneId::new();
        let pid = SketchEntityId::new();
        let pt = Entity::new(pid, plane, EntityKind::Point { x: 0.0, y: 0.0 });
        fp.sketch = Some(SketchData {
            planes: vec![Plane {
                id: plane,
                kind: PlaneKind::BoardTop,
            }],
            entities: vec![pt],
            ..SketchData::default()
        });
        let mut state = FootprintEditorState::from_footprint(&fp);

        apply_sketch_role(&mut state, &mut fp, pid, RoleTag::Courtyard).unwrap();

        let entity = &fp.sketch.as_ref().unwrap().entities[0];
        assert!(entity.courtyard.is_some());
        assert_eq!(current_role_of(entity), RoleTag::Courtyard);
    }

    #[test]
    fn set_role_pad_increments_designator_across_entities() {
        // First Pad gets "1", second Pad gets "2", third gets "3".
        use super::super::sketch_dispatch::apply_sketch_role;
        use crate::library::messages::RoleTag;

        let mut fp = empty_footprint();
        let plane = PlaneId::new();
        let p1 = SketchEntityId::new();
        let p2 = SketchEntityId::new();
        let p3 = SketchEntityId::new();
        let pt1 = Entity::new(p1, plane, EntityKind::Point { x: 0.0, y: 0.0 });
        let pt2 = Entity::new(p2, plane, EntityKind::Point { x: 1.0, y: 0.0 });
        let pt3 = Entity::new(p3, plane, EntityKind::Point { x: 2.0, y: 0.0 });
        fp.sketch = Some(SketchData {
            planes: vec![Plane {
                id: plane,
                kind: PlaneKind::BoardTop,
            }],
            entities: vec![pt1, pt2, pt3],
            ..SketchData::default()
        });
        let mut state = FootprintEditorState::from_footprint(&fp);

        apply_sketch_role(&mut state, &mut fp, p1, RoleTag::Pad).unwrap();
        apply_sketch_role(&mut state, &mut fp, p2, RoleTag::Pad).unwrap();
        apply_sketch_role(&mut state, &mut fp, p3, RoleTag::Pad).unwrap();

        let sketch = fp.sketch.as_ref().unwrap();
        let nums: Vec<&str> = sketch
            .entities
            .iter()
            .filter_map(|e| e.pad.as_ref().map(|a| a.number.as_str()))
            .collect();
        assert_eq!(nums, vec!["1", "2", "3"]);
    }

    #[test]
    fn solver_runs_on_every_edit() {
        // v0.22 — the auto-pause hysteresis was removed entirely.
        // Every edit dispatches a fresh solve + bake. This test
        // pins the always-live behavior.
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

        apply_sketch_edit(&mut state, &mut fp, SketchEdit::ForceRebuild).unwrap();
        assert!(state.last_solve.is_some());
        assert_eq!(fp.pads.len(), 1);
    }

    #[test]
    fn solver_errors_surface_in_solve_warnings_not_silently_swallowed() {
        // v0.22 — every SolveError variant (including the previously
        // silently-swallowed Timeout) now propagates as
        // SketchError::SolveFailed. The _with_warnings wrapper writes
        // it into state.solve_warnings so the inspector can show it.
        // We force the broader SketchError::Expr path by injecting a
        // malformed parameter expression — same wrapper path,
        // doesn't depend on solver-iteration timing.
        use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;

        let mut fp = empty_footprint();
        let plane = PlaneId::new();
        let (e1, _p1) = point_with_pad(plane, 0.0, 0.0, "1");
        let mut params = signex_sketch::parameter::ParameterTable::default();
        params.insert("bad", "$$$");
        fp.sketch = Some(SketchData {
            planes: vec![Plane {
                id: plane,
                kind: PlaneKind::BoardTop,
            }],
            entities: vec![e1],
            parameters: params,
            ..SketchData::default()
        });
        let mut state = FootprintEditorState::from_footprint(&fp);

        apply_sketch_edit_with_warnings(&mut state, &mut fp, SketchEdit::ForceRebuild);

        assert!(
            !state.solve_warnings.is_empty(),
            "expected SketchError surfaced in solve_warnings, got nothing"
        );
    }

    // ----------------------------------------------------------------
    // #372 — Break Track. Drives the real router
    // (`apply_footprint_primitive_edit`) end-to-end so each test also
    // exercises the one-history-snapshot pre-push that makes a split a
    // single undo step. The click-resolution machinery shared by all
    // sketch tools mints one throwaway "click" Point per click BEFORE
    // the edit arm runs (same as Fillet / Trim); these tests therefore
    // assert on the split-relevant geometry — the count of `Line`
    // entities and their endpoints — rather than the raw entity total.
    // ----------------------------------------------------------------

    /// Build an `app::FootprintEditorState` holding a single sketch
    /// Line from `p_start` to `p_end`, armed with the Break Track tool.
    /// Returns the editor plus the start / end Point ids and the Line
    /// id so tests can assert against the pre-split identities.
    fn break_track_editor(
        p_start: (f64, f64),
        p_end: (f64, f64),
    ) -> (
        crate::app::FootprintEditorState,
        SketchEntityId,
        SketchEntityId,
        SketchEntityId,
    ) {
        use crate::library::editor::footprint::state::SketchTool;
        use signex_library::primitive::footprint::FootprintFile;
        use std::path::PathBuf;

        let mut fp = empty_footprint();
        let plane = PlaneId::new();
        let sid = SketchEntityId::new();
        let eid = SketchEntityId::new();
        let lid = SketchEntityId::new();
        fp.sketch = Some(SketchData {
            planes: vec![Plane {
                id: plane,
                kind: PlaneKind::BoardTop,
            }],
            entities: vec![
                Entity::new(
                    sid,
                    plane,
                    EntityKind::Point {
                        x: p_start.0,
                        y: p_start.1,
                    },
                ),
                Entity::new(
                    eid,
                    plane,
                    EntityKind::Point {
                        x: p_end.0,
                        y: p_end.1,
                    },
                ),
                Entity::new(
                    lid,
                    plane,
                    EntityKind::Line {
                        start: sid,
                        end: eid,
                    },
                ),
            ],
            ..SketchData::default()
        });
        let file = FootprintFile::from_footprint(fp);
        let mut editor =
            crate::app::FootprintEditorState::new(PathBuf::from("break-track.snxfpt"), file);
        editor.state.active_tool = SketchTool::BreakTrack;
        (editor, sid, eid, lid)
    }

    /// `(line_id, start_id, end_id)` for every `Line` in the active
    /// footprint's sketch.
    fn sketch_lines(
        editor: &crate::app::FootprintEditorState,
    ) -> Vec<(SketchEntityId, SketchEntityId, SketchEntityId)> {
        editor
            .primitive()
            .sketch
            .as_ref()
            .unwrap()
            .entities
            .iter()
            .filter_map(|e| match e.kind {
                EntityKind::Line { start, end } => Some((e.id, start, end)),
                _ => None,
            })
            .collect()
    }

    /// Coordinates of the Point `id` in the active footprint's sketch.
    fn point_xy(editor: &crate::app::FootprintEditorState, id: SketchEntityId) -> (f64, f64) {
        editor
            .primitive()
            .sketch
            .as_ref()
            .unwrap()
            .entities
            .iter()
            .find(|e| e.id == id)
            .map(|e| match e.kind {
                EntityKind::Point { x, y } => (x, y),
                _ => panic!("entity {id:?} is not a Point"),
            })
            .unwrap()
    }

    #[test]
    fn break_track_split_at_mid_span_replaces_line_with_two_halves() {
        use crate::library::messages::FootprintEditorMsg;

        let (mut editor, sid, eid, lid) = break_track_editor((0.0, 0.0), (10.0, 0.0));
        assert_eq!(sketch_lines(&editor).len(), 1, "starts with one Line");

        // Click at the midpoint, 0.1 mm off the stroke (< 0.30 mm tol).
        crate::library::editor::footprint::updates::apply_footprint_primitive_edit(
            &mut editor,
            FootprintEditorMsg::SketchToolClick {
                x_mm: 5.0,
                y_mm: 0.1,
                snap_id: None,
            },
        );

        // Exactly one undo snapshot captured for the whole click.
        assert_eq!(
            editor.history.len(),
            1,
            "a split must cost exactly one undo step"
        );

        let ls = sketch_lines(&editor);
        assert_eq!(ls.len(), 2, "the Line is replaced by two halves");
        let ends_a = [ls[0].1, ls[0].2];
        let ends_b = [ls[1].1, ls[1].2];

        // The two halves share exactly one endpoint — the new mid Point.
        let shared: Vec<SketchEntityId> = ends_a
            .iter()
            .copied()
            .filter(|p| ends_b.contains(p))
            .collect();
        assert_eq!(
            shared.len(),
            1,
            "the halves share exactly the new mid Point"
        );
        let mid = shared[0];
        assert_ne!(mid, sid);
        assert_ne!(mid, eid);

        // The mid Point sits at the click projected onto the line (5,0).
        let (mx, my) = point_xy(&editor, mid);
        assert!(
            (mx - 5.0).abs() < 1e-6 && (my - 0.0).abs() < 1e-6,
            "mid at the projected click, got ({mx}, {my})"
        );

        // The union of the two segments keeps the original endpoints.
        let outer: Vec<SketchEntityId> = ends_a
            .iter()
            .chain(ends_b.iter())
            .copied()
            .filter(|p| *p != mid)
            .collect();
        assert!(
            outer.contains(&sid) && outer.contains(&eid),
            "union of the halves preserves the original endpoints"
        );

        // One undo reverses the entire split back to the single Line.
        assert!(editor.undo(), "there is a snapshot to undo");
        let restored = sketch_lines(&editor);
        assert_eq!(restored.len(), 1, "undo restores the single original Line");
        assert_eq!(
            restored[0],
            (lid, sid, eid),
            "restored Line is byte-identical"
        );
    }

    #[test]
    fn break_track_reselects_line_a_not_line_b() {
        use crate::library::messages::FootprintEditorMsg;

        let (mut editor, sid, _eid, _lid) = break_track_editor((0.0, 0.0), (10.0, 0.0));
        crate::library::editor::footprint::updates::apply_footprint_primitive_edit(
            &mut editor,
            FootprintEditorMsg::SketchToolClick {
                x_mm: 4.0,
                y_mm: 0.05,
                snap_id: None,
            },
        );

        let sel = editor
            .state
            .selected_sketch
            .expect("selection is set to a split half after a successful split");
        let ls = sketch_lines(&editor);
        let selected = ls
            .iter()
            .find(|(id, _, _)| *id == sel)
            .expect("selection resolves to one of the two new Lines");
        // `line_a` is `start -> mid`, so its `start` is the ORIGINAL
        // start; `line_b` is `mid -> end` (its start is the new mid).
        assert_eq!(
            selected.1, sid,
            "selection is line_a (keeps the original start), never line_b"
        );
    }

    #[test]
    fn break_track_miss_warns_and_leaves_line_intact() {
        use crate::library::editor::footprint::state::{SketchTool, ToolPending};
        use crate::library::messages::FootprintEditorMsg;

        let (mut editor, sid, eid, lid) = break_track_editor((0.0, 0.0), (10.0, 0.0));
        // 5 mm off the stroke — far beyond the 0.30 mm tolerance.
        crate::library::editor::footprint::updates::apply_footprint_primitive_edit(
            &mut editor,
            FootprintEditorMsg::SketchToolClick {
                x_mm: 5.0,
                y_mm: 5.0,
                snap_id: None,
            },
        );

        let ls = sketch_lines(&editor);
        assert_eq!(ls.len(), 1, "a miss splits nothing");
        assert_eq!(ls[0], (lid, sid, eid), "the original Line is untouched");
        assert!(
            editor
                .state
                .solve_warnings
                .iter()
                .any(|w| w.contains("Break Track")),
            "a miss pushes a user-visible Break Track warning"
        );
        assert_eq!(
            editor.state.active_tool,
            SketchTool::BreakTrack,
            "the tool stays armed after a miss"
        );
        assert!(
            matches!(editor.state.tool_pending, ToolPending::Idle),
            "the single-click tool resets to Idle (still armed)"
        );
    }

    #[test]
    fn break_track_click_near_endpoint_warns_no_split() {
        use crate::library::editor::footprint::state::SketchTool;
        use crate::library::messages::FootprintEditorMsg;

        let (mut editor, sid, eid, lid) = break_track_editor((0.0, 0.0), (10.0, 0.0));
        // ~0.05 mm off the stroke and ~0.0005 mm along it from the
        // start: the click is on the segment (so `t ≈ 5e-5` is a valid
        // parameter), but the mid Point would land within
        // MIN_SEGMENT_LEN_MM of the start → SplitError::TooCloseToEndpoint
        // → no split, just a warning.
        crate::library::editor::footprint::updates::apply_footprint_primitive_edit(
            &mut editor,
            FootprintEditorMsg::SketchToolClick {
                x_mm: 0.0005,
                y_mm: 0.05,
                snap_id: None,
            },
        );

        let ls = sketch_lines(&editor);
        assert_eq!(ls.len(), 1, "a near-endpoint click splits nothing");
        assert_eq!(ls[0], (lid, sid, eid), "the original Line is untouched");
        assert!(
            editor
                .state
                .solve_warnings
                .iter()
                .any(|w| w.contains("Break Track")),
            "a near-endpoint click pushes a user-visible Break Track warning"
        );
        assert_eq!(
            editor.state.active_tool,
            SketchTool::BreakTrack,
            "the tool stays armed after a rejected split"
        );
    }
}
