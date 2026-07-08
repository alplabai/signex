//! Footprint sketch updates — sketch↔pad bridge (roles / profile / corner radius) concern.
//!
//! Carved out of the monolithic `sketch::apply` (ADR-0001 D1/D2). Arm
//! bodies are moved verbatim; each keeps its own inner `use`s.

use crate::library::messages::PrimitiveEditorMsg;

pub(super) fn apply(editor: &mut crate::app::FootprintEditorState, msg: PrimitiveEditorMsg) {
    match msg {
        PrimitiveEditorMsg::FootprintSketchSetRole { id, role } => {
            use crate::library::editor::footprint::sketch_dispatch::apply_sketch_role_with_warnings;
            use crate::library::editor::footprint::state::EditorPad;
            use signex_library::primitive::footprint::{
                LayerId, PadKind as LibPadKind, PadShape as LibPadShape,
            };

            // v0.27 — the Role=Pad-on-a-Line case is rewritten to
            // MakePadFromProfile at the top of
            // `apply_footprint_primitive_edit`, so this arm only
            // sees Point-targeted Pad assignments + every other
            // role. PadAttr is Point-only on the schema side, so
            // dispatching to `apply_sketch_role_with_warnings` is
            // always meaningful from here on.
            editor.with_parts(|state, primitive| {
                apply_sketch_role_with_warnings(state, primitive, id, role);
            });

            // Diff `state.pads` against the entity's new role so the
            // canvas's pad list mirrors role assignments. Per-entity
            // diff (rather than full rebuild from `primitive.pads`)
            // preserves `sketch_entity_id` + `corner_entity_ids` on
            // previously auto-minted Pads-mode pads.
            let entity_has_pad = editor
                .primitive()
                .sketch
                .as_ref()
                .and_then(|s| s.entities.iter().find(|e| e.id == id))
                .map(|e| e.pad.is_some())
                .unwrap_or(false);
            let existing_idx = editor
                .state
                .pads
                .iter()
                .position(|p| p.sketch_entity_id == Some(id));
            match (entity_has_pad, existing_idx) {
                (true, None) => {
                    use signex_sketch::entity::EntityKind;
                    let (x, y, number) = editor
                        .primitive()
                        .sketch
                        .as_ref()
                        .and_then(|s| s.entities.iter().find(|e| e.id == id))
                        .map(|e| {
                            let (x, y) = match e.kind {
                                EntityKind::Point { x, y } => (x, y),
                                _ => (0.0, 0.0),
                            };
                            let num = e.pad.as_ref().map(|a| a.number.clone()).unwrap_or_default();
                            (x, y, num)
                        })
                        .unwrap_or((0.0, 0.0, String::new()));
                    editor.state.pads.push(EditorPad {
                        number,
                        position_mm: (x, y),
                        size_mm: (1.0, 1.0),
                        kind: LibPadKind::Smd,
                        shape: LibPadShape::Rect,
                        layers: vec![LayerId::new("Top Layer")],
                        sketch_entity_id: Some(id),
                        corner_entity_ids: None,
                        rotation_deg: 0.0,
                        drill_diameter_mm: None,
                        stack: crate::library::editor::footprint::state::PadStackUi::default(),
                        feature_top: signex_sketch::attr::PadFeature::None,
                        feature_bottom: signex_sketch::attr::PadFeature::None,
                        testpoint: signex_sketch::attr::TestpointFlags::default(),
                        template: String::new(),
                        template_library: String::new(),
                        electrical_type: signex_sketch::attr::ElectricalType::Load,
                        net: String::new(),
                        locked: false,
                        hole_tolerance_plus_mm: None,
                        hole_tolerance_minus_mm: None,
                        hole_rotation_deg: None,
                        copper_offset_x_mm: None,
                        copper_offset_y_mm: None,
                        shape_params: crate::library::editor::footprint::state::ShapeParamMap::new(
                        ),
                    });
                }
                (false, Some(idx)) => {
                    editor.state.pads.remove(idx);
                    if editor.state.selected_pad == Some(idx) {
                        editor.state.selected_pad = None;
                    } else if let Some(sel) = editor.state.selected_pad {
                        if sel > idx {
                            editor.state.selected_pad = Some(sel - 1);
                        }
                    }
                }
                _ => {}
            }
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        PrimitiveEditorMsg::FootprintSketchMakePadFromProfile => {
            // v0.22 Phase D4 — convert the closed-loop profile that
            // includes the currently-selected Line into a
            // `PadShape::Custom(SketchProfile)` pad.
            //
            // Walk: start from the selected Line, use
            // `signex_bake::profile::trace_closed_profile` to chase
            // the unique-incident-edge cycle in the sketch. On
            // success, compute the centroid of the traced vertices,
            // mint a centre `Point` there, and attach a `PadAttr`
            // whose `shape` is `Custom(SketchProfile{source: vec![
            // seed_line_id]})`. The bake re-walks the loop on the
            // next solve and emits a `LibPadShape::Custom` polygon.
            //
            // Designator: `next_pad_num` from existing `PadAttr`
            // entities, identical pattern to
            // `apply_sketch_role(.., RoleTag::Pad)` for ordering
            // consistency.
            //
            // Fail modes (silent except for warning push):
            // - No Line selected → "select a Line first".
            // - Line is not part of a closed loop → "loop is open
            //   or branches".
            // - `last_solve` is None (no solve has run yet) → ask
            //   user to interact briefly so a solve fires, then
            //   retry. (Auto-mint paths on entry to Sketch mode
            //   already trigger a solve, so this is rare.)
            use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
            use crate::library::editor::footprint::sketch_mode::SketchEdit;
            use signex_sketch::attr::{
                CustomPadShape, PadAttr, PadKind, PadShape, PadSide, PasteAperturePattern,
            };
            use signex_sketch::entity::{Entity, EntityKind};
            use signex_sketch::id::SketchEntityId;
            use signex_sketch::plane::{Plane, PlaneId, PlaneKind};

            // v0.27 — walk the full sketch selection (primary +
            // secondary + extras) for the first Line. The
            // closed-loop walker doesn't care which seed edge it
            // gets — any Line on the loop seeds the trace. Falls
            // back to scanning every sketch Line when nothing
            // suitable is selected, so the action also works on a
            // bare "select-nothing-and-click-Make-Pad" workflow.
            let line_id: SketchEntityId = {
                let sketch_lookup = editor.primitive().sketch.as_ref();
                let kind_of = |id: SketchEntityId| -> Option<EntityKind> {
                    sketch_lookup
                        .and_then(|s| s.entities.iter().find(|e| e.id == id))
                        .map(|e| e.kind.clone())
                };
                let selection: Vec<SketchEntityId> = editor
                    .state
                    .selected_sketch
                    .into_iter()
                    .chain(editor.state.selected_sketch_secondary.into_iter())
                    .chain(editor.state.selected_sketch_extra.iter().copied())
                    .collect();

                // First pass — Line directly in the selection.
                let direct_line = selection
                    .iter()
                    .find(|id| matches!(kind_of(**id), Some(EntityKind::Line { .. })))
                    .copied();
                // Second pass — a selected Point's incident Line.
                let incident_line = selection.iter().find_map(|id| {
                    if matches!(kind_of(*id), Some(EntityKind::Point { .. })) {
                        sketch_lookup.and_then(|s| {
                            s.entities
                                .iter()
                                .find(|e| match e.kind {
                                    EntityKind::Line { start, end } => start == *id || end == *id,
                                    _ => false,
                                })
                                .map(|e| e.id)
                        })
                    } else {
                        None
                    }
                });
                // Third pass — any sketch Line at all.
                let any_line = sketch_lookup.and_then(|s| {
                    s.entities
                        .iter()
                        .find(|e| matches!(e.kind, EntityKind::Line { .. }))
                        .map(|e| e.id)
                });

                match direct_line.or(incident_line).or(any_line) {
                    Some(id) => id,
                    None => {
                        editor.state.solve_warnings.push(
                            "Make Pad from Profile: no Lines in the sketch — draw a closed shape first"
                                .into(),
                        );
                        editor.canvas_cache.clear();
                        return;
                    }
                }
            };

            // Walk the loop to compute the centroid; needs a fresh
            // solve so vertex positions are accurate.
            let solve = match editor.state.last_solve.as_ref() {
                Some(s) => s,
                None => {
                    editor.state.solve_warnings.push(
                        "Make Pad from Profile: no solve has run yet — interact briefly to trigger a solve, then retry"
                            .into(),
                    );
                    editor.canvas_cache.clear();
                    return;
                }
            };
            let sketch_for_walk = match editor.primitive().sketch.as_ref() {
                Some(s) => s,
                None => return,
            };

            let trace = signex_bake::profile::trace_closed_profile(sketch_for_walk, solve, line_id);
            let vertices = match trace {
                Ok(v) if v.len() >= 3 => v,
                Ok(_) => {
                    editor.state.solve_warnings.push(
                        "Make Pad from Profile: traced loop has fewer than 3 vertices".into(),
                    );
                    editor.canvas_cache.clear();
                    return;
                }
                Err(e) => {
                    editor.state.solve_warnings.push(format!(
                        "Make Pad from Profile: loop walk failed ({e:?}) — the loop must be closed and non-branching"
                    ));
                    editor.canvas_cache.clear();
                    return;
                }
            };
            // v0.27 — area-weighted centroid + axis-aligned bbox of
            // the closed-loop polygon. The arithmetic mean of vertex
            // positions only matches the geometric centroid for
            // regular polygons; for an arbitrary triangle / outline
            // it lands biased toward whichever side has the most
            // densely-spaced vertices (which is why the user saw
            // the pad mint near a corner instead of inside the
            // shape). The shoelace centroid is the proper EDA
            // answer — pad sits at the geometric middle of its own
            // copper outline.
            let n_v = vertices.len();
            let mut signed_area = 0.0_f64;
            let mut cx_acc = 0.0_f64;
            let mut cy_acc = 0.0_f64;
            for i in 0..n_v {
                let (x0, y0) = (vertices[i][0], vertices[i][1]);
                let (x1, y1) = (vertices[(i + 1) % n_v][0], vertices[(i + 1) % n_v][1]);
                let cross = x0 * y1 - x1 * y0;
                signed_area += cross;
                cx_acc += (x0 + x1) * cross;
                cy_acc += (y0 + y1) * cross;
            }
            let area = signed_area * 0.5;
            let (cx, cy) = if area.abs() > 1e-12 {
                let s = 1.0 / (6.0 * area);
                (cx_acc * s, cy_acc * s)
            } else {
                // Degenerate polygon — fall back to mean.
                let n = n_v as f64;
                (
                    vertices.iter().map(|p| p[0]).sum::<f64>() / n,
                    vertices.iter().map(|p| p[1]).sum::<f64>() / n,
                )
            };
            // Axis-aligned bbox — drives `size_x_expr` / `size_y_expr`
            // so the synced `state.pads` row is at least bbox-sized
            // (instead of the default 1mm × 1mm). Polygon-shape
            // rendering on the editor canvas is a follow-up.
            let mut min_x = f64::INFINITY;
            let mut min_y = f64::INFINITY;
            let mut max_x = f64::NEG_INFINITY;
            let mut max_y = f64::NEG_INFINITY;
            for p in &vertices {
                if p[0] < min_x {
                    min_x = p[0];
                }
                if p[1] < min_y {
                    min_y = p[1];
                }
                if p[0] > max_x {
                    max_x = p[0];
                }
                if p[1] > max_y {
                    max_y = p[1];
                }
            }
            let bbox_w = (max_x - min_x).max(0.05);
            let bbox_h = (max_y - min_y).max(0.05);

            // Plane: reuse the seed Line's plane so the new pad
            // entity ends up on the same one.
            let plane_id = sketch_for_walk
                .entities
                .iter()
                .find(|e| e.id == line_id)
                .map(|e| e.plane)
                .unwrap_or_else(|| {
                    sketch_for_walk
                        .planes
                        .first()
                        .map(|p| p.id)
                        .unwrap_or_else(PlaneId::new)
                });
            // Ensure plane exists (defensive — almost always already
            // in `sketch.planes`).
            let _ = Plane {
                id: plane_id,
                kind: PlaneKind::BoardTop,
            };

            // Next pad designator from existing pad attrs.
            let next_pad_num = sketch_for_walk
                .entities
                .iter()
                .filter_map(|e| e.pad.as_ref())
                .filter_map(|attr| attr.number.parse::<u32>().ok())
                .max()
                .unwrap_or(0)
                + 1;

            let centre_id = SketchEntityId::new();
            let mut centre = Entity::new(centre_id, plane_id, EntityKind::Point { x: cx, y: cy });
            centre.pad = Some(PadAttr {
                number: next_pad_num.to_string(),
                kind: PadKind::Smd,
                side: PadSide::Top,
                shape: PadShape::Custom(CustomPadShape::SketchProfile {
                    source: vec![line_id],
                }),
                size_x_expr: format!("{:.3}mm", bbox_w),
                size_y_expr: format!("{:.3}mm", bbox_h),
                rotation_expr: None,
                offset_x_expr: None,
                offset_y_expr: None,
                drill: None,
                mask_margin_expr: None,
                paste_margin_expr: None,
                paste_apertures: PasteAperturePattern::Single,
                ..PadAttr::default()
            });
            editor.with_parts(|state, primitive| {
                apply_sketch_edit_with_warnings(state, primitive, SketchEdit::AddEntity(centre));
            });
            // v0.27 — pivot the selection onto the new pad's centre
            // Point. Without this, the Role dropdown still reads
            // "Unassigned" because the user's prior selection (the
            // Line we walked) has no PadAttr — the new PadAttr lives
            // on the freshly-minted centre. Clearing extras avoids
            // a confusing "primary is the centre but extras still
            // point at the loop's lines" state right after Make Pad.
            editor.state.selected_sketch = Some(centre_id);
            editor.state.selected_sketch_secondary = None;
            editor.state.selected_sketch_extra.clear();
            editor.dirty = true;
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintSketchUnlinkCornerRadius { arc_entity_id } => {
            // v0.24 Phase 3 (Track A3) — split a RoundRect pad's
            // shared `corner_r_<slug>` parameter into a per-corner
            // override for the right-clicked Arc.
            //
            // Lookup chain:
            //   1. Walk every EditorPad to find the one whose
            //      `shape_params` contains a `corner_r_<corner>_arc`
            //      key whose value (UUID slug) matches `arc_entity_id`.
            //   2. From that match, derive the corner key
            //      (`corner_r_ne` / `_se` / `_sw` / `_nw`).
            //   3. Mint a fresh parameter `<shared_name>_<corner>`,
            //      copy the current shared expression as its value,
            //      and bind the corner key on `pad.shape_params`.
            //   4. Trigger a `ForceRebuild` so the solver re-runs and
            //      the bake reflects the new parametric link.
            //
            // Defensive: arc not part of any pad → tracing::warn +
            // no-op. Pad has no shared `corner_r` binding (e.g.
            // legacy data) → tracing::warn + no-op.
            use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
            use crate::library::editor::footprint::sketch_mode::SketchEdit;

            let arc_id_str = arc_entity_id.0.simple().to_string();

            // Locate the pad + corner this arc belongs to.
            let pad_corner: Option<(usize, &'static str)> =
                editor.state.pads.iter().enumerate().find_map(|(idx, pad)| {
                    let arc_keys: [(&str, &str); 4] = [
                        ("corner_r_ne_arc", "corner_r_ne"),
                        ("corner_r_se_arc", "corner_r_se"),
                        ("corner_r_sw_arc", "corner_r_sw"),
                        ("corner_r_nw_arc", "corner_r_nw"),
                    ];
                    for (sidecar_key, corner_key) in arc_keys {
                        if pad.shape_params.get(sidecar_key).map(|s| s.as_str())
                            == Some(arc_id_str.as_str())
                        {
                            return Some((idx, corner_key));
                        }
                    }
                    None
                });

            let Some((pad_idx, corner_key)) = pad_corner else {
                tracing::warn!(
                    target: "signex::v024",
                    "FootprintSketchUnlinkCornerRadius: arc {arc_entity_id:?} doesn't belong \
                     to any pad's shape_params; ignoring"
                );
                return;
            };

            // Already unlinked → no-op (idempotent).
            if editor.state.pads[pad_idx]
                .shape_params
                .contains_key(corner_key)
            {
                tracing::warn!(
                    target: "signex::v024",
                    "FootprintSketchUnlinkCornerRadius: corner {corner_key} on pad {pad_idx} \
                     is already unlinked; ignoring"
                );
                return;
            }

            // Resolve the shared parameter name + current value.
            let shared_name = match editor.state.pads[pad_idx]
                .shape_params
                .get("corner_r")
                .cloned()
            {
                Some(n) => n,
                None => {
                    tracing::warn!(
                        target: "signex::v024",
                        "FootprintSketchUnlinkCornerRadius: pad {pad_idx} has no shared \
                         corner_r binding; ignoring"
                    );
                    return;
                }
            };
            let shared_value = editor
                .primitive()
                .sketch
                .as_ref()
                .and_then(|s| s.parameters.get_raw(&shared_name).map(str::to_string))
                .unwrap_or_default();

            // Mint the per-corner parameter name. Use the corner_key
            // suffix (e.g. `_ne`) appended to the shared name's slug
            // so the per-corner names cluster together in the
            // parameter table for inspection.
            let corner_suffix = corner_key.strip_prefix("corner_r_").unwrap_or(corner_key);
            let new_param_name = format!("{shared_name}_{corner_suffix}");

            // Apply the rewrite. push_history is already captured at
            // the top of this dispatcher arm via mutates_footprint_state.
            editor.with_parts(|state, primitive| {
                // Mint the new parameter on the sketch.
                if let Some(sketch) = primitive.sketch.as_mut() {
                    sketch
                        .parameters
                        .insert(new_param_name.clone(), shared_value.clone());
                }
                // Record the per-corner override on the pad.
                if let Some(pad) = state.pads.get_mut(pad_idx) {
                    pad.shape_params
                        .insert(corner_key.to_string(), new_param_name.clone());
                }
                // ForceRebuild → solver re-runs, bake regenerates pad
                // geometry from the (now per-corner-aware) parameters.
                apply_sketch_edit_with_warnings(state, primitive, SketchEdit::ForceRebuild);
            });
            editor.dirty = true;
            editor.canvas_cache.clear();
        }
        _ => unreachable!("non-sketch↔pad bridge (roles / profile / corner radius) sketch variant routed to sketch_pad_bridge.rs"),
    }
}
