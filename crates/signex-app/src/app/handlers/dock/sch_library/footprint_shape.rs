//! Footprint-editor pour / keepout / cutout / snap / array setters —
//! the helper methods behind the remaining `FpEditor*` dock-panel
//! messages that edit copper pours, keepouts, cutouts, the snapping
//! model, and pad arrays on the active `.snxfpt` editor. The
//! dispatcher in `mod.rs` routes here.
//!
//! Pure code motion out of the former `sch_library.rs` god-file
//! (ADR-0001 #163); zero behaviour change.

use super::*;

impl Signex {
    /// v0.16.4 — mutate the selected entity's pour `net` and re-bake.
    pub(crate) fn fp_editor_set_pour_net(
        &mut self,
        id: signex_sketch::id::SketchEntityId,
        value: String,
    ) -> bool {
        let net = if value.trim().is_empty() {
            None
        } else {
            Some(value)
        };
        if let Some(editor) = self.active_footprint_editor_mut() {
            if let Some(sketch) = editor.primitive_mut().sketch.as_mut() {
                if let Some(e) = sketch.entities.iter_mut().find(|e| e.id == id) {
                    if let Some(p) = e.pour.as_mut() {
                        p.net = net;
                    }
                }
            }
            use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
            use crate::library::editor::footprint::sketch_mode::SketchEdit;
            editor.with_parts(|state, primitive| {
                apply_sketch_edit_with_warnings(state, primitive, SketchEdit::ForceRebuild);
            });
            editor.dirty = true;
            editor.canvas_cache.clear();
        }
        self.refresh_panel_ctx();
        true
    }

    pub(crate) fn fp_editor_set_pour_fill_type(
        &mut self,
        id: signex_sketch::id::SketchEntityId,
        value: signex_sketch::attr::PourFillType,
    ) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            if let Some(sketch) = editor.primitive_mut().sketch.as_mut() {
                if let Some(e) = sketch.entities.iter_mut().find(|e| e.id == id) {
                    if let Some(p) = e.pour.as_mut() {
                        p.fill_type = value;
                    }
                }
            }
            use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
            use crate::library::editor::footprint::sketch_mode::SketchEdit;
            editor.with_parts(|state, primitive| {
                apply_sketch_edit_with_warnings(state, primitive, SketchEdit::ForceRebuild);
            });
            editor.dirty = true;
            editor.canvas_cache.clear();
        }
        self.refresh_panel_ctx();
        true
    }

    pub(crate) fn fp_editor_set_pour_priority(
        &mut self,
        id: signex_sketch::id::SketchEntityId,
        value: String,
    ) -> bool {
        let parsed = value.trim().parse::<u32>().ok();
        if let Some(editor) = self.active_footprint_editor_mut() {
            if let Some(sketch) = editor.primitive_mut().sketch.as_mut() {
                if let Some(e) = sketch.entities.iter_mut().find(|e| e.id == id) {
                    if let Some(p) = e.pour.as_mut() {
                        if let Some(n) = parsed {
                            p.priority = n;
                        }
                    }
                }
            }
            use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
            use crate::library::editor::footprint::sketch_mode::SketchEdit;
            editor.with_parts(|state, primitive| {
                apply_sketch_edit_with_warnings(state, primitive, SketchEdit::ForceRebuild);
            });
            editor.dirty = true;
            editor.canvas_cache.clear();
        }
        self.refresh_panel_ctx();
        true
    }

    pub(crate) fn fp_editor_set_keepout_kind(
        &mut self,
        id: signex_sketch::id::SketchEntityId,
        kind: crate::panels::KeepoutKindFlag,
        value: bool,
    ) -> bool {
        use crate::panels::KeepoutKindFlag;
        if let Some(editor) = self.active_footprint_editor_mut() {
            if let Some(sketch) = editor.primitive_mut().sketch.as_mut() {
                if let Some(e) = sketch.entities.iter_mut().find(|e| e.id == id) {
                    if let Some(k) = e.keepout.as_mut() {
                        match kind {
                            KeepoutKindFlag::NoRouting => k.kinds.no_routing = value,
                            KeepoutKindFlag::NoComponents => k.kinds.no_components = value,
                            KeepoutKindFlag::NoCopper => k.kinds.no_copper = value,
                            KeepoutKindFlag::NoVias => k.kinds.no_vias = value,
                            KeepoutKindFlag::NoDrilling => k.kinds.no_drilling = value,
                            KeepoutKindFlag::NoPours => k.kinds.no_pours = value,
                        }
                    }
                }
            }
            use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
            use crate::library::editor::footprint::sketch_mode::SketchEdit;
            editor.with_parts(|state, primitive| {
                apply_sketch_edit_with_warnings(state, primitive, SketchEdit::ForceRebuild);
            });
            editor.dirty = true;
            editor.canvas_cache.clear();
        }
        self.refresh_panel_ctx();
        true
    }

    pub(crate) fn fp_editor_set_cutout_edge_radius(
        &mut self,
        id: signex_sketch::id::SketchEntityId,
        value: String,
    ) -> bool {
        let edge_radius = if value.trim().is_empty() {
            None
        } else {
            Some(value)
        };
        if let Some(editor) = self.active_footprint_editor_mut() {
            if let Some(sketch) = editor.primitive_mut().sketch.as_mut() {
                if let Some(e) = sketch.entities.iter_mut().find(|e| e.id == id) {
                    if let Some(c) = e.board_cutout.as_mut() {
                        c.edge_radius_expr = edge_radius;
                    }
                }
            }
            use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
            use crate::library::editor::footprint::sketch_mode::SketchEdit;
            editor.with_parts(|state, primitive| {
                apply_sketch_edit_with_warnings(state, primitive, SketchEdit::ForceRebuild);
            });
            editor.dirty = true;
            editor.canvas_cache.clear();
        }
        self.refresh_panel_ctx();
        true
    }

    pub(crate) fn fp_editor_toggle_snap_option(
        &mut self,
        flag: crate::panels::SnapOptionFlag,
    ) -> bool {
        use crate::panels::SnapOptionFlag;
        if let Some(editor) = self.active_footprint_editor_mut() {
            let opts = &mut editor.state.snap_options;
            match flag {
                SnapOptionFlag::PointHit => opts.point_hit = !opts.point_hit,
                SnapOptionFlag::HorizontalVertical => {
                    opts.horizontal_vertical = !opts.horizontal_vertical
                }
                SnapOptionFlag::Angle => opts.angle = !opts.angle,
                SnapOptionFlag::Grid => opts.grid = !opts.grid,
                SnapOptionFlag::TrackVertices => {
                    opts.snap_track_vertices = !opts.snap_track_vertices
                }
                SnapOptionFlag::TrackLines => opts.snap_track_lines = !opts.snap_track_lines,
                SnapOptionFlag::ArcCenters => opts.snap_arc_centers = !opts.snap_arc_centers,
                SnapOptionFlag::Intersections => opts.snap_intersections = !opts.snap_intersections,
                SnapOptionFlag::PadCenters => opts.snap_pad_centers = !opts.snap_pad_centers,
                SnapOptionFlag::PadVertices => opts.snap_pad_vertices = !opts.snap_pad_vertices,
                SnapOptionFlag::PadEdges => opts.snap_pad_edges = !opts.snap_pad_edges,
                SnapOptionFlag::ViaCenters => opts.snap_via_centers = !opts.snap_via_centers,
                SnapOptionFlag::Texts => opts.snap_texts = !opts.snap_texts,
                SnapOptionFlag::Regions => opts.snap_regions = !opts.snap_regions,
                SnapOptionFlag::FootprintOrigins => {
                    opts.snap_footprint_origins = !opts.snap_footprint_origins
                }
                SnapOptionFlag::Body3dPoints => {
                    opts.snap_3d_body_points = !opts.snap_3d_body_points
                }
                SnapOptionFlag::SnapToGrids => opts.snap_to_grids = !opts.snap_to_grids,
                SnapOptionFlag::SnapToGuides => opts.snap_to_guides = !opts.snap_to_guides,
                SnapOptionFlag::SnapToAxes => opts.snap_to_axes = !opts.snap_to_axes,
            }
            editor.canvas_cache.clear();
        }
        self.refresh_panel_ctx();
        true
    }

    /// v0.13 — Altium "Snap Distance" setter.
    pub(crate) fn handle_fp_set_snap_distance(&mut self, raw: String) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            if let Ok(v) = raw.trim().parse::<f64>() {
                editor.state.snap_options.snap_distance_mm = v.clamp(0.001, 100.0);
                editor.canvas_cache.clear();
            }
        }
        self.refresh_panel_ctx();
        true
    }

    /// v0.13 — Altium "Axis Snap Range" setter.
    pub(crate) fn handle_fp_set_axis_snap_range(&mut self, raw: String) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            if let Ok(v) = raw.trim().parse::<f64>() {
                editor.state.snap_options.axis_snap_range_mm = v.clamp(0.001, 100.0);
                editor.canvas_cache.clear();
            }
        }
        self.refresh_panel_ctx();
        true
    }

    /// v0.18.9 — Properties-panel "Grid step" numeric input. Parses
    /// the user's text; on a clean positive parse writes
    /// `state.snap_options.grid_step_mm`. Invalid / empty / non-
    /// positive strings no-op so partial keystrokes don't snap to
    /// zero (which would crash the canvas's grid math).
    pub(crate) fn fp_editor_set_snap_grid_step(&mut self, value: &str) -> bool {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return true;
        }
        let parsed: f64 = match trimmed.parse::<f64>() {
            Ok(v) if v > 0.0 && v.is_finite() => v,
            _ => return true,
        };
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.snap_options.grid_step_mm = parsed;
            // v0.18.21 — mirror onto the active grid row so the
            // Manager view + the canvas stay aligned.
            let idx = editor.state.active_grid_idx;
            if let Some(row) = editor.state.grids.get_mut(idx) {
                row.step_mm = parsed;
            }
            editor.canvas_cache.clear();
        }
        self.refresh_panel_ctx();
        true
    }

    pub(crate) fn fp_editor_set_cutout_through(
        &mut self,
        id: signex_sketch::id::SketchEntityId,
        value: bool,
    ) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            if let Some(sketch) = editor.primitive_mut().sketch.as_mut() {
                if let Some(e) = sketch.entities.iter_mut().find(|e| e.id == id) {
                    if let Some(c) = e.board_cutout.as_mut() {
                        c.through = value;
                    }
                }
            }
            use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
            use crate::library::editor::footprint::sketch_mode::SketchEdit;
            editor.with_parts(|state, primitive| {
                apply_sketch_edit_with_warnings(state, primitive, SketchEdit::ForceRebuild);
            });
            editor.dirty = true;
            editor.canvas_cache.clear();
        }
        self.refresh_panel_ctx();
        true
    }

    /// v0.23 — Pattern sub-form text-input edit. Walks `sketch.arrays`
    /// to find the array, mutates the field identified by
    /// `ArrayParamField`, then runs `SketchEdit::ForceRebuild` so the
    /// bake re-expands. `MaskExpr` with an empty value clears the
    /// depopulation entirely (to avoid leaving a `mask_expr=""` orphan
    /// that blocks re-enabling instances later).
    pub(crate) fn fp_editor_edit_array_param(
        &mut self,
        array_id: signex_sketch::array::ArrayId,
        field: crate::panels::ArrayParamField,
        value: String,
    ) -> bool {
        use crate::panels::ArrayParamField;
        use signex_sketch::array::{ArrayKind, GridDepopulation};
        if let Some(editor) = self.active_footprint_editor_mut() {
            if let Some(sketch) = editor.primitive_mut().sketch.as_mut() {
                if let Some(array) = sketch.arrays.iter_mut().find(|a| a.id == array_id) {
                    let trimmed = value.trim();
                    match (&mut array.kind, field) {
                        (
                            ArrayKind::Linear { count_expr, .. },
                            ArrayParamField::LinearCountExpr,
                        ) => {
                            *count_expr = value;
                        }
                        (ArrayKind::Linear { dx_expr, .. }, ArrayParamField::LinearDxExpr) => {
                            *dx_expr = value;
                        }
                        (ArrayKind::Linear { dy_expr, .. }, ArrayParamField::LinearDyExpr) => {
                            *dy_expr = value;
                        }
                        (ArrayKind::Grid { nx_expr, .. }, ArrayParamField::GridNxExpr) => {
                            *nx_expr = value;
                        }
                        (ArrayKind::Grid { ny_expr, .. }, ArrayParamField::GridNyExpr) => {
                            *ny_expr = value;
                        }
                        (ArrayKind::Grid { dx_expr, .. }, ArrayParamField::GridDxExpr) => {
                            *dx_expr = value;
                        }
                        (ArrayKind::Grid { dy_expr, .. }, ArrayParamField::GridDyExpr) => {
                            *dy_expr = value;
                        }
                        (ArrayKind::Polar { count_expr, .. }, ArrayParamField::PolarCountExpr) => {
                            *count_expr = value;
                        }
                        (
                            ArrayKind::Polar {
                                sweep_angle_expr, ..
                            },
                            ArrayParamField::PolarSweepAngleExpr,
                        ) => {
                            *sweep_angle_expr = value;
                        }
                        (
                            ArrayKind::Grid { depopulation, .. }
                            | ArrayKind::Polar { depopulation, .. },
                            ArrayParamField::MaskExpr,
                        ) => {
                            // Preserve any existing per-instance
                            // suppression list when editing the mask
                            // expression — the user might be tweaking
                            // both at once via the Properties panel.
                            let prior = depopulation
                                .as_ref()
                                .map(|d| d.suppressed_instances.clone())
                                .unwrap_or_default();
                            if trimmed.is_empty() && prior.is_empty() {
                                *depopulation = None;
                            } else {
                                *depopulation = Some(GridDepopulation {
                                    mask_expr: value,
                                    suppressed_instances: prior,
                                });
                            }
                        }
                        // Mismatched (kind, field) pairs no-op so a
                        // stale panel can't corrupt the array.
                        _ => {}
                    }
                }
            }
            use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
            use crate::library::editor::footprint::sketch_mode::SketchEdit;
            editor.with_parts(|state, primitive| {
                apply_sketch_edit_with_warnings(state, primitive, SketchEdit::ForceRebuild);
            });
            editor.dirty = true;
            editor.canvas_cache.clear();
        }
        self.refresh_panel_ctx();
        true
    }

    /// v0.23 — Switch numbering scheme. Maps the panel's enum onto
    /// [`signex_sketch::array::NumberingScheme`] using sensible
    /// defaults (1-step LinearIncrement, BGA `A1`-rooted, empty
    /// Explicit list). Existing inner state isn't preserved across
    /// kind flips — switching numbering schemes is a discrete edit.
    pub(crate) fn fp_editor_set_array_numbering_scheme(
        &mut self,
        array_id: signex_sketch::array::ArrayId,
        scheme: crate::panels::NumberingSchemeKindUi,
    ) -> bool {
        use crate::panels::NumberingSchemeKindUi;
        use signex_sketch::array::NumberingScheme;
        if let Some(editor) = self.active_footprint_editor_mut() {
            if let Some(sketch) = editor.primitive_mut().sketch.as_mut() {
                if let Some(array) = sketch.arrays.iter_mut().find(|a| a.id == array_id) {
                    array.numbering = match scheme {
                        NumberingSchemeKindUi::LinearIncrement => {
                            NumberingScheme::LinearIncrement {
                                start_expr: "1".into(),
                                step_expr: "1".into(),
                            }
                        }
                        NumberingSchemeKindUi::BgaRowCol => NumberingScheme::BgaRowCol {
                            skip_letters: true,
                            start_row: 'A',
                            start_col: 1,
                        },
                        NumberingSchemeKindUi::Explicit => {
                            NumberingScheme::Explicit { names: Vec::new() }
                        }
                    };
                }
            }
            use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
            use crate::library::editor::footprint::sketch_mode::SketchEdit;
            editor.with_parts(|state, primitive| {
                apply_sketch_edit_with_warnings(state, primitive, SketchEdit::ForceRebuild);
            });
            editor.dirty = true;
            editor.canvas_cache.clear();
        }
        self.refresh_panel_ctx();
        true
    }

    pub(crate) fn fp_editor_set_bga_skip_letters(
        &mut self,
        array_id: signex_sketch::array::ArrayId,
        skip_letters: bool,
    ) -> bool {
        use signex_sketch::array::NumberingScheme;
        if let Some(editor) = self.active_footprint_editor_mut() {
            if let Some(sketch) = editor.primitive_mut().sketch.as_mut() {
                if let Some(array) = sketch.arrays.iter_mut().find(|a| a.id == array_id) {
                    if let NumberingScheme::BgaRowCol {
                        skip_letters: s, ..
                    } = &mut array.numbering
                    {
                        *s = skip_letters;
                    }
                }
            }
            use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
            use crate::library::editor::footprint::sketch_mode::SketchEdit;
            editor.with_parts(|state, primitive| {
                apply_sketch_edit_with_warnings(state, primitive, SketchEdit::ForceRebuild);
            });
            editor.dirty = true;
            editor.canvas_cache.clear();
        }
        self.refresh_panel_ctx();
        true
    }

    /// v0.25 polish — set BGA `start_row` letter. Empty / non-letter
    /// input no-ops; multi-char takes the first letter; uppercased
    /// before storage. Letters I/O/Q/S/X/Z are valid starts even when
    /// `skip_letters = true` (the skip applies to the row alphabet,
    /// not the start point).
    pub(crate) fn fp_editor_set_bga_start_row(
        &mut self,
        array_id: signex_sketch::array::ArrayId,
        value: String,
    ) -> bool {
        use signex_sketch::array::NumberingScheme;
        let Some(first_char) = value.chars().next() else {
            return true;
        };
        if !first_char.is_ascii_alphabetic() {
            return true;
        }
        let upper = first_char.to_ascii_uppercase();
        if let Some(editor) = self.active_footprint_editor_mut() {
            if let Some(sketch) = editor.primitive_mut().sketch.as_mut() {
                if let Some(array) = sketch.arrays.iter_mut().find(|a| a.id == array_id) {
                    if let NumberingScheme::BgaRowCol { start_row, .. } = &mut array.numbering {
                        *start_row = upper;
                    }
                }
            }
            use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
            use crate::library::editor::footprint::sketch_mode::SketchEdit;
            editor.with_parts(|state, primitive| {
                apply_sketch_edit_with_warnings(state, primitive, SketchEdit::ForceRebuild);
            });
            editor.dirty = true;
            editor.canvas_cache.clear();
        }
        self.refresh_panel_ctx();
        true
    }

    /// v0.25 polish — set BGA `start_col` integer. Empty / non-numeric
    /// no-ops; values are u32 so negatives are silently rejected by
    /// the parse.
    pub(crate) fn fp_editor_set_bga_start_col(
        &mut self,
        array_id: signex_sketch::array::ArrayId,
        value: String,
    ) -> bool {
        use signex_sketch::array::NumberingScheme;
        let trimmed = value.trim();
        let Ok(parsed) = trimmed.parse::<u32>() else {
            return true;
        };
        if let Some(editor) = self.active_footprint_editor_mut() {
            if let Some(sketch) = editor.primitive_mut().sketch.as_mut() {
                if let Some(array) = sketch.arrays.iter_mut().find(|a| a.id == array_id) {
                    if let NumberingScheme::BgaRowCol { start_col, .. } = &mut array.numbering {
                        *start_col = parsed;
                    }
                }
            }
            use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
            use crate::library::editor::footprint::sketch_mode::SketchEdit;
            editor.with_parts(|state, primitive| {
                apply_sketch_edit_with_warnings(state, primitive, SketchEdit::ForceRebuild);
            });
            editor.dirty = true;
            editor.canvas_cache.clear();
        }
        self.refresh_panel_ctx();
        true
    }

    /// v0.23 — Delete an array. The source entity stays in the sketch
    /// — only the array record is removed, so existing constraints on
    /// the source survive intact.
    pub(crate) fn fp_editor_delete_array(
        &mut self,
        array_id: signex_sketch::array::ArrayId,
    ) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            if let Some(sketch) = editor.primitive_mut().sketch.as_mut() {
                sketch.arrays.retain(|a| a.id != array_id);
            }
            use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
            use crate::library::editor::footprint::sketch_mode::SketchEdit;
            editor.with_parts(|state, primitive| {
                apply_sketch_edit_with_warnings(state, primitive, SketchEdit::ForceRebuild);
            });
            editor.dirty = true;
            editor.canvas_cache.clear();
        }
        self.refresh_panel_ctx();
        true
    }

    /// v0.23 — Begin re-picking a polar centre. Sets
    /// `ToolPending::RepickPolarCenter` so the next sketch click on a
    /// Point overwrites the array's `center`. The dispatcher in
    /// [`crate::app::dispatch::library`] consumes the pending state and
    /// resets to `Idle`.
    pub(crate) fn fp_editor_begin_repick_polar_center(
        &mut self,
        array_id: signex_sketch::array::ArrayId,
    ) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            use crate::library::editor::footprint::state::ToolPending;
            editor.state.tool_pending = ToolPending::RepickPolarCenter { array_id };
            editor.canvas_cache.clear();
        }
        self.refresh_panel_ctx();
        true
    }

    /// v0.23 — Toggle a single `(i, j)` instance in an array's
    /// per-instance suppression list. `value=true` enables the
    /// instance (removes the entry); `value=false` suppresses it
    /// (adds the entry, deduplicated). Polar arrays set `j = 0`.
    ///
    /// When the suppression list grows from empty, the array gains a
    /// fresh `GridDepopulation { mask_expr: "", suppressed_instances }`.
    /// When the list returns to empty AND the existing `mask_expr` is
    /// blank, the depopulation is removed entirely so the array
    /// returns to its parametric-only state.
    pub(crate) fn fp_editor_toggle_array_instance(
        &mut self,
        array_id: signex_sketch::array::ArrayId,
        i: u32,
        j: u32,
        value: bool,
    ) -> bool {
        use signex_sketch::array::{ArrayKind, GridDepopulation};
        if let Some(editor) = self.active_footprint_editor_mut() {
            if let Some(sketch) = editor.primitive_mut().sketch.as_mut() {
                if let Some(array) = sketch.arrays.iter_mut().find(|a| a.id == array_id) {
                    let depop_slot: &mut Option<GridDepopulation> = match &mut array.kind {
                        ArrayKind::Grid { depopulation, .. } => depopulation,
                        ArrayKind::Polar { depopulation, .. } => depopulation,
                        ArrayKind::Linear { .. } => return true,
                    };
                    if value {
                        // Re-enable the instance — drop matching entries.
                        if let Some(d) = depop_slot.as_mut() {
                            d.suppressed_instances
                                .retain(|(si, sj)| !(*si == i && *sj == j));
                            if d.mask_expr.trim().is_empty() && d.suppressed_instances.is_empty() {
                                *depop_slot = None;
                            }
                        }
                    } else {
                        // Suppress the instance — append if absent.
                        let d = depop_slot.get_or_insert_with(|| GridDepopulation {
                            mask_expr: String::new(),
                            suppressed_instances: Vec::new(),
                        });
                        if !d
                            .suppressed_instances
                            .iter()
                            .any(|(si, sj)| *si == i && *sj == j)
                        {
                            d.suppressed_instances.push((i, j));
                        }
                    }
                }
            }
            use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
            use crate::library::editor::footprint::sketch_mode::SketchEdit;
            editor.with_parts(|state, primitive| {
                apply_sketch_edit_with_warnings(state, primitive, SketchEdit::ForceRebuild);
            });
            editor.dirty = true;
            editor.canvas_cache.clear();
        }
        self.refresh_panel_ctx();
        true
    }
}
