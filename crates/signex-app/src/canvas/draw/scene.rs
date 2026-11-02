use super::super::*;

impl SchematicCanvas<'_> {
    /// Whether the retained content layer was drawn at the camera as it is now,
    /// within a sub-pixel pan and a negligible zoom.
    pub(in crate::canvas) fn content_cache_fits_camera(&self) -> bool {
        let cam = self.camera();
        let (x, y, scale) = self.content_cache_camera.get();
        (x - cam.offset.x).abs() < 0.01
            && (y - cam.offset.y).abs() < 0.01
            && (scale - cam.scale).abs() < 0.0001
    }

    /// Layer 2 — the schematic content (cached unless panning/dragging).
    pub(in crate::canvas) fn draw_content(
        &self,
        state: &CanvasState,
        renderer: &Renderer,
        bounds: Rectangle,
        effective_snapshot: Option<&crate::schematic_runtime::SchematicRenderSnapshot>,
        drag_offset: Option<(f64, f64)>,
    ) -> canvas::Geometry {
        let cam = self.camera();
        let live_transform = crate::schematic_runtime::ScreenTransform {
            offset_x: cam.offset.x,
            offset_y: cam.offset.y,
            scale: cam.scale,
            device_px_per_logical_px: self.prefs.device_px_per_logical_px,
        };
        // #632 — this used to republish the camera into a
        // `live_camera: Cell<(f32, f32, f32)>` on every frame so `view` could
        // place world-anchored overlays. `view` now reads
        // `CanvasSlot::live_camera()` off the same cell `draw` reads,
        // so there is nothing to publish and nothing that can lag a frame.
        let camera_matches_cache = self.content_cache_fits_camera();
        // The frame is local; cull against the widget size, not its
        // window-space origin.
        let bounds = crate::schematic_runtime::frame_local_bounds(bounds);
        let focus_set = self.auto_focus_set();
        let focus_ref = focus_set.as_ref();
        if state.panning || drag_offset.is_some() {
            let mut frame = canvas::Frame::new(renderer, bounds.size());
            if let Some(snapshot) = effective_snapshot {
                if drag_offset.is_some() {
                    // The drag snapshot is rebuilt every frame, so its scene
                    // cannot be cached.
                    crate::schematic_runtime::render_schematic(
                        &mut frame,
                        snapshot,
                        &live_transform,
                        &self.prefs.canvas_colors,
                        bounds,
                        focus_ref,
                        self.scene_overrides(),
                    );
                } else {
                    self.render_content_scene(
                        &mut frame,
                        snapshot,
                        &live_transform,
                        bounds,
                        focus_ref,
                    );
                }
            }
            frame.into_geometry()
        } else {
            if !camera_matches_cache {
                self.content_cache.clear();
            }
            self.content_cache.draw(renderer, bounds.size(), |frame| {
                self.content_cache_camera
                    .set((cam.offset.x, cam.offset.y, cam.scale));
                if let Some(snapshot) = effective_snapshot {
                    self.render_content_scene(frame, snapshot, &live_transform, bounds, focus_ref);
                }
            })
        }
    }

    /// Layer 2 during a move-drag: the retained static layer, then the moving
    /// items on top (#1335). A moving item draws above every unmoved one.
    pub(in crate::canvas) fn draw_drag_content(
        &self,
        renderer: &Renderer,
        bounds: Rectangle,
        frame: &super::super::drag_layers::DragFrame,
    ) -> [canvas::Geometry; 2] {
        let cam = self.camera();
        let transform = crate::schematic_runtime::ScreenTransform {
            offset_x: cam.offset.x,
            offset_y: cam.offset.y,
            scale: cam.scale,
            device_px_per_logical_px: self.prefs.device_px_per_logical_px,
        };
        let bounds = crate::schematic_runtime::frame_local_bounds(bounds);
        let focus_set = self.auto_focus_set();
        let focus_ref = focus_set.as_ref();
        let colors = &self.prefs.canvas_colors;
        let overrides = self.scene_overrides();
        let revision = self.render_cache.as_ref().map_or(0, |c| c.revision());

        let static_layer = {
            let mut layers = self.drag_layers.borrow_mut();
            layers.refresh_static_key(
                &super::super::drag_layers::StaticView {
                    transform: &transform,
                    colors,
                    viewport: bounds.size(),
                    focus_set: focus_ref,
                    overrides,
                },
                revision,
                &frame.skip,
            );
            layers.static_cache.draw(renderer, bounds.size(), |target| {
                if let Some(snap) = self.active_snapshot() {
                    let rest = super::super::drag_layers::sheet_without(snap, &frame.skip);
                    crate::schematic_runtime::render_schematic(
                        target, &rest, &transform, colors, bounds, focus_ref, overrides,
                    );
                }
            })
        };

        let mut moving = canvas::Frame::new(renderer, bounds.size());
        crate::schematic_runtime::render_schematic(
            &mut moving,
            &frame.moving,
            &transform,
            colors,
            bounds,
            focus_ref,
            overrides,
        );
        [static_layer, moving.into_geometry()]
    }

    /// Draw `snapshot` through the camera-independent scene cache.
    pub(in crate::canvas) fn render_content_scene(
        &self,
        frame: &mut canvas::Frame,
        snapshot: &crate::schematic_runtime::SchematicRenderSnapshot,
        transform: &crate::schematic_runtime::ScreenTransform,
        bounds: Rectangle,
        focus_set: Option<&std::collections::HashSet<uuid::Uuid>>,
    ) {
        let mut cache = self.content_scene.borrow_mut();
        if let Some(scene) = cache.scene_for(
            snapshot,
            transform,
            &self.prefs.canvas_colors,
            bounds,
            focus_set,
            self.scene_overrides(),
        ) {
            match self.mode {
                ContentMode::Cpu => {
                    crate::schematic_runtime::draw_built_scene(frame, scene, transform);
                }
                // The GPU shader beneath draws everything else.
                ContentMode::GpuOverlay => {
                    crate::schematic_runtime::draw_rotated_text_fallback(frame, scene, transform);
                }
            }
        }
    }

    /// Layer 2.5 — AutoFocus (F9) dim frame around the selection bbox.
    pub(in crate::canvas) fn draw_autofocus_dim(
        &self,
        renderer: &Renderer,
        bounds: Rectangle,
        effective_snapshot: Option<&crate::schematic_runtime::SchematicRenderSnapshot>,
    ) -> Option<canvas::Geometry> {
        let cam = self.camera();
        // Layer 2.5: AutoFocus dim — when F9 is on and a selection
        // exists, fade everything outside the selection bbox + margin
        // with a translucent dark overlay. Uses four rects forming a
        // frame around the bbox, so 2D paths can express the hole
        // without compositing modes.
        if self.prefs.auto_focus
            && !self.selected.is_empty()
            && let Some(snapshot) = effective_snapshot
        {
            use signex_types::schematic::SelectedKind;
            let mut xs: Vec<f32> = Vec::new();
            let mut ys: Vec<f32> = Vec::new();
            let mut push_pt = |x: f64, y: f64, r: f32| {
                xs.push(x as f32 - r);
                xs.push(x as f32 + r);
                ys.push(y as f32 - r);
                ys.push(y as f32 + r);
            };
            for item in &self.selected {
                match item.kind {
                    SelectedKind::Symbol
                    | SelectedKind::SymbolRefField
                    | SelectedKind::SymbolValField => {
                        if let Some(s) = snapshot.symbols.iter().find(|s| s.uuid == item.uuid) {
                            push_pt(s.position.x, s.position.y, 8.0);
                        }
                    }
                    SelectedKind::Wire => {
                        if let Some(w) = snapshot.wires.iter().find(|w| w.uuid == item.uuid) {
                            push_pt(w.start.x, w.start.y, 1.0);
                            push_pt(w.end.x, w.end.y, 1.0);
                        }
                    }
                    SelectedKind::Bus => {
                        if let Some(b) = snapshot.buses.iter().find(|b| b.uuid == item.uuid) {
                            push_pt(b.start.x, b.start.y, 1.0);
                            push_pt(b.end.x, b.end.y, 1.0);
                        }
                    }
                    SelectedKind::Label => {
                        if let Some(l) = snapshot.labels.iter().find(|l| l.uuid == item.uuid) {
                            push_pt(l.position.x, l.position.y, 4.0);
                        }
                    }
                    SelectedKind::Junction | SelectedKind::NoConnect => {
                        if let Some(j) = snapshot.junctions.iter().find(|j| j.uuid == item.uuid) {
                            push_pt(j.position.x, j.position.y, 1.0);
                        } else if let Some(nc) =
                            snapshot.no_connects.iter().find(|n| n.uuid == item.uuid)
                        {
                            push_pt(nc.position.x, nc.position.y, 1.0);
                        }
                    }
                    SelectedKind::NoErcDirective => {
                        if let Some(d) = snapshot
                            .no_erc_directives
                            .iter()
                            .find(|d| d.uuid == item.uuid)
                        {
                            push_pt(d.position.x, d.position.y, 1.0);
                        }
                    }
                    SelectedKind::TextNote => {
                        if let Some(tn) = snapshot.text_notes.iter().find(|t| t.uuid == item.uuid) {
                            push_pt(tn.position.x, tn.position.y, 6.0);
                        }
                    }
                    SelectedKind::ChildSheet => {
                        if let Some(cs) = snapshot.child_sheets.iter().find(|c| c.uuid == item.uuid)
                        {
                            push_pt(cs.position.x, cs.position.y, 0.0);
                            push_pt(cs.position.x + cs.size.0, cs.position.y + cs.size.1, 0.0);
                        }
                    }
                    SelectedKind::SheetPin => {
                        if let Some(pin) = snapshot
                            .child_sheets
                            .iter()
                            .find_map(|cs| cs.pins.iter().find(|pin| pin.uuid == item.uuid))
                        {
                            push_pt(pin.position.x, pin.position.y, 2.0);
                        }
                    }
                    SelectedKind::Drawing => {
                        use signex_types::schematic::SchDrawing;
                        if let Some(d) = snapshot.drawings.iter().find(|d| {
                            let u = match d {
                                SchDrawing::Line { uuid, .. }
                                | SchDrawing::Rect { uuid, .. }
                                | SchDrawing::Circle { uuid, .. }
                                | SchDrawing::Arc { uuid, .. }
                                | SchDrawing::Polyline { uuid, .. } => *uuid,
                            };
                            u == item.uuid
                        }) {
                            match d {
                                SchDrawing::Line { start, end, .. } => {
                                    push_pt(start.x, start.y, 1.0);
                                    push_pt(end.x, end.y, 1.0);
                                }
                                SchDrawing::Rect { start, end, .. } => {
                                    push_pt(start.x, start.y, 1.0);
                                    push_pt(end.x, end.y, 1.0);
                                }
                                SchDrawing::Circle { center, radius, .. } => {
                                    push_pt(center.x, center.y, *radius as f32);
                                }
                                SchDrawing::Arc {
                                    start, mid, end, ..
                                } => {
                                    push_pt(start.x, start.y, 1.0);
                                    push_pt(mid.x, mid.y, 1.0);
                                    push_pt(end.x, end.y, 1.0);
                                }
                                SchDrawing::Polyline { points, .. } => {
                                    for p in points {
                                        push_pt(p.x, p.y, 1.0);
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            if !xs.is_empty() && !ys.is_empty() {
                let min_x = xs.iter().cloned().fold(f32::INFINITY, f32::min);
                let max_x = xs.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
                let min_y = ys.iter().cloned().fold(f32::INFINITY, f32::min);
                let max_y = ys.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
                let p_min = cam.world_to_screen(iced::Point::new(min_x, min_y), bounds);
                let p_max = cam.world_to_screen(iced::Point::new(max_x, max_y), bounds);
                let margin = 30.0_f32;
                let sx0 = p_min.x.min(p_max.x) - margin;
                let sy0 = p_min.y.min(p_max.y) - margin;
                let sx1 = p_min.x.max(p_max.x) + margin;
                let sy1 = p_min.y.max(p_max.y) + margin;
                let dim = iced::Color::from_rgba(0.05, 0.05, 0.08, 0.55);
                let dim_frame = {
                    let mut f = canvas::Frame::new(renderer, bounds.size());
                    let bw = bounds.width;
                    let bh = bounds.height;
                    // Top
                    f.fill_rectangle(
                        iced::Point::new(0.0, 0.0),
                        iced::Size::new(bw, sy0.max(0.0)),
                        dim,
                    );
                    // Bottom
                    f.fill_rectangle(
                        iced::Point::new(0.0, sy1.min(bh)),
                        iced::Size::new(bw, (bh - sy1).max(0.0)),
                        dim,
                    );
                    // Left
                    let mid_h = (sy1.min(bh) - sy0.max(0.0)).max(0.0);
                    f.fill_rectangle(
                        iced::Point::new(0.0, sy0.max(0.0)),
                        iced::Size::new(sx0.max(0.0), mid_h),
                        dim,
                    );
                    // Right
                    f.fill_rectangle(
                        iced::Point::new(sx1.min(bw), sy0.max(0.0)),
                        iced::Size::new((bw - sx1).max(0.0), mid_h),
                        dim,
                    );
                    f.into_geometry()
                };
                return Some(dim_frame);
            }
        }
        None
    }

    /// Whether layer 3 has anything to draw: a selection or an ERC marker.
    pub(in crate::canvas) fn selection_layer_needed(&self) -> bool {
        !self.selected.is_empty() || !self.erc_markers.is_empty()
    }

    /// Layer 3 — selection overlay + ERC markers (uncached while dragging).
    pub(in crate::canvas) fn draw_selection(
        &self,
        renderer: &Renderer,
        bounds: Rectangle,
        effective_snapshot: Option<&crate::schematic_runtime::SchematicRenderSnapshot>,
        drag_offset: Option<(f64, f64)>,
    ) -> Option<canvas::Geometry> {
        let cam = self.camera();
        // Layer 3: selection overlay — always uses live cam (redrawn each frame)
        // During drag we use the shifted snapshot so the selection rectangle
        // travels with the dragged items instead of staying behind.
        // ERC markers live on this layer too, so it must draw whenever there
        // is a marker, not only while something is selected (#1333).
        // Locked objects carry a padlock, so they need the layer too (#1250, #1573).
        if let Some(snapshot) = effective_snapshot
            && (self.selection_layer_needed()
                || crate::schematic_runtime::lock_indicator::has_locked_item(snapshot))
        {
            // During a drag we can't rely on overlay_cache — it must redraw
            // with the shifted positions every frame.
            let draw_overlay = |frame: &mut canvas::Frame| {
                let transform = crate::schematic_runtime::ScreenTransform {
                    offset_x: cam.offset.x,
                    offset_y: cam.offset.y,
                    scale: cam.scale,
                    device_px_per_logical_px: self.prefs.device_px_per_logical_px,
                };
                crate::schematic_runtime::selection::draw_selection_overlay(
                    frame,
                    snapshot,
                    &self.selected,
                    &transform,
                    &self.prefs.canvas_colors,
                    &self.designators,
                );
                crate::schematic_runtime::lock_indicator::draw_lock_indicators(
                    frame,
                    snapshot,
                    &transform,
                    &self.prefs.canvas_colors,
                    &self.designators,
                );
                let erc_overlay: Vec<crate::schematic_runtime::overlay::ErcMarker> = self
                    .erc_markers
                    .iter()
                    .map(|marker| crate::schematic_runtime::overlay::ErcMarker {
                        x_mm: marker.x,
                        y_mm: marker.y,
                        net_level: marker.net_level,
                        severity: match marker.severity {
                            ErcMarkerSeverity::Error => {
                                crate::schematic_runtime::overlay::ErcSeverity::Error
                            }
                            ErcMarkerSeverity::Warning => {
                                crate::schematic_runtime::overlay::ErcSeverity::Warning
                            }
                            ErcMarkerSeverity::Info => {
                                crate::schematic_runtime::overlay::ErcSeverity::Info
                            }
                        },
                    })
                    .collect();
                crate::schematic_runtime::overlay::draw_erc_markers(
                    frame,
                    &erc_overlay,
                    &transform,
                    &self.prefs.canvas_colors,
                );
            };
            if drag_offset.is_some() {
                let mut frame = canvas::Frame::new(renderer, bounds.size());
                draw_overlay(&mut frame);
                return Some(frame.into_geometry());
            } else {
                let sel_overlay = self
                    .overlay_cache
                    .draw(renderer, bounds.size(), draw_overlay);
                return Some(sel_overlay);
            }
        }
        None
    }
}
