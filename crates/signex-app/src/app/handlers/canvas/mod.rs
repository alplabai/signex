use iced::Task;
use signex_types::coord::Unit;

use super::super::*;

mod clicked;
mod double_clicked;
mod layout_drag;
mod move_pick;
mod sheet_tool;
mod sync_sheet_pins;
#[cfg(test)]
mod tests;

/// Default stroke width applied when the user hasn't edited the
/// pre_placement Width value yet. Standard's "default line width"
/// is ~0.15 mm in schematics; showing 0 in the properties panel
/// used to confuse users because the line was still visible
/// (renderer substitutes its own default for 0).
const DEFAULT_SHAPE_STROKE_MM: f64 = 0.15;

/// Read the shape width + fill defaults out of the current
/// pre_placement slot (TAB-configured) so shape tools pick up the
/// user's Width/Fill edits when committing the next click.
fn pre_placement_shape(
    doc: &super::super::state::DocumentState,
) -> (f64, signex_types::schematic::FillType) {
    doc.panel_ctx
        .pre_placement
        .as_ref()
        .map(|pp| {
            let w = if pp.shape_width_mm > 0.0 {
                pp.shape_width_mm
            } else {
                DEFAULT_SHAPE_STROKE_MM
            };
            (w, pp.shape_fill)
        })
        .unwrap_or((
            DEFAULT_SHAPE_STROKE_MM,
            signex_types::schematic::FillType::None,
        ))
}

impl Signex {
    pub(crate) fn handle_canvas_interaction_event(&mut self, event: CanvasEvent) -> Task<Message> {
        match event {
            CanvasEvent::CursorAt { x, y, zoom_pct } => {
                self.ui_state.cursor_x = x as f64;
                self.ui_state.cursor_y = y as f64;
                if self.ui_state.cursor_document.as_deref() != self.cursor_document_now() {
                    self.ui_state.cursor_document =
                        self.cursor_document_now().map(std::path::Path::to_path_buf);
                }
                self.ui_state.zoom = zoom_pct;
                // Hover detection: fast hit-test against the active
                // schematic snapshot so the tooltip overlay (in
                // `view::collect_overlays`) can show after a 250 ms
                // dwell on a placed symbol. Snapshot lookups are cheap
                // (Vec scan keyed by world bounds), so doing this on
                // every cursor tick is fine. Other hit kinds (wires,
                // labels) intentionally fall through — only Symbol
                // hovers carry library metadata worth surfacing.
                let hover_uuid: Option<uuid::Uuid> = self
                    .interaction_state
                    .active_canvas()
                    .active_snapshot()
                    .and_then(|snap| {
                        crate::schematic_runtime::hit_test::hit_test(snap, x as f64, y as f64)
                    })
                    .and_then(|hit| {
                        matches!(hit.kind, signex_types::schematic::SelectedKind::Symbol)
                            .then_some(hit.uuid)
                    });
                if hover_uuid != self.interaction_state.hover_symbol_uuid {
                    self.interaction_state.hover_symbol_uuid = hover_uuid;
                    self.interaction_state.hover_started_at =
                        hover_uuid.map(|_| std::time::Instant::now());
                }
                // Track screen position for the tooltip's translate
                // offset on every move, even if the symbol is the
                // same (the card follows the cursor).
                self.interaction_state.hover_screen_pos = if hover_uuid.is_some() {
                    Some(self.interaction_state.last_mouse_pos)
                } else {
                    None
                };
                // Lasso auto-sample: while a lasso is anchored (>= 1
                // vertex already committed), sample a new vertex each
                // time the cursor moves more than SAMPLE_MIN from the
                // last recorded point. Produces a freehand polyline
                // between the two clicks (Altium's behaviour).
                // Sample at a constant ~6 screen pixels so the lasso
                // tracks the cursor at all zoom levels. At 100% zoom
                // (scale=3 px/mm) that's 2.0 mm world; at 50% zoom
                // we need 4.0 mm world for the same screen distance.
                // `zoom_pct` comes from `camera.zoom_percent()` which
                // is `scale/3 * 100`, so mm-per-6px = 200 / zoom_pct.
                const SAMPLE_MIN_PX: f64 = 6.0;
                let sample_min_mm = if zoom_pct > 1.0 {
                    (SAMPLE_MIN_PX * 100.0) / (zoom_pct * 3.0)
                } else {
                    2.0
                };
                let sampled = if let Some(pts) = self.ui_state.lasso_polygon.as_mut()
                    && let Some(&last) = pts.last()
                {
                    let dx = x as f64 - last.x;
                    let dy = y as f64 - last.y;
                    if (dx * dx + dy * dy).sqrt() >= sample_min_mm {
                        pts.push(signex_types::schematic::Point::new(x as f64, y as f64));
                        true
                    } else {
                        false
                    }
                } else {
                    false
                };
                if sampled {
                    self.sync_lasso_polygon_to_canvas();
                }
            }
            CanvasEvent::Clicked { world_x, world_y } => {
                return self.handle_canvas_clicked(world_x, world_y);
            }
            CanvasEvent::MoveSelected { dx, dy } => {
                // Snap so the PRIMARY selected item's connection point (its
                // stored `position`) lands on a grid dot after the move, not
                // just the drag delta. Snapping only the delta preserves an
                // off-grid origin; users expect the endpoint to be on-grid
                // like Standard/Altium do.
                let (dx, dy) = crate::canvas::drag_preview::snapped_move_delta(
                    self.active_render_snapshot(),
                    &self.interaction_state.active_canvas().selected,
                    dx,
                    dy,
                    self.ui_state.snap_enabled,
                    crate::canvas::grid::pitch_mm_exact(self.ui_state.grid_size_mm),
                );
                if (dx.abs() > 0.001 || dy.abs() > 0.001)
                    && !self
                        .interaction_state
                        .active_canvas_mut()
                        .selected
                        .is_empty()
                {
                    self.apply_engine_command(
                        signex_engine::Command::MoveSelection {
                            items: self.interaction_state.active_canvas().selected.clone(),
                            dx,
                            dy,
                            keep_wires: self.ui_state.move_keeps_wires,
                        },
                        true,
                        true,
                    );
                }
            }
            CanvasEvent::MovePicked { world_x, world_y } => {
                self.handle_canvas_move_picked(world_x, world_y);
            }
            CanvasEvent::DoubleClicked {
                world_x,
                world_y,
                screen_x: _,
                screen_y: _,
            } => return self.handle_canvas_double_clicked(world_x, world_y),
            CanvasEvent::BoxSelect { x1, y1, x2, y2 } => {
                return self.handle_selection_request(
                    selection_request::SelectionRequest::BoxSelect { x1, y1, x2, y2 },
                );
            }
            CanvasEvent::CursorMoved => {
                self.interaction_state.active_canvas_mut().clear_bg_cache();
                self.interaction_state
                    .active_canvas_mut()
                    .clear_overlay_cache();
                self.interaction_state.pcb_canvas.clear_bg_cache();
                self.interaction_state
                    .active_canvas_mut()
                    .pending_fit
                    .set(None);
                self.interaction_state.pcb_canvas.pending_fit.set(None);
            }
            CanvasEvent::CenterAtCursor => {
                // `cursor_x/y` is the world position `CursorAt` last reported.
                // It means nothing in another document (a tab switched with
                // the keyboard, another window), nor before the first hover.
                if self.ui_state.cursor_document.is_none()
                    || self.ui_state.cursor_document.as_deref() != self.cursor_document_now()
                {
                    return Task::none();
                }
                let (x, y) = (self.ui_state.cursor_x as f32, self.ui_state.cursor_y as f32);
                // A user camera move cancels a deferred fit, as in `CursorMoved`;
                // otherwise a queued fit lands afterwards and undoes the centring.
                self.interaction_state
                    .active_canvas_mut()
                    .pending_fit
                    .set(None);
                self.interaction_state.pcb_canvas.pending_fit.set(None);
                if self.has_active_schematic() {
                    self.interaction_state
                        .active_canvas_mut()
                        .center_on_world(x, y);
                } else if self.has_active_pcb() {
                    self.interaction_state.pcb_canvas.center_on_world(x, y);
                }
                self.clear_active_camera_caches();
            }
            CanvasEvent::FitAll => {
                if self.has_active_schematic() {
                    self.interaction_state.active_canvas_mut().fit_to_content();
                } else if self.has_active_pcb() {
                    self.interaction_state.pcb_canvas.fit_to_board();
                }
                self.clear_active_camera_caches();
            }
            CanvasEvent::CtrlClicked { world_x, world_y } => {
                if let Some(snapshot) = self.active_render_snapshot()
                    && let Some(hit) =
                        crate::schematic_runtime::hit_test::hit_test(snapshot, world_x, world_y)
                    && crate::app::handlers::selection_workflow::passes_filter(
                        &hit,
                        snapshot,
                        &self.interaction_state.selection_filters,
                    )
                {
                    if let Some(pos) = self
                        .interaction_state
                        .canvas
                        .selected
                        .iter()
                        .position(|s| s.uuid == hit.uuid)
                    {
                        self.interaction_state
                            .active_canvas_mut()
                            .selected
                            .remove(pos);
                    } else {
                        self.interaction_state
                            .active_canvas_mut()
                            .selected
                            .push(hit);
                    }
                    self.interaction_state
                        .active_canvas_mut()
                        .clear_overlay_cache();
                    self.update_selection_info();
                }
            }
        }

        Task::none()
    }

    pub(crate) fn handle_unit_cycle_request(&mut self) {
        self.ui_state.unit = match self.ui_state.unit {
            Unit::Mm => Unit::Mil,
            Unit::Mil => Unit::Inch,
            Unit::Inch => Unit::Micrometer,
            Unit::Micrometer => Unit::Mm,
        };
        crate::fonts::write_unit_pref(self.ui_state.unit);
    }

    pub(crate) fn resolve_child_sheet_path(
        &self,
        child_filename: &str,
    ) -> Option<std::path::PathBuf> {
        // #339/#406 — resolve a relative child reference against the directory
        // of the sheet that CARRIES the reference (the active tab's path), not
        // the project directory. This matches `project_graph` / the
        // netlist, so navigation lands on the same file the netlist stitched
        // instead of a phantom sibling beside the `.snxprj`. Falls back to the
        // project directory when the active tab is unsaved (no path). Both call
        // sites go through the one shared resolver (`resolve_child_reference`),
        // which also handles the empty and absolute-reference cases.
        let parent_dir = self
            .active_tab_path()
            .and_then(|path| path.parent().map(std::path::PathBuf::from))
            .or_else(|| {
                self.document_state
                    .active_loaded_project()
                    .and_then(|p| p.path.parent().map(std::path::PathBuf::from))
            })
            .unwrap_or_default();
        crate::app::project_sheets::resolve_child_reference(&parent_dir, child_filename)
    }

    pub(crate) fn open_or_focus_child_sheet(&mut self, child_filename: &str) {
        let Some(path) = self.resolve_child_sheet_path(child_filename) else {
            return;
        };
        self.open_or_focus_sheet_path(path);
    }

    /// Focus the tab already showing `path`, or open it as a new schematic tab.
    /// The Navigator opens sheets by the path its hierarchy resolved; a
    /// canvas double-click reaches here through `open_or_focus_child_sheet`.
    pub(crate) fn open_or_focus_sheet_path(&mut self, path: std::path::PathBuf) {
        let path = self.resolve_held_spelling(path);
        if !path.exists() {
            crate::diagnostics::log_info(format!("Child-sheet file not found: {}", path.display()));
            return;
        }

        if let Some(index) = self
            .document_state
            .tabs
            .iter()
            .position(|tab| tab.path == path)
        {
            if index != self.document_state.active_tab {
                self.park_active_schematic_session();
                self.document_state.active_tab = index;
                self.sync_active_tab();
            }
            return;
        }

        // A dirty engine parked without a tab holds unsaved edits: reattach
        // it rather than replace it with the on-disk sheet (#1135).
        if self.path_is_owned_by_open_document(&path) {
            let title = path
                .file_stem()
                .map(|stem| stem.to_string_lossy().to_string())
                .unwrap_or_else(|| "Schematic".to_string());
            self.attach_parked_schematic_tab(path, title);
            return;
        }

        let parse_result = std::fs::read_to_string(&path)
            .map_err(anyhow::Error::from)
            .and_then(|text| {
                signex_types::format::SnxSchematic::parse(&text)
                    .map(|snx| snx.sheet)
                    .map_err(anyhow::Error::from)
            });
        match parse_result {
            Ok(sheet) => {
                let title = path
                    .file_stem()
                    .map(|stem| stem.to_string_lossy().to_string())
                    .unwrap_or_else(|| "Schematic".to_string());
                self.open_schematic_tab(path, title, sheet);
            }
            Err(error) => {
                crate::diagnostics::log_info(format!(
                    "Failed to open child-sheet schematic from double-click: {}",
                    error
                ));
            }
        }
    }

    /// File reference of the first selected sheet symbol on the active sheet.
    pub(crate) fn selected_child_sheet_filename(&self) -> Option<String> {
        // Read the engine document (the source of truth) rather than the
        // render snapshot, which is only a derived copy.
        let document = self.document_state.active_engine()?.document();
        self.interaction_state
            .active_canvas()
            .selected
            .iter()
            .find(|item| item.kind == signex_types::schematic::SelectedKind::ChildSheet)
            .and_then(|item| {
                document
                    .child_sheets
                    .iter()
                    .find(|sheet| sheet.uuid == item.uuid)
                    .map(|sheet| sheet.filename.clone())
            })
    }

    /// File references of every selected sheet symbol on the active sheet,
    /// each distinct file once, in selection order.
    pub(crate) fn selected_child_sheet_filenames(&self) -> Vec<String> {
        let Some(engine) = self.document_state.active_engine() else {
            return Vec::new();
        };
        let document = engine.document();
        let mut filenames: Vec<String> = Vec::new();
        for item in &self.interaction_state.active_canvas().selected {
            if item.kind != signex_types::schematic::SelectedKind::ChildSheet {
                continue;
            }
            let Some(sheet) = document.child_sheets.iter().find(|s| s.uuid == item.uuid) else {
                continue;
            };
            if !filenames.contains(&sheet.filename) {
                filenames.push(sheet.filename.clone());
            }
        }
        filenames
    }

    pub(crate) fn open_selected_child_sheet(&mut self) -> bool {
        if let Some(filename) = self.selected_child_sheet_filename() {
            self.open_or_focus_child_sheet(filename.as_str());
            true
        } else {
            false
        }
    }

    /// The document a reported cursor point belongs to: the active schematic,
    /// else the active tab (a PCB has no `active_path`). `active_path` comes
    /// first because an undocked schematic window's event swaps it but not
    /// `active_tab`. An undocked PCB window's event swaps both (#494), and
    /// both then name its tab.
    fn cursor_document_now(&self) -> Option<&std::path::Path> {
        self.document_state.active_path.as_deref().or_else(|| {
            self.document_state
                .tabs
                .get(self.document_state.active_tab)
                .map(|tab| tab.path.as_path())
        })
    }

    /// Redraw the active schematic or PCB canvas after its camera moved.
    fn clear_active_camera_caches(&mut self) {
        if self.has_active_schematic() {
            let canvas = self.interaction_state.active_canvas_mut();
            canvas.clear_bg_cache();
            canvas.clear_content_cache();
        } else if self.has_active_pcb() {
            let canvas = &mut self.interaction_state.pcb_canvas;
            canvas.clear_bg_cache();
            canvas.clear_content_cache();
        }
    }
}
