use super::*;

impl Signex {
    /// Clear every cursor-following ghost preview. Call before arming a new
    /// ghost or switching to a tool that doesn't have one, so a previously
    /// armed ghost from another tool doesn't linger on the canvas.
    pub(crate) fn clear_ghost_previews(&mut self) {
        self.interaction_state.active_canvas_mut().ghost_label = None;
        self.interaction_state.active_canvas_mut().ghost_symbol = None;
        self.interaction_state.active_canvas_mut().ghost_text = None;
        self.interaction_state.active_canvas_mut().ghost_bus_entry = None;
    }

    /// Drop a half-placed two-click shape anchor on the app and on every
    /// window's canvas, so a leftover corner from one tool or tab can't
    /// complete as the first click of another.
    pub(crate) fn clear_shape_anchors(&mut self) {
        self.interaction_state.shape_anchor = None;
        let clear = |canvas: &mut crate::canvas::CanvasSlot| {
            canvas.shape_anchor = None;
            canvas.clear_overlay_cache();
        };
        clear(&mut self.interaction_state.canvas);
        for canvas in self.interaction_state.canvases.values_mut() {
            clear(canvas);
        }
    }

    /// Mirror `ui_state.lasso_polygon` into the canvas widget's copy and
    /// invalidate the overlay cache. Iced's `canvas::Program` only sees
    /// the widget's own state, so the canvas needs its own snapshot; any
    /// mutation to the in-flight lasso must route through this helper so
    /// the two copies never diverge.
    pub(crate) fn sync_lasso_polygon_to_canvas(&mut self) {
        self.interaction_state.active_canvas_mut().lasso_polygon =
            self.ui_state.lasso_polygon.clone();
        self.interaction_state
            .active_canvas_mut()
            .clear_overlay_cache();
    }

    pub(crate) fn increment_designator(reference: &str) -> Option<String> {
        let digit_start = reference
            .char_indices()
            .find(|(_, ch)| ch.is_ascii_digit())
            .map(|(idx, _)| idx)?;
        let prefix = &reference[..digit_start];
        let digits = &reference[digit_start..];
        let value = digits.parse::<u32>().ok()?;
        Some(format!(
            "{prefix}{:0width$}",
            value.checked_add(1)?,
            width = digits.len()
        ))
    }

    /// The next free `prefix` designator for a part placed on `sheet`,
    /// numbered against every sheet of its project so a new part never
    /// collides with another sheet (#1381). Open sheets are read live and
    /// closed ones from the cached designator scan (`designator_scan.rs`),
    /// so there is no disk read here; until the first scan lands only open
    /// sheets count. A free-standing sheet only looks at itself. Annotate
    /// still does the full renumber.
    pub(crate) fn next_designator_in_sheet(&self, sheet: &std::path::Path, prefix: &str) -> String {
        next_designator(prefix, self.used_designators(sheet).into_iter())
    }

    /// Cancel the in-flight placement session and every armed mode.
    ///
    /// There is exactly ONE session app-wide — `current_tool` and all the
    /// placement buffers below are single fields, and a tool picked from
    /// an undocked window's toolbar sets those same globals — so the
    /// session half of this runs once. The canvas half does not: the
    /// ghosts and previews live on each window's `CanvasSlot`.
    ///
    /// #554 — this used to reach the canvas only through
    /// `active_canvas_mut()`, which is always the MAIN slot
    /// (`state/interaction.rs`). The per-window canvases in
    /// `InteractionState::canvases` were never swept, and the
    /// wire-preview paint gate is two per-canvas fields
    /// (`if self.drawing_mode && !self.wire_preview.is_empty()`,
    /// `canvas/draw/previews.rs`) — so cancelling a wire left an undocked
    /// window painting a frozen ghost of it. Sweeping every canvas fixes
    /// that in both directions: Esc in the undocked window, AND Esc in
    /// the main window while the wire was being drawn in the undocked
    /// one, which per-window routing alone would not.
    pub(crate) fn clear_transient_schematic_tool_state(&mut self) {
        // ── The one app-wide session ─────────────────────────────────
        self.interaction_state.pending_power = None;
        self.interaction_state.pending_port = None;
        // A part on the cursor is part of the same session: Esc, a
        // right-click and every tool change end it, and a library read
        // still in flight no longer arms one.
        self.drop_placement_session();
        // Drop any configured pre-placement defaults so the next tool
        // session starts fresh instead of inheriting the previous one.
        self.document_state.panel_ctx.pre_placement = None;
        self.interaction_state.editing_text = None;
        // Escape / right-click also cancels the net-colour pen, the
        // z-order reference picker, and any in-flight lasso —
        // Altium-parity "one terminator kills every armed mode".
        self.ui_state.pending_net_color = None;
        self.ui_state.reorder_picker = None;
        self.ui_state.move_pick = None;
        self.ui_state.lasso_polygon = None;
        self.clear_drawing_buffers();

        // ── Every window's leftovers ─────────────────────────────────
        //
        // The main slot plus `canvases` covers both regimes: during a
        // `dispatch_canvas_event_in_window` swap the window's canvas IS
        // the main slot and its `canvases` entry is temporarily absent,
        // so the two together are always the full set, with nothing
        // visited twice.
        let clear_canvas = |canvas: &mut crate::canvas::CanvasSlot| {
            canvas.ghost_label = None;
            canvas.ghost_symbol = None;
            canvas.ghost_text = None;
            canvas.ghost_bus_entry = None;
            canvas.tool_preview = None;
            canvas.placement_paused = false;
            canvas.pending_net_color = None;
            canvas.reorder_picker_armed = false;
            canvas.move_pick = None;
            canvas.move_epoch += 1;
            canvas.lasso_polygon = None;
            // Mirrors `sync_lasso_polygon_to_canvas`, which only ever
            // reaches the main slot.
            canvas.clear_overlay_cache();
        };
        clear_canvas(&mut self.interaction_state.canvas);
        for canvas in self.interaction_state.canvases.values_mut() {
            clear_canvas(canvas);
        }
    }

    /// Drop every in-flight multi-click drawing buffer: the wire in
    /// progress, the arc / polyline click lists and the two-click shape
    /// anchor, on the app-wide session and on every window's canvas.
    ///
    /// Shared by [`Self::clear_transient_schematic_tool_state`] and every
    /// tool switch (#102): a buffer started under one placement tool must
    /// not survive into the next (a half-drawn wire turning into a
    /// polyline vertex, a Line anchor becoming a Rectangle corner).
    pub(crate) fn clear_drawing_buffers(&mut self) {
        let was_drawing = std::mem::take(&mut self.interaction_state.wire_drawing);
        if was_drawing {
            self.interaction_state.wire_points.clear();
        }
        self.interaction_state.arc_points.clear();
        self.interaction_state.polyline_points.clear();
        self.interaction_state.shape_anchor = None;

        // The main slot plus `canvases` is the full set of windows.
        let clear_canvas = |canvas: &mut crate::canvas::CanvasSlot| {
            if was_drawing {
                canvas.wire_preview.clear();
                canvas.drawing_mode = false;
            }
            canvas.arc_points.clear();
            canvas.polyline_points.clear();
            canvas.shape_anchor = None;
            canvas.measure = None;
            canvas.clear_overlay_cache();
        };
        clear_canvas(&mut self.interaction_state.canvas);
        for canvas in self.interaction_state.canvases.values_mut() {
            clear_canvas(canvas);
        }
    }

    /// Snap a world-space point to the grid when snapping is on and the grid
    /// size is positive; otherwise return it unchanged. The one snap rule
    /// every canvas click path shares.
    pub(crate) fn snap_to_grid(&self, x: f64, y: f64) -> (f64, f64) {
        crate::canvas::placement_ghost::ghost_anchor(
            (x, y),
            self.ui_state.snap_enabled,
            crate::canvas::grid::pitch_mm_exact(self.ui_state.grid_size_mm),
        )
    }

    /// Where a wire click at `(x, y)` lands: on the nearest electrical hotspot
    /// (pin end, wire end, junction) within the capture radius when the
    /// electrical-grid toggle (Shift+E) is on, otherwise on the grid.
    pub(crate) fn snap_wire_point(&self, x: f64, y: f64) -> (f64, f64) {
        if self.ui_state.snap_hotspots
            && let Some(sheet) = self.active_render_snapshot()
            && let Some(p) = signex_types::schematic::snap_to_hotspot(
                sheet,
                signex_types::schematic::Point::new(x, y),
                signex_types::schematic::hotspot_capture_radius(if self.ui_state.snap_enabled {
                    crate::canvas::grid::pitch_mm_exact(self.ui_state.grid_size_mm)
                } else {
                    0.0
                }),
            )
        {
            return (p.x, p.y);
        }
        self.snap_to_grid(x, y)
    }

    pub(crate) fn align_selected(&mut self, action: &crate::active_bar::ActiveBarAction) {
        use crate::active_bar::ActiveBarAction;

        if self.interaction_state.active_canvas().selected.len() < 2
            && !matches!(action, ActiveBarAction::AlignToGrid)
        {
            return;
        }
        let Some(engine) = self.document_state.active_engine() else {
            return;
        };

        let positions = engine.selection_anchors(&self.interaction_state.active_canvas().selected);

        if positions.is_empty() {
            return;
        }

        let min_x = positions
            .iter()
            .map(|anchor| anchor.x)
            .fold(f64::INFINITY, f64::min);
        let max_x = positions
            .iter()
            .map(|anchor| anchor.x)
            .fold(f64::NEG_INFINITY, f64::max);
        let min_y = positions
            .iter()
            .map(|anchor| anchor.y)
            .fold(f64::INFINITY, f64::min);
        let max_y = positions
            .iter()
            .map(|anchor| anchor.y)
            .fold(f64::NEG_INFINITY, f64::max);
        let center_x = (min_x + max_x) / 2.0;
        let center_y = (min_y + max_y) / 2.0;
        let gs = crate::canvas::grid::pitch_mm_exact(self.ui_state.grid_size_mm);

        let keep_wires = self.ui_state.move_keeps_wires;
        let mut moves = Vec::new();
        for anchor in &positions {
            let (target_x, target_y) = match action {
                ActiveBarAction::AlignLeft => (min_x, anchor.y),
                ActiveBarAction::AlignRight => (max_x, anchor.y),
                ActiveBarAction::AlignTop => (anchor.x, min_y),
                ActiveBarAction::AlignBottom => (anchor.x, max_y),
                ActiveBarAction::AlignHorizontalCenters => (center_x, anchor.y),
                ActiveBarAction::AlignVerticalCenters => (anchor.x, center_y),
                ActiveBarAction::AlignToGrid => {
                    ((anchor.x / gs).round() * gs, (anchor.y / gs).round() * gs)
                }
                _ => (anchor.x, anchor.y),
            };
            let dx = target_x - anchor.x;
            let dy = target_y - anchor.y;
            if dx.abs() > 0.001 || dy.abs() > 0.001 {
                let item = signex_types::schematic::SelectedItem::new(anchor.uuid, anchor.kind);
                moves.push((item, dx, dy));
            }
        }

        if matches!(
            action,
            ActiveBarAction::DistributeHorizontally | ActiveBarAction::DistributeVertically
        ) && positions.len() > 2
        {
            moves.clear();
            let mut sorted = positions.clone();
            let n = sorted.len();
            match action {
                ActiveBarAction::DistributeHorizontally => {
                    sorted.sort_by(|a, b| a.x.total_cmp(&b.x));
                    let step = (max_x - min_x) / (n - 1) as f64;
                    for (index, anchor) in sorted.iter().enumerate() {
                        let target_x = min_x + step * index as f64;
                        let dx = target_x - anchor.x;
                        if dx.abs() > 0.001 {
                            let item = signex_types::schematic::SelectedItem::new(
                                anchor.uuid,
                                anchor.kind,
                            );
                            moves.push((item, dx, 0.0));
                        }
                    }
                }
                ActiveBarAction::DistributeVertically => {
                    sorted.sort_by(|a, b| a.y.total_cmp(&b.y));
                    let step = (max_y - min_y) / (n - 1) as f64;
                    for (index, anchor) in sorted.iter().enumerate() {
                        let target_y = min_y + step * index as f64;
                        let dy = target_y - anchor.y;
                        if dy.abs() > 0.001 {
                            let item = signex_types::schematic::SelectedItem::new(
                                anchor.uuid,
                                anchor.kind,
                            );
                            moves.push((item, 0.0, dy));
                        }
                    }
                }
                _ => {}
            }
        }

        // One command for the whole align/distribute: one undo step, and a
        // wire between two symbols that move alike translates whole.
        if !moves.is_empty() {
            self.apply_engine_commands(
                vec![signex_engine::Command::MoveEach { moves, keep_wires }],
                true,
                false,
            );
        }
    }
}

/// `prefix` followed by one more than the highest number any of `references`
/// carries under that prefix.
fn next_designator<'a>(prefix: &str, references: impl Iterator<Item = &'a str>) -> String {
    let next_index = references
        .filter_map(|reference| {
            let reference = reference.trim();
            reference
                .strip_prefix(prefix)
                .and_then(|n| n.parse::<u32>().ok())
        })
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    format!("{prefix}{next_index}")
}

#[cfg(test)]
mod designator_tests {
    use super::Signex;

    #[test]
    fn designator_increment_does_not_overflow() {
        assert_eq!(Signex::increment_designator("R4294967295"), None);
        assert_eq!(
            Signex::increment_designator("R009").as_deref(),
            Some("R010")
        );
    }

    fn sheet_with(references: &[&str]) -> signex_types::schematic::SchematicSheet {
        let mut sheet = signex_types::schematic::SchematicSheet::default();
        for reference in references {
            sheet.symbols.push(signex_types::schematic::Symbol {
                uuid: uuid::Uuid::now_v7(),
                reference: (*reference).to_string(),
                ..Default::default()
            });
        }
        sheet
    }

    /// #1381: the suggestion is numbered against the whole project.
    #[test]
    fn next_designator_is_unique_across_the_project() {
        use crate::app::handlers::menu::export::tests::{app_workspace, open_with};
        let mut app = app_workspace("/w/a", &["a.snxsch", "b.snxsch"]);
        let a = std::path::PathBuf::from("/w/a/a.snxsch");
        let b = std::path::PathBuf::from("/w/a/b.snxsch");
        open_with(&mut app.document_state, &a, sheet_with(&["C1"]));
        let on_b: Vec<String> = (1..=7).map(|n| format!("R{n}")).collect();
        let on_b: Vec<&str> = on_b.iter().map(String::as_str).collect();
        open_with(&mut app.document_state, &b, sheet_with(&on_b));

        assert_eq!(app.next_designator_in_sheet(&a, "R"), "R8");
        assert_eq!(app.next_designator_in_sheet(&b, "R"), "R8");
    }

    #[test]
    fn a_free_standing_sheet_only_looks_at_itself() {
        use crate::app::handlers::menu::export::tests::{app_workspace, open_with};
        let mut app = app_workspace("/w/a", &["a.snxsch"]);
        let loose = std::path::PathBuf::from("/w/loose/x.snxsch");
        let other = std::path::PathBuf::from("/w/loose/y.snxsch");
        open_with(&mut app.document_state, &loose, sheet_with(&["R2"]));
        open_with(&mut app.document_state, &other, sheet_with(&["R9"]));

        assert_eq!(app.next_designator_in_sheet(&loose, "R"), "R3");
    }
}
