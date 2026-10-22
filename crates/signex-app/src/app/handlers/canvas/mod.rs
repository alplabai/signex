use iced::Task;
use signex_types::coord::Unit;

use super::super::*;

mod clicked;
mod double_clicked;
mod layout_drag;
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
                let (dx, dy) = if self.ui_state.snap_enabled {
                    let gs = self.ui_state.grid_size_mm as f64;
                    let primary = self
                        .interaction_state
                        .canvas
                        .selected
                        .first()
                        .and_then(|item| {
                            let snap = self.active_render_snapshot()?;
                            primary_anchor_world(snap, item)
                        });
                    if let Some((px, py)) = primary {
                        let target_x = px + dx;
                        let target_y = py + dy;
                        let snapped_x = (target_x / gs).round() * gs;
                        let snapped_y = (target_y / gs).round() * gs;
                        (snapped_x - px, snapped_y - py)
                    } else {
                        ((dx / gs).round() * gs, (dy / gs).round() * gs)
                    }
                } else {
                    (dx, dy)
                };
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
                        },
                        true,
                        true,
                    );
                }
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
            CanvasEvent::FitAll => {
                if self.has_active_schematic() {
                    self.interaction_state.active_canvas_mut().fit_to_paper();
                    self.interaction_state.active_canvas_mut().clear_bg_cache();
                    self.interaction_state
                        .active_canvas_mut()
                        .clear_content_cache();
                } else if self.has_active_pcb() {
                    self.interaction_state.pcb_canvas.fit_to_board();
                    self.interaction_state.pcb_canvas.clear_bg_cache();
                    self.interaction_state.pcb_canvas.clear_content_cache();
                }
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

    fn resolve_child_sheet_path(&self, child_filename: &str) -> Option<std::path::PathBuf> {
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

    pub(crate) fn open_selected_child_sheet(&mut self) -> bool {
        let Some(snapshot) = self.active_render_snapshot() else {
            return false;
        };

        let filename = self
            .interaction_state
            .active_canvas()
            .selected
            .iter()
            .find(|item| item.kind == signex_types::schematic::SelectedKind::ChildSheet)
            .and_then(|item| {
                snapshot
                    .child_sheets
                    .iter()
                    .find(|sheet| sheet.uuid == item.uuid)
                    .map(|sheet| sheet.filename.clone())
            });

        if let Some(filename) = filename {
            self.open_or_focus_child_sheet(filename.as_str());
            true
        } else {
            false
        }
    }
}

/// Resolve a selected item's primary anchor — the world point that should
/// snap to the grid (connection point for labels/wires/symbols, etc.).
fn primary_anchor_world(
    snap: &crate::schematic_runtime::SchematicRenderSnapshot,
    item: &signex_types::schematic::SelectedItem,
) -> Option<(f64, f64)> {
    use signex_types::schematic::SelectedKind;
    match item.kind {
        SelectedKind::Label => snap
            .labels
            .iter()
            .find(|l| l.uuid == item.uuid)
            .map(|l| (l.position.x, l.position.y)),
        SelectedKind::Symbol => snap
            .symbols
            .iter()
            .find(|s| s.uuid == item.uuid)
            .map(|s| (s.position.x, s.position.y)),
        SelectedKind::Wire => snap
            .wires
            .iter()
            .find(|w| w.uuid == item.uuid)
            .map(|w| (w.start.x, w.start.y)),
        SelectedKind::Bus => snap
            .buses
            .iter()
            .find(|b| b.uuid == item.uuid)
            .map(|b| (b.start.x, b.start.y)),
        SelectedKind::Junction => snap
            .junctions
            .iter()
            .find(|j| j.uuid == item.uuid)
            .map(|j| (j.position.x, j.position.y)),
        SelectedKind::NoConnect => snap
            .no_connects
            .iter()
            .find(|n| n.uuid == item.uuid)
            .map(|n| (n.position.x, n.position.y)),
        SelectedKind::TextNote => snap
            .text_notes
            .iter()
            .find(|t| t.uuid == item.uuid)
            .map(|t| (t.position.x, t.position.y)),
        SelectedKind::ChildSheet => snap
            .child_sheets
            .iter()
            .find(|c| c.uuid == item.uuid)
            .map(|c| (c.position.x, c.position.y)),
        SelectedKind::SheetPin => snap
            .child_sheets
            .iter()
            .find_map(|c| c.pins.iter().find(|pin| pin.uuid == item.uuid))
            .map(|pin| (pin.position.x, pin.position.y)),
        SelectedKind::SymbolRefField => snap
            .symbols
            .iter()
            .find(|s| s.uuid == item.uuid)
            .and_then(|s| s.ref_text.as_ref().map(|rt| (rt.position.x, rt.position.y))),
        SelectedKind::SymbolValField => snap
            .symbols
            .iter()
            .find(|s| s.uuid == item.uuid)
            .and_then(|s| s.val_text.as_ref().map(|vt| (vt.position.x, vt.position.y))),
        _ => None,
    }
}
