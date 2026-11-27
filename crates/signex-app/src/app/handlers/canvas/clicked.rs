use iced::Task;

use super::super::super::*;
use super::shape_tool::ClickKind;

/// What a Wire tool point does to the wire in progress
/// (docs/DECISIONS.md, 2026-11-24).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(in crate::app::handlers) enum WirePoint {
    /// A click: the second point places the wire and ends it; the tool
    /// stays armed for the next wire.
    EndsWire,
    /// The add-vertex key (#1669): the point is a corner, the wire goes on.
    AddsVertex,
}

impl Signex {
    pub(in crate::app::handlers) fn handle_canvas_clicked(
        &mut self,
        world_x: f64,
        world_y: f64,
        wire_point: WirePoint,
    ) -> Task<Message> {
        // Altium-style lasso: first click anchors the start,
        // the cursor path auto-samples vertices, a second
        // click closes the polygon and commits. Escape /
        // right-click cancels.
        if self.ui_state.lasso_polygon.is_some() {
            let (vx, vy) = self.snap_to_grid(world_x, world_y);
            let has_start = self
                .ui_state
                .lasso_polygon
                .as_ref()
                .is_some_and(|v| !v.is_empty());
            if !has_start {
                // First click — anchor vertex 0.
                if let Some(poly) = self.ui_state.lasso_polygon.as_mut() {
                    poly.push(signex_types::schematic::Point::new(vx, vy));
                }
                self.sync_lasso_polygon_to_canvas();
                return Task::none();
            }
            // Second click — close and commit. Append the
            // click position as the final vertex so the
            // polygon lands exactly where the user clicked.
            let mut pts = self.ui_state.lasso_polygon.take().unwrap_or_default();
            pts.push(signex_types::schematic::Point::new(vx, vy));
            if pts.len() >= 3 {
                let poly: Vec<(f64, f64)> = pts.iter().map(|p| (p.x, p.y)).collect();
                if let Some(snapshot) = self.active_render_snapshot() {
                    let filters = self.interaction_state.selection_filters.clone();
                    self.interaction_state.active_canvas_mut().selected =
                        crate::schematic_runtime::hit_test::hit_test_polygon(snapshot, &poly)
                            .into_iter()
                            .filter(|h| {
                                crate::app::handlers::selection_workflow::passes_filter(
                                    h, snapshot, &filters,
                                )
                            })
                            .collect();
                    self.update_selection_info();
                }
            }
            // `lasso_polygon` already drained via `.take()` above;
            // mirror the None into the canvas copy + invalidate
            // the overlay cache.
            self.sync_lasso_polygon_to_canvas();
            return Task::none();
        }
        // Net-colour flood: the user picked a swatch from the
        // Active Bar and is now clicking a wire. Union-find the
        // whole connected net and apply the colour (or clear it
        // if alpha == 0) as one undoable engine edit of the sheet
        // (`commit_net_colors`, the path the `color_net` command runs).
        if let Some(pending) = self.ui_state.pending_net_color {
            // Snap the click point to the grid before hit
            // testing so the click lands where the pen ghost
            // previewed. Without this the cursor's raw world
            // coords can land a full grid cell off the nearest
            // wire — the visible pen snaps, but the hit target
            // was being missed.
            let (hit_x, hit_y) = if self.ui_state.snap_enabled && self.ui_state.grid_size_mm > 0.0 {
                let g = crate::canvas::grid::pitch_mm_exact(self.ui_state.grid_size_mm);
                ((world_x / g).round() * g, (world_y / g).round() * g)
            } else {
                (world_x, world_y)
            };
            let net = self.net_elements_at(hit_x, hit_y);
            if !net.is_empty() {
                self.commit_net_colors(&net, pending);
            }
            // Altium-style continuous placement: stay armed
            // until the user right-clicks or presses Escape.
            // Empty-canvas clicks are no-ops; wire clicks keep
            // the pen ready for the next net. Return early so
            // the normal selection path doesn't run and pick
            // up the clicked wire as a selection.
            return Task::none();
        }
        // Z-order reference picker: the user previously chose
        // Bring-To-Front-Of / Send-To-Back-Of on the Active Bar
        // and is now clicking a reference item. Resolve the hit,
        // emit a Reorder command, and clear the picker — matches
        // Altium's "click the object to reference" convention.
        if let Some(picker) = self.ui_state.reorder_picker {
            if let Some(snapshot) = self.active_render_snapshot() {
                let hit = crate::schematic_runtime::hit_test::hit_test_styled(
                    snapshot,
                    &self.interaction_state.active_canvas().designators,
                    world_x,
                    world_y,
                    self.active_wire_widths(),
                );
                if let Some(reference) = hit {
                    let direction = match picker {
                        super::super::super::state::ReorderPicker::Above => {
                            signex_engine::ReorderDirection::JustAbove(reference.uuid)
                        }
                        super::super::super::state::ReorderPicker::Below => {
                            signex_engine::ReorderDirection::JustBelow(reference.uuid)
                        }
                    };
                    let items = self.interaction_state.active_canvas_mut().selected.clone();
                    self.apply_engine_command(
                        signex_engine::Command::ReorderObjects { items, direction },
                        false,
                        true,
                    );
                }
            }
            self.ui_state.reorder_picker = None;
            self.interaction_state
                .active_canvas_mut()
                .reorder_picker_armed = false;
            self.interaction_state
                .active_canvas_mut()
                .clear_overlay_cache();
            return Task::none();
        }

        // Altium-style placement pause: while the ghost is frozen
        // (TAB pressed, pre-placement form open), canvas clicks are
        // suppressed so the user can edit properties without a
        // stray click dropping an object. `pre_placement` lives on
        // past Resume so the first click after Resume inherits the
        // edited defaults — so key the gate on `placement_paused`,
        // not on `pre_placement.is_some()`.
        if self.interaction_state.active_canvas_mut().placement_paused {
            return Task::none();
        }
        // The point buffer of a tool paused in another window belongs to
        // that window's sheet, even while it is still empty.
        if self.is_annotation_pause_of_another(None) {
            return Task::none();
        }
        // A click outside the inline editor commits its current value
        // and returns to normal selection — same convention as most
        // IDEs (Escape cancels, click elsewhere confirms).
        if let Some(state) = self.interaction_state.editing_text.take() {
            self.commit_text_edit(state);
            return Task::none();
        }
        // Mockup row 197: a click on a Notebook badge opens the notebook at
        // its first block instead of selecting.
        if self.interaction_state.current_tool == Tool::Select
            && self.interaction_state.placement.is_none()
            && let Some(task) = self.click_notebook_badge(world_x, world_y)
        {
            return task;
        }
        let (wx, wy) = if self.interaction_state.current_tool == Tool::Wire {
            self.snap_wire_point(self.active_render_snapshot(), world_x, world_y)
        } else {
            self.snap_to_grid(world_x, world_y)
        };
        // A click places where it lands: a position typed in the Tab form
        // is not carried over to the object after it (#2532).
        self.clear_typed_position();
        self.click_with_tool((wx, wy), (world_x, world_y), wire_point)
    }

    /// What a click of the armed tool does at `(wx, wy)`, already snapped
    /// (or typed in the Tab form); `world` is the raw point a selection
    /// click hit-tests.
    pub(in crate::app::handlers) fn click_with_tool(
        &mut self,
        (wx, wy): (f64, f64),
        (world_x, world_y): (f64, f64),
        wire_point: WirePoint,
    ) -> Task<Message> {
        // A part on the cursor: the click places it, whatever tool is shown.
        if self.interaction_state.placement.is_some() {
            return self.handle_placement_click(wx, wy);
        }

        self.end_wire_of_another_canvas();
        match self.interaction_state.current_tool {
            Tool::Wire => {
                let pt = signex_types::schematic::Point::new(wx, wy);
                if !self.interaction_state.wire_drawing {
                    self.start_wire_drawing(pt);
                } else if let Some(&start) = self.interaction_state.wire_points.last() {
                    let (end_pt, placed) =
                        self.place_wire_at(start, pt, self.interaction_state.corner_rule);
                    // A point that placed nothing keeps the start.
                    if placed && wire_point == WirePoint::EndsWire {
                        self.end_wire_drawing();
                    } else {
                        self.continue_wire_drawing(end_pt);
                    }
                }
            }
            Tool::Bus => {
                let pt = signex_types::schematic::Point::new(wx, wy);
                if !self.interaction_state.wire_drawing {
                    self.start_wire_drawing(pt);
                } else if let Some(&start) = self.interaction_state.wire_points.last() {
                    let (end_pt, _) =
                        self.place_bus_at(start, pt, self.interaction_state.corner_rule);
                    self.continue_wire_drawing(end_pt);
                }
            }
            // #1280: a harness wire is drawn like a bus.
            Tool::HarnessWire => {
                let pt = signex_types::schematic::Point::new(wx, wy);
                if !self.interaction_state.wire_drawing {
                    self.start_wire_drawing(pt);
                } else if let Some(&start) = self.interaction_state.wire_points.last() {
                    let (end_pt, _) =
                        self.place_harness_wire_at(start, pt, self.interaction_state.corner_rule);
                    self.continue_wire_drawing(end_pt);
                }
            }
            Tool::HarnessConnector => {
                self.harness_connector_click(signex_types::schematic::Point::new(wx, wy));
            }
            Tool::HarnessEntry => {
                self.harness_entry_click(signex_types::schematic::Point::new(wx, wy));
            }
            Tool::Component if self.interaction_state.pending_power.is_some() => {
                if let Some((net_name, lib_id)) = self.interaction_state.pending_power.clone() {
                    self.place_power_at(
                        signex_types::schematic::Point::new(wx, wy),
                        &net_name,
                        &lib_id,
                    );
                }
            }
            Tool::NoConnect => {
                self.place_no_connect_at(signex_types::schematic::Point::new(wx, wy));
            }
            Tool::Junction => {
                self.place_junction_at(signex_types::schematic::Point::new(wx, wy));
            }
            Tool::NoErcDirective => {
                self.place_no_erc_directive_at(signex_types::schematic::Point::new(wx, wy));
            }
            Tool::BusEntry => {
                let size = self.interaction_state.bus_entry_size;
                self.place_bus_entry_at(signex_types::schematic::Point::new(wx, wy), size);
            }
            tool @ (Tool::Line | Tool::Rectangle | Tool::Circle | Tool::Arc | Tool::Polyline) => {
                let p = signex_types::schematic::Point::new(wx, wy);
                self.shape_tool_point(tool, p, ClickKind::Click);
            }
            // #1267 / #1300: the Bezier, curve and dimension tools take one
            // point per click; the Table tool places on its first.
            tool @ (Tool::Bezier
            | Tool::Ellipse
            | Tool::EllipticalArc
            | Tool::RoundRectangle
            | Tool::LinearDimension
            | Tool::AlignedDimension
            | Tool::Table) => {
                self.annotation_tool_point(tool, signex_types::schematic::Point::new(wx, wy));
            }
            Tool::Sheet => return self.place_sheet_click(wx, wy),
            Tool::SheetEntry => self.place_sheet_entry_click(wx, wy),
            Tool::Measure => self.measure_click(wx, wy),
            Tool::BreakWire => self.break_wire_click(wx, wy),
            tool @ (Tool::Blanket | Tool::CompileMask) => {
                let p = signex_types::schematic::Point::new(wx, wy);
                self.directive_region_point(tool, p, ClickKind::Click);
            }
            Tool::DiffPair => self.diff_pair_click(signex_types::schematic::Point::new(wx, wy)),
            Tool::NetParameters => {
                self.net_parameters_click(signex_types::schematic::Point::new(wx, wy));
            }
            Tool::Text => {
                let note_text = self
                    .document_state
                    .panel_ctx
                    .pre_placement
                    .as_ref()
                    .map(|pp| pp.label_text.clone())
                    .unwrap_or_else(|| "Text".to_string());
                if self.place_text_at(
                    signex_types::schematic::Point::new(wx, wy),
                    note_text.clone(),
                ) {
                    self.text_note_placed(&note_text);
                }
                self.interaction_state.current_tool = Tool::Select;
            }
            Tool::Label => {
                let spec = self.label_spec_from_form();
                let placed_text = spec.text.clone();
                if self.place_label_at(signex_types::schematic::Point::new(wx, wy), spec) {
                    self.label_placed(&placed_text);
                }
            }
            _ => {
                return self.handle_selection_request(selection_request::SelectionRequest::HitAt {
                    world_x,
                    world_y,
                });
            }
        }
        Task::none()
    }

    /// A wire belongs to the canvas it started on: a click on another one
    /// must not commit a segment from that sheet's point into this sheet.
    /// The old wire ends, so the click starts a new one here.
    fn end_wire_of_another_canvas(&mut self) {
        let state = &mut self.interaction_state;
        if !state.wire_drawing || state.wire_window == state.acting_window {
            return;
        }
        // Main's own slot is out of reach while a window acts: `run_in_window`
        // clears it when it swaps back.
        if let Some(slot) = state.wire_window.and_then(|id| state.canvases.get_mut(&id)) {
            slot.wire_preview.clear();
            slot.drawing_mode = false;
        }
        state.wire_drawing = false;
        state.wire_points.clear();
    }

    /// The first click of a wire or bus: anchor it at `pt`.
    fn start_wire_drawing(&mut self, pt: signex_types::schematic::Point) {
        self.interaction_state.wire_drawing = true;
        self.interaction_state.wire_window = self.interaction_state.acting_window;
        self.interaction_state.wire_points.clear();
        self.interaction_state.wire_points.push(pt);
        self.interaction_state.wire_start_style = self.wire_style_at(pt);
        let canvas = self.interaction_state.active_canvas_mut();
        canvas.wire_preview = vec![pt];
        canvas.drawing_mode = true;
        canvas.tool_preview = None;
    }

    /// After a committed segment: the next one starts at `end_pt`.
    fn continue_wire_drawing(&mut self, end_pt: signex_types::schematic::Point) {
        self.interaction_state.wire_points = vec![end_pt];
        self.interaction_state.active_canvas_mut().wire_preview = vec![end_pt];
    }

    /// After the wire's second click: no wire in progress, the tool stays
    /// armed and the next click starts a new wire.
    fn end_wire_drawing(&mut self) {
        let state = &mut self.interaction_state;
        state.wire_drawing = false;
        state.wire_points.clear();
        let canvas = state.active_canvas_mut();
        canvas.wire_preview.clear();
        canvas.drawing_mode = false;
    }
}
