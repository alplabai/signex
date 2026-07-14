use super::footprint_summaries::{
    build_over_constraint_summaries, build_sketch_entity_summary, footprint_pad_kind_label,
    footprint_pad_shape_label,
};

/// v0.14.2 — project the active `.snxfpt` editor's data into a
/// panel-side snapshot. Mirrors `build_symbol_editor_panel_ctx`.
pub(super) fn build_footprint_editor_panel_ctx(
    app: &super::super::Signex,
) -> Option<crate::panels::FootprintEditorPanelContext> {
    use crate::library::editor::footprint::state::EditorMode;
    use crate::panels::{
        FootprintEditorPanelContext, FootprintModeKind, FootprintPadSummary,
        FootprintSketchEntitySummary, FootprintSolveSummary, OverConstraintSummary,
    };

    let active = app.document_state.tabs.get(app.document_state.active_tab)?;
    let path = active.kind.as_footprint_editor()?.clone();
    let editor = app.document_state.footprint_editors.get(&path)?;

    let mode_kind = match editor.state.mode {
        EditorMode::Normal => FootprintModeKind::Pads,
        EditorMode::Sketch => FootprintModeKind::Sketch,
        EditorMode::View3d => FootprintModeKind::View3d,
    };

    let pad_count = editor.primitive().pads.len();
    let (sketch_entity_count, sketch_constraint_count) = match editor.primitive().sketch.as_ref() {
        Some(s) => (s.entities.len(), s.constraints.len()),
        None => (0, 0),
    };

    let last_solve = editor.state.last_solve.as_ref().map(|out| {
        let over_constraints = build_over_constraint_summaries(editor.primitive(), out);
        FootprintSolveSummary {
            iterations: out.result.iterations,
            elapsed_ms: out.result.elapsed_ms,
            final_residual_norm: out.result.final_residual_norm,
            over_constraint_count: out.over_constraints.len(),
            over_constraints,
        }
    });

    // Pad summary — populated only when in Pads mode AND a pad is
    // selected. Avoids confusing the user with a stale pad selection
    // surfacing while they're authoring sketch entities.
    let selected_pad = if mode_kind == FootprintModeKind::Pads {
        editor.state.selected_pad.and_then(|idx| {
            editor.state.pads.get(idx).map(|pad| {
                use crate::library::editor::footprint::state::PadSide;
                // Side derived from the first layer's prefix. THT/NPT
                // pads carry both copper sides → All. Otherwise Top
                // for F.* and Bottom for B.*.
                let side = if pad.layers.iter().any(|l| l.as_str().starts_with("*.")) {
                    PadSide::All
                } else if pad
                    .layers
                    .first()
                    .map(|l| l.as_str().starts_with("B."))
                    .unwrap_or(false)
                {
                    PadSide::Bottom
                } else {
                    PadSide::Top
                };
                FootprintPadSummary {
                    idx,
                    number: pad.number.clone(),
                    kind_label: footprint_pad_kind_label(pad),
                    shape_label: footprint_pad_shape_label(pad),
                    size_mm: [pad.size_mm.0, pad.size_mm.1],
                    position_mm: [pad.position_mm.0, pad.position_mm.1],
                    rotation_deg: pad.rotation_deg,
                    layer_count: pad.layers.len(),
                    has_drill: pad.drill_diameter_mm.is_some(),
                    side,
                    shape: pad.shape.clone(),
                    kind: pad.kind,
                    drill_diameter_mm: pad.drill_diameter_mm,
                    stack: pad.stack.clone(),
                    feature_top: pad.feature_top,
                    feature_bottom: pad.feature_bottom,
                    testpoint: pad.testpoint,
                    template: pad.template.clone(),
                    template_library: pad.template_library.clone(),
                    electrical_type: pad.electrical_type,
                    net: pad.net.clone(),
                    locked: pad.locked,
                    hole_tolerance_plus_mm: pad.hole_tolerance_plus_mm,
                    hole_tolerance_minus_mm: pad.hole_tolerance_minus_mm,
                    hole_rotation_deg: pad.hole_rotation_deg,
                    copper_offset_x_mm: pad.copper_offset_x_mm,
                    copper_offset_y_mm: pad.copper_offset_y_mm,
                }
            })
        })
    } else {
        None
    };

    // Sketch entity summary — populated only when in Sketch mode AND
    // an entity is selected.
    let selected_sketch_entity = if mode_kind == FootprintModeKind::Sketch {
        editor
            .state
            .selected_sketch
            .and_then(|id| build_sketch_entity_summary(editor, id))
    } else {
        None
    };

    // v0.24 Phase 3 (Track A2) — surface the selected pad's
    // `shape_params` bindings so the Properties panel can render a
    // "Corner radius" / "Diameter" row reading the live sketch
    // parameter expression. Empty when the selected pad has no
    // bindings (e.g. Rect/Oval shapes whose geometry is bbox-only) or
    // when no pad is selected.
    let selected_pad_shape_params: Vec<crate::panels::PadShapeParamSummary> =
        if mode_kind == FootprintModeKind::Pads {
            editor
                .state
                .selected_pad
                .and_then(|idx| editor.state.pads.get(idx))
                .map(|pad| {
                    let parameters = editor.primitive().sketch.as_ref().map(|s| &s.parameters);
                    let mut entries: Vec<crate::panels::PadShapeParamSummary> = pad
                        .shape_params
                        .iter()
                        .filter_map(|(key, parameter_name)| {
                            // v0.24 Phase 3 — Sidecar keys ending
                            // in `_arc` map a corner key (e.g.
                            // `corner_r_ne`) to the matching Arc
                            // entity ID, NOT a sketch parameter.
                            // Filter them out so they don't render
                            // as Properties rows.
                            //
                            // v0.24 Track A6 — Chamfered pads register
                            // `chamfer_<corner>_anchor1` /
                            // `..._anchor2` sidecar keys with the
                            // anchor Point UUIDs as values. These are
                            // referenced by a future Unlink-chamfer
                            // action; like `_arc`, they're not sketch
                            // parameters so we skip them here.
                            if key.ends_with("_arc")
                                || key.ends_with("_anchor")
                                || key.ends_with("_anchor1")
                                || key.ends_with("_anchor2")
                            {
                                return None;
                            }
                            // v0.24 Track A5 — Oval pads store anchor
                            // / arc-centre / Line / Arc entity IDs
                            // under `oval_{anchor,centre,line,arc}_*`
                            // sidecar keys so the delete sweep can
                            // pick them up via `Uuid::parse_str`.
                            // None of these surface as user-editable
                            // Properties rows.
                            if key.starts_with("oval_") {
                                return None;
                            }
                            let label = match key.as_str() {
                                "corner_r" => "Corner radius".to_string(),
                                "diameter" => "Diameter".to_string(),
                                "width" => "Width".to_string(),
                                "height" => "Height".to_string(),
                                "chamfer_len" => "Chamfer length".to_string(),
                                "corner_r_ne" => "Corner radius (NE)".to_string(),
                                "corner_r_se" => "Corner radius (SE)".to_string(),
                                "corner_r_sw" => "Corner radius (SW)".to_string(),
                                "corner_r_nw" => "Corner radius (NW)".to_string(),
                                _ => key.clone(),
                            };
                            let current_expr = parameters
                                .and_then(|p| p.get_raw(parameter_name))
                                .unwrap_or("")
                                .to_string();
                            Some(crate::panels::PadShapeParamSummary {
                                key: key.clone(),
                                label,
                                parameter_name: parameter_name.clone(),
                                current_expr,
                            })
                        })
                        .collect();
                    // Sort by label so the Properties panel renders the
                    // rows in a stable order across rebuilds (HashMap
                    // iteration is unstable). "Corner radius" before
                    // "Corner radius (NE)" before "(NW)" etc — the
                    // alphabetic order of the labels gives the right
                    // grouping for the Fusion-parity layout.
                    entries.sort_by(|a, b| a.label.cmp(&b.label));
                    entries
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        };

    // v0.14.2 — discover every `.snxfpt` sibling inside the
    // containing `.snxlib`'s `footprints/` directory. Walks the
    // active footprint's path ancestors looking for a `.snxlib`
    // file, then reads the sibling `footprints/` dir. Best-effort:
    // failures (no library, missing dir, read error) just yield an
    // empty siblings vec — the panel handles that gracefully.
    let mut library_siblings: Vec<crate::panels::FootprintLibSibling> = Vec::new();
    let mut library_stem: Option<String> = None;
    let snxlib_ancestor = path.ancestors().find(|p| {
        p.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("snxlib"))
            .unwrap_or(false)
    });
    if let Some(snxlib_path) = snxlib_ancestor {
        library_stem = snxlib_path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string());
        let footprints_dir = snxlib_path.parent().map(|d| d.join("footprints"));
        if let Some(dir) = footprints_dir {
            if let Ok(entries) = std::fs::read_dir(&dir) {
                let mut paths: Vec<std::path::PathBuf> = entries
                    .filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| {
                        p.extension()
                            .and_then(|e| e.to_str())
                            .map(|e| e.eq_ignore_ascii_case("snxfpt"))
                            .unwrap_or(false)
                    })
                    .collect();
                paths.sort();
                for p in paths {
                    let display_name = p
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| {
                            p.file_name()
                                .map(|f| f.to_string_lossy().into_owned())
                                .unwrap_or_default()
                        });
                    let is_active = p == path;
                    library_siblings.push(crate::panels::FootprintLibSibling {
                        path: p,
                        display_name,
                        is_active,
                    });
                }
            }
        }
    } else {
        // v0.16.0.1 — lone `.snxfpt` (not inside a `.snxlib`). Show
        // the single open footprint as a one-row list rather than an
        // empty panel with a misleading "right-click the .snxlib"
        // hint.
        let display_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| editor.primitive().name.clone());
        library_siblings.push(crate::panels::FootprintLibSibling {
            path: path.clone(),
            display_name,
            is_active: true,
        });
    }

    // v0.16.2 — Properties-panel migration of the bottom inspector
    // strip. Surfaces parameters, solve warnings, and the selected
    // entity's role so the panel can host the Role pick_list +
    // Parameter inputs.
    let sketch_parameters: Vec<(String, String)> = editor
        .primitive()
        .sketch
        .as_ref()
        .map(|s| {
            s.parameters
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        })
        .unwrap_or_default();
    let solve_warnings = editor.state.solve_warnings.clone();
    let selected_sketch_entity_id = editor.state.selected_sketch;
    let (selected_sketch_role, selected_sketch_is_point) = match selected_sketch_entity_id {
        Some(id) => {
            use crate::library::editor::footprint::sketch_dispatch::current_role_of;
            use crate::library::messages::RoleTag;
            use signex_sketch::entity::EntityKind;
            editor
                .primitive()
                .sketch
                .as_ref()
                .and_then(|s| s.entities.iter().find(|e| e.id == id))
                .map(|e| {
                    (
                        current_role_of(e),
                        matches!(e.kind, EntityKind::Point { .. }),
                    )
                })
                .unwrap_or((RoleTag::Unassigned, false))
        }
        None => (crate::library::messages::RoleTag::Unassigned, false),
    };

    // v0.16.3 — pad-placement defaults exposed for the Properties
    // panel form. Form is visible whenever Pads mode + PlacePad tool
    // are active; TAB pause adds a pause hint but does not gate the
    // form itself.
    use crate::library::editor::footprint::state::PadsTool;
    // v0.13 — placement_active true for any primitive-placement tool
    // (Pad / Via / String). TAB pause works uniformly across them.
    let placement_active = matches!(
        editor.state.pads_tool,
        PadsTool::PlacePad | PadsTool::PlaceVia | PadsTool::PlaceString,
    ) && mode_kind == FootprintModeKind::Pads;
    let placement_paused = editor.state.placement_paused;
    let next_pad_designator_override = editor.state.next_pad_defaults.designator_override.clone();
    let next_pad_size_x_mm = editor.state.next_pad_defaults.size_x_mm;
    let next_pad_size_y_mm = editor.state.next_pad_defaults.size_y_mm;
    let next_pad_side = editor.state.next_pad_defaults.side;
    let next_pad_rotation_deg = editor.state.next_pad_defaults.rotation_deg;

    // v0.16.4 — role sub-form summaries. Populated only when the
    // selected entity carries the matching `*Attr`; the Properties
    // panel renders the sub-form conditionally below the Role pick_list.
    let (selected_pour, selected_keepout, selected_cutout, selected_sketch_pad) =
        match selected_sketch_entity_id {
            Some(id) => editor
                .primitive()
                .sketch
                .as_ref()
                .and_then(|s| s.entities.iter().find(|e| e.id == id))
                .map(|e| {
                    let pour = e.pour.as_ref().map(|p| crate::panels::PourSummary {
                        net: p.net.clone(),
                        fill_type: p.fill_type,
                        priority: p.priority,
                    });
                    let keepout = e.keepout.as_ref().map(|k| crate::panels::KeepoutSummary {
                        no_routing: k.kinds.no_routing,
                        no_components: k.kinds.no_components,
                        no_copper: k.kinds.no_copper,
                        no_vias: k.kinds.no_vias,
                        no_drilling: k.kinds.no_drilling,
                        no_pours: k.kinds.no_pours,
                    });
                    let cutout = e
                        .board_cutout
                        .as_ref()
                        .map(|c| crate::panels::CutoutSummary {
                            edge_radius_expr: c.edge_radius_expr.clone(),
                            through: c.through,
                        });
                    let sketch_pad = e.pad.as_ref().map(|p| crate::panels::SketchPadAttrSummary {
                        id: e.id,
                        electrical_type: p.electrical_type,
                        net: p.net.clone(),
                        locked: p.locked,
                        template: p.template.clone(),
                        template_library: p.library.clone(),
                        feature_top: p.feature_top,
                        feature_bottom: p.feature_bottom,
                        testpoint: p.testpoint,
                        thermal_relief: p.stack.thermal_relief,
                        mask_top_tented: p.stack.mask_top_tented,
                        mask_bottom_tented: p.stack.mask_bottom_tented,
                        paste_top_enabled: p.stack.paste_top_enabled,
                        paste_bottom_enabled: p.stack.paste_bottom_enabled,
                        corner_radius_pct: p.stack.corner_radius_pct,
                        hole_tolerance_plus_mm: p.hole_tolerance_plus_mm,
                        hole_tolerance_minus_mm: p.hole_tolerance_minus_mm,
                        hole_rotation_deg: p.hole_rotation_deg,
                        copper_offset_x_mm: p.copper_offset_x_mm,
                        copper_offset_y_mm: p.copper_offset_y_mm,
                        has_drill: p.drill.is_some(),
                    });
                    (pour, keepout, cutout, sketch_pad)
                })
                .unwrap_or((None, None, None, None)),
            None => (None, None, None, None),
        };

    // v0.18.8 — surface every footprint inside the active envelope
    // for the Footprint Library panel rows (Altium PCB Library
    // parity). The `is_active` flag mirrors `editor.active_idx`;
    // panel rendering uses it to highlight the currently-edited
    // sibling.
    let internal_footprints: Vec<crate::panels::FootprintLibInternalRow> = editor
        .file
        .footprints
        .iter()
        .enumerate()
        .map(|(i, fp)| crate::panels::FootprintLibInternalRow {
            name: fp.name.clone(),
            pad_count: fp.pads.len(),
            is_active: i == editor.active_idx,
        })
        .collect();
    let internal_selected_idx = editor.panel_selected_idx;

    // v0.21 — selected silk-front graphic summary with full per-kind
    // editable geometry. Line + Text get dedicated forms; Arc /
    // Rectangle / Circle / Polygon collapse to `Other` and the
    // panel surfaces a sketch-mode hint instead of a custom form.
    // v0.23 — Pattern Properties sub-form. When the selected sketch
    // entity is the source of an array, surface its parameters so the
    // Properties panel can render the Pattern sub-section. Walks
    // `sketch.arrays` for a match — first hit wins (a single entity
    // can be the source of at most one array in v0.23).
    let selected_array = selected_sketch_entity_id.and_then(|sel_id| {
        let sketch = editor.primitive().sketch.as_ref()?;
        use crate::library::editor::footprint::state::ToolPending;
        use signex_sketch::array::{ArrayKind, NumberingScheme};
        let array = sketch.arrays.iter().find(|a| match &a.kind {
            ArrayKind::Linear { source, .. }
            | ArrayKind::Grid { source, .. }
            | ArrayKind::Polar { source, .. } => *source == sel_id,
        })?;
        let kind = match &array.kind {
            ArrayKind::Linear {
                count_expr,
                dx_expr,
                dy_expr,
                ..
            } => crate::panels::ArrayKindSummary::Linear {
                count_expr: count_expr.clone(),
                dx_expr: dx_expr.clone(),
                dy_expr: dy_expr.clone(),
            },
            ArrayKind::Grid {
                nx_expr,
                ny_expr,
                dx_expr,
                dy_expr,
                depopulation,
                ..
            } => {
                let (mask_expr, suppressed_instances) = depopulation
                    .as_ref()
                    .map(|d| (d.mask_expr.clone(), d.suppressed_instances.clone()))
                    .unwrap_or_default();
                crate::panels::ArrayKindSummary::Grid {
                    nx_expr: nx_expr.clone(),
                    ny_expr: ny_expr.clone(),
                    dx_expr: dx_expr.clone(),
                    dy_expr: dy_expr.clone(),
                    mask_expr,
                    suppressed_instances,
                    nx_value: nx_expr.trim().parse::<u32>().ok(),
                    ny_value: ny_expr.trim().parse::<u32>().ok(),
                }
            }
            ArrayKind::Polar {
                count_expr,
                sweep_angle_expr,
                center,
                depopulation,
                ..
            } => {
                let center_position_mm =
                    sketch
                        .entities
                        .iter()
                        .find(|e| e.id == *center)
                        .and_then(|e| match e.kind {
                            signex_sketch::entity::EntityKind::Point { x, y } => Some([x, y]),
                            _ => None,
                        });
                let (mask_expr, suppressed_instances): (String, Vec<u32>) = depopulation
                    .as_ref()
                    .map(|d| {
                        // Polar entries are (i, 0); flatten to a single
                        // index per row.
                        let suppressed = d
                            .suppressed_instances
                            .iter()
                            .filter_map(|(si, sj)| if *sj == 0 { Some(*si) } else { None })
                            .collect();
                        (d.mask_expr.clone(), suppressed)
                    })
                    .unwrap_or_default();
                crate::panels::ArrayKindSummary::Polar {
                    count_expr: count_expr.clone(),
                    sweep_angle_expr: sweep_angle_expr.clone(),
                    center_position_mm,
                    mask_expr,
                    suppressed_instances,
                    count_value: count_expr.trim().parse::<u32>().ok(),
                }
            }
        };
        let numbering = match &array.numbering {
            NumberingScheme::LinearIncrement { .. } => {
                crate::panels::NumberingSchemeKindUi::LinearIncrement
            }
            NumberingScheme::BgaRowCol { .. } => crate::panels::NumberingSchemeKindUi::BgaRowCol,
            NumberingScheme::Explicit { .. } => crate::panels::NumberingSchemeKindUi::Explicit,
        };
        // v0.25 polish — surface BGA-specific config when the
        // numbering scheme is BgaRowCol so the Properties panel can
        // render skip_letters / start_row / start_col rows.
        let bga_config = match &array.numbering {
            NumberingScheme::BgaRowCol {
                skip_letters,
                start_row,
                start_col,
            } => Some(crate::panels::BgaConfigSummary {
                skip_letters: *skip_letters,
                start_row: *start_row,
                start_col: *start_col,
            }),
            _ => None,
        };
        let repicking_polar_center = matches!(
            editor.state.tool_pending,
            ToolPending::RepickPolarCenter { array_id } if array_id == array.id
        );
        Some(crate::panels::ArraySummary {
            array_id: array.id,
            kind,
            numbering,
            repicking_polar_center,
            bga_config,
        })
    });

    let selected_silk_summary = editor.state.selected_silk_f.and_then(|idx| {
        let g = editor.primitive().silk_f.get(idx)?;
        use crate::panels::SilkKindGeometry;
        use signex_library::primitive::footprint::FpGraphicKind;
        let (kind_label, kind) = match &g.kind {
            FpGraphicKind::Line { from, to } => (
                "Line",
                SilkKindGeometry::Line {
                    from_mm: *from,
                    to_mm: *to,
                },
            ),
            FpGraphicKind::Text {
                position,
                content,
                size,
                ..
            } => (
                "Text",
                SilkKindGeometry::Text {
                    position_mm: *position,
                    content: content.clone(),
                    size_mm: *size,
                },
            ),
            FpGraphicKind::Rectangle { .. } => ("Rectangle", SilkKindGeometry::Other),
            FpGraphicKind::Circle { .. } => ("Circle", SilkKindGeometry::Other),
            FpGraphicKind::Arc { .. } => ("Arc", SilkKindGeometry::Other),
            FpGraphicKind::Polygon { .. } => ("Polygon", SilkKindGeometry::Other),
        };
        Some(crate::panels::FootprintSelectedSilkSummary {
            idx,
            kind_label,
            stroke_width_mm: g.stroke_width,
            filled: g.filled,
            kind,
        })
    });

    Some(FootprintEditorPanelContext {
        path,
        footprint_name: editor.primitive().name.clone(),
        version: editor.primitive().version.clone(),
        mode_kind,
        pad_count,
        sketch_entity_count,
        sketch_constraint_count,
        last_solve,
        selected_pad,
        selected_pad_count: editor
            .state
            .selected_pad
            .map(|_| 1 + editor.state.selected_pads_extra.len())
            .unwrap_or(0),
        selected_sketch_entity,
        auto_fit_courtyard: editor.state.auto_fit_courtyard,
        library_siblings,
        library_stem,
        internal_footprints,
        internal_selected_idx,
        sketch_parameters,
        solve_warnings,
        selected_sketch_entity_id,
        selected_sketch_role,
        selected_sketch_is_point,
        placement_active,
        placement_paused,
        next_pad_designator_override,
        next_pad_size_x_mm,
        next_pad_size_y_mm,
        next_pad_side,
        next_pad_rotation_deg,
        next_pad_stack: editor.state.next_pad_defaults.stack.clone(),
        next_pad_shape: editor.state.next_pad_defaults.shape.clone(),
        next_pad_drill_diameter_mm: editor.state.next_pad_defaults.drill_diameter_mm,
        next_pad_drill_slot_length_mm: editor.state.next_pad_defaults.drill_slot_length_mm,
        next_pad_template: editor.state.next_pad_defaults.template.clone(),
        next_pad_template_library: editor.state.next_pad_defaults.template_library.clone(),
        next_pad_feature_top: editor.state.next_pad_defaults.feature_top,
        next_pad_feature_bottom: editor.state.next_pad_defaults.feature_bottom,
        next_pad_testpoint: editor.state.next_pad_defaults.testpoint,
        pad_stack_tab: editor.state.pad_stack_tab,
        next_pad_electrical_type: editor.state.next_pad_defaults.electrical_type,
        next_pad_net: editor.state.next_pad_defaults.net.clone(),
        next_pad_locked: editor.state.next_pad_defaults.locked,
        next_pad_kind: editor.state.next_pad_defaults.kind,
        footprint_description: editor.primitive().description.clone(),
        footprint_default_designator: editor.primitive().default_designator.clone(),
        footprint_component_type: editor.primitive().component_type,
        footprint_height_mm: editor.primitive().height_mm,
        next_pad_hole_tolerance_plus_mm: editor.state.next_pad_defaults.hole_tolerance_plus_mm,
        next_pad_hole_tolerance_minus_mm: editor.state.next_pad_defaults.hole_tolerance_minus_mm,
        next_pad_hole_rotation_deg: editor.state.next_pad_defaults.hole_rotation_deg,
        next_pad_copper_offset_x_mm: editor.state.next_pad_defaults.copper_offset_x_mm,
        next_pad_copper_offset_y_mm: editor.state.next_pad_defaults.copper_offset_y_mm,
        selected_pour,
        selected_keepout,
        selected_cutout,
        selected_sketch_pad,
        snap_options: editor.state.snap_options,
        selection_filter: editor.state.selection_filter,
        snap_subtab: editor.state.snap_subtab,
        snapping_mode: editor.state.snapping_mode,
        guides: editor.state.guides.clone(),
        grids: editor.state.grids.clone(),
        active_grid_idx: editor.state.active_grid_idx,
        selected_silk_summary,
        selected_array,
        selected_pad_shape_params,
        numeric_buffers: editor.state.numeric_buffers.clone(),
    })
}
