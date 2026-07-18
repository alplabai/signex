use super::super::*;

impl SchematicCanvas {
    /// Layer 2 — the schematic content (cached unless panning/dragging).
    pub(in crate::canvas) fn draw_content(
        &self,
        state: &CanvasState,
        renderer: &Renderer,
        bounds: Rectangle,
        effective_snapshot: Option<&crate::schematic_runtime::SchematicRenderSnapshot>,
        drag_offset: Option<(f64, f64)>,
    ) -> canvas::Geometry {
        let live_transform = crate::schematic_runtime::ScreenTransform {
            offset_x: state.camera.offset.x,
            offset_y: state.camera.offset.y,
            scale: state.camera.scale,
        };
        // Publish the live camera every frame so world-anchored overlays
        // (inline editor) can track pan/zoom without waiting on cache rebuilds.
        self.live_camera.set((
            state.camera.offset.x,
            state.camera.offset.y,
            state.camera.scale,
        ));
        let (cached_offset_x, cached_offset_y, cached_scale) = self.content_cache_camera.get();
        let camera_matches_cache = (cached_offset_x - state.camera.offset.x).abs() < 0.01
            && (cached_offset_y - state.camera.offset.y).abs() < 0.01
            && (cached_scale - state.camera.scale).abs() < 0.0001;
        let focus_set = self.auto_focus_set();
        let focus_ref = focus_set.as_ref();
        if state.panning || drag_offset.is_some() {
            let mut frame = canvas::Frame::new(renderer, bounds.size());
            if let Some(snapshot) = effective_snapshot {
                crate::schematic_runtime::render_schematic(
                    &mut frame,
                    snapshot,
                    &live_transform,
                    &self.canvas_colors,
                    bounds,
                    focus_ref,
                    Some(&self.wire_color_overrides),
                );
            }
            frame.into_geometry()
        } else {
            if !camera_matches_cache {
                self.content_cache.clear();
            }
            self.content_cache.draw(renderer, bounds.size(), |frame| {
                self.content_cache_camera.set((
                    state.camera.offset.x,
                    state.camera.offset.y,
                    state.camera.scale,
                ));
                if let Some(snapshot) = effective_snapshot {
                    crate::schematic_runtime::render_schematic(
                        frame,
                        snapshot,
                        &live_transform,
                        &self.canvas_colors,
                        bounds,
                        focus_ref,
                        Some(&self.wire_color_overrides),
                    );
                }
            })
        }
    }

    /// Layer 2.5 — AutoFocus (F9) dim frame around the selection bbox.
    pub(in crate::canvas) fn draw_autofocus_dim(
        &self,
        state: &CanvasState,
        renderer: &Renderer,
        bounds: Rectangle,
        effective_snapshot: Option<&crate::schematic_runtime::SchematicRenderSnapshot>,
    ) -> Option<canvas::Geometry> {
        // Layer 2.5: AutoFocus dim — when F9 is on and a selection
        // exists, fade everything outside the selection bbox + margin
        // with a translucent dark overlay. Uses four rects forming a
        // frame around the bbox, so 2D paths can express the hole
        // without compositing modes.
        if self.auto_focus
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
                let p_min = state
                    .camera
                    .world_to_screen(iced::Point::new(min_x, min_y), bounds);
                let p_max = state
                    .camera
                    .world_to_screen(iced::Point::new(max_x, max_y), bounds);
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

    /// Layer 3 — selection overlay + ERC markers (uncached while dragging).
    pub(in crate::canvas) fn draw_selection(
        &self,
        state: &CanvasState,
        renderer: &Renderer,
        bounds: Rectangle,
        effective_snapshot: Option<&crate::schematic_runtime::SchematicRenderSnapshot>,
        drag_offset: Option<(f64, f64)>,
    ) -> Option<canvas::Geometry> {
        // Layer 3: selection overlay — always uses live camera (redrawn each frame)
        // During drag we use the shifted snapshot so the selection rectangle
        // travels with the dragged items instead of staying behind.
        if !self.selected.is_empty()
            && let Some(snapshot) = effective_snapshot
        {
            // During a drag we can't rely on overlay_cache — it must redraw
            // with the shifted positions every frame.
            let draw_overlay = |frame: &mut canvas::Frame| {
                let transform = crate::schematic_runtime::ScreenTransform {
                    offset_x: state.camera.offset.x,
                    offset_y: state.camera.offset.y,
                    scale: state.camera.scale,
                };
                crate::schematic_runtime::selection::draw_selection_overlay(
                    frame,
                    snapshot,
                    &self.selected,
                    &transform,
                );
                let erc_overlay: Vec<crate::schematic_runtime::overlay::ErcMarker> = self
                    .erc_markers
                    .iter()
                    .map(|marker| crate::schematic_runtime::overlay::ErcMarker {
                        x_mm: marker.x,
                        y_mm: marker.y,
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
