use super::super::*;

impl SchematicCanvas {
    /// Drag-to-move guide line + connection-point X markers.
    pub(in crate::canvas) fn draw_move_guides(
        &self,
        frame: &mut canvas::Frame,
        state: &CanvasState,
        bounds: Rectangle,
    ) {
        // Drag-to-move: the content layer already renders selected items
        // at the dragged offset (via shifted_snapshot). Here we just handle
        // the symbol-field anchor→moved guide line, which the content
        // render doesn't draw on its own.
        if state.move_dragging
            && let (Some(origin), Some(current)) = (state.move_origin, state.move_current)
        {
            let dx = (current.0 - origin.0) as f32;
            let dy = (current.1 - origin.1) as f32;
            if let Some(render_cache) = self.active_render_cache()
                && let Some(preview) = render_cache.prepared_preview()
            {
                for sel in &self.selected {
                    if matches!(
                        sel.kind,
                        signex_types::schematic::SelectedKind::SymbolRefField
                            | signex_types::schematic::SelectedKind::SymbolValField
                    ) {
                        let anchor_pos = preview.symbol_position(sel.uuid);
                        let moved_pos = match sel.kind {
                            signex_types::schematic::SelectedKind::SymbolRefField => {
                                preview.symbol_reference_position(sel.uuid)
                            }
                            signex_types::schematic::SelectedKind::SymbolValField => {
                                preview.symbol_value_position(sel.uuid)
                            }
                            _ => None,
                        };

                        if let (Some((anchor_x, anchor_y)), Some((field_x, field_y))) =
                            (anchor_pos, moved_pos)
                        {
                            let anchor = state.camera.world_to_screen(
                                iced::Point::new(anchor_x as f32, anchor_y as f32),
                                bounds,
                            );
                            let moved = state.camera.world_to_screen(
                                iced::Point::new(field_x as f32 + dx, field_y as f32 + dy),
                                bounds,
                            );

                            let guide = canvas::Path::line(anchor, moved);
                            frame.stroke(
                                &guide,
                                canvas::Stroke::default()
                                    .with_color(Color::from_rgb(0.3, 0.7, 1.0))
                                    .with_width(1.0),
                            );

                            let anchor_circle = canvas::Path::circle(anchor, 3.0);
                            frame.fill(&anchor_circle, Color::from_rgb(0.3, 0.7, 1.0));
                        }
                    }
                }
            }

            // Altium-style connection-point markers: small thin X at every
            // pin / wire-end / junction position of the dragged objects.
            let snapshot_live = self.active_snapshot();
            if let Some(snap) = snapshot_live {
                let x_color = Color::from_rgb(1.0, 0.3, 0.3);
                let x_stroke = canvas::Stroke::default()
                    .with_color(x_color)
                    .with_width(1.0);
                let draw_x = |frame: &mut canvas::Frame, screen: iced::Point| {
                    let r = 4.0;
                    frame.stroke(
                        &canvas::Path::line(
                            iced::Point::new(screen.x - r, screen.y - r),
                            iced::Point::new(screen.x + r, screen.y + r),
                        ),
                        x_stroke,
                    );
                    frame.stroke(
                        &canvas::Path::line(
                            iced::Point::new(screen.x - r, screen.y + r),
                            iced::Point::new(screen.x + r, screen.y - r),
                        ),
                        x_stroke,
                    );
                };
                for sel in &self.selected {
                    use signex_types::schematic::{Point, SelectedKind};
                    let dxf = dx as f64;
                    let dyf = dy as f64;
                    match sel.kind {
                        SelectedKind::Wire => {
                            if let Some(w) = snap.wires.iter().find(|w| w.uuid == sel.uuid) {
                                for p in [w.start, w.end] {
                                    let s = state.camera.world_to_screen(
                                        iced::Point::new((p.x + dxf) as f32, (p.y + dyf) as f32),
                                        bounds,
                                    );
                                    draw_x(&mut *frame, s);
                                }
                            }
                        }
                        SelectedKind::Bus => {
                            if let Some(b) = snap.buses.iter().find(|b| b.uuid == sel.uuid) {
                                for p in [b.start, b.end] {
                                    let s = state.camera.world_to_screen(
                                        iced::Point::new((p.x + dxf) as f32, (p.y + dyf) as f32),
                                        bounds,
                                    );
                                    draw_x(&mut *frame, s);
                                }
                            }
                        }
                        SelectedKind::Junction => {
                            if let Some(j) = snap.junctions.iter().find(|j| j.uuid == sel.uuid) {
                                let s = state.camera.world_to_screen(
                                    iced::Point::new(
                                        (j.position.x + dxf) as f32,
                                        (j.position.y + dyf) as f32,
                                    ),
                                    bounds,
                                );
                                draw_x(&mut *frame, s);
                            }
                        }
                        SelectedKind::Label => {
                            if let Some(l) = snap.labels.iter().find(|l| l.uuid == sel.uuid) {
                                let s = state.camera.world_to_screen(
                                    iced::Point::new(
                                        (l.position.x + dxf) as f32,
                                        (l.position.y + dyf) as f32,
                                    ),
                                    bounds,
                                );
                                draw_x(&mut *frame, s);
                            }
                        }
                        SelectedKind::NoConnect => {
                            if let Some(nc) = snap.no_connects.iter().find(|n| n.uuid == sel.uuid) {
                                let s = state.camera.world_to_screen(
                                    iced::Point::new(
                                        (nc.position.x + dxf) as f32,
                                        (nc.position.y + dyf) as f32,
                                    ),
                                    bounds,
                                );
                                draw_x(&mut *frame, s);
                            }
                        }
                        SelectedKind::SheetPin => {
                            if let Some(pin) = snap
                                .child_sheets
                                .iter()
                                .find_map(|cs| cs.pins.iter().find(|pin| pin.uuid == sel.uuid))
                            {
                                let s = state.camera.world_to_screen(
                                    iced::Point::new(
                                        (pin.position.x + dxf) as f32,
                                        (pin.position.y + dyf) as f32,
                                    ),
                                    bounds,
                                );
                                draw_x(&mut *frame, s);
                            }
                        }
                        SelectedKind::Symbol => {
                            if let Some(sym) = snap.symbols.iter().find(|s| s.uuid == sel.uuid)
                                && let Some(lib_sym) = snap.lib_symbols.get(&sym.lib_id)
                            {
                                // Build a shifted copy so instance_transform
                                // uses the dragged position.
                                let mut shifted = sym.clone();
                                shifted.position =
                                    Point::new(sym.position.x + dxf, sym.position.y + dyf);
                                for lp in &lib_sym.pins {
                                    if lp.unit != 0 && lp.unit != sym.unit {
                                        continue;
                                    }
                                    let p = &lp.pin;
                                    let (wx, wy) = crate::schematic_runtime::instance_transform(
                                        &shifted,
                                        &p.position,
                                    );
                                    let s = state.camera.world_to_screen(
                                        iced::Point::new(wx as f32, wy as f32),
                                        bounds,
                                    );
                                    draw_x(&mut *frame, s);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    /// Drag-to-select (box-select) rectangle.
    pub(in crate::canvas) fn draw_select_rect(
        &self,
        frame: &mut canvas::Frame,
        state: &CanvasState,
        bounds: Rectangle,
    ) {
        // Drag-to-select rectangle
        if let (Some(start), Some(end)) = (state.select_drag_start, state.select_drag_end) {
            let s1 = state
                .camera
                .world_to_screen(iced::Point::new(start.0 as f32, start.1 as f32), bounds);
            let s2 = state
                .camera
                .world_to_screen(iced::Point::new(end.0 as f32, end.1 as f32), bounds);
            let x = s1.x.min(s2.x);
            let y = s1.y.min(s2.y);
            let w = (s2.x - s1.x).abs();
            let h = (s2.y - s1.y).abs();
            if w > 2.0 || h > 2.0 {
                // Fill (semi-transparent blue)
                frame.fill_rectangle(
                    iced::Point::new(x, y),
                    iced::Size::new(w, h),
                    Color::from_rgba(0.2, 0.4, 0.8, 0.15),
                );
                // Border (dashed blue)
                let rect_path =
                    canvas::Path::rectangle(iced::Point::new(x, y), iced::Size::new(w, h));
                frame.stroke(
                    &rect_path,
                    canvas::Stroke::default()
                        .with_color(Color::from_rgba(0.3, 0.5, 1.0, 0.7))
                        .with_width(1.0),
                );
            }
        }
    }
}
