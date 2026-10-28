use super::super::*;

/// Radius (px) of the ring marking the hotspot the next wire click lands on.
const HOTSPOT_RING_RADIUS_PX: f32 = 7.0;
/// Stroke width (px) of the hotspot ring. The ring takes the theme's cursor
/// colour, which contrasts with the paper on both light and dark themes.
const HOTSPOT_RING_WIDTH_PX: f32 = 2.0;

impl SchematicCanvas<'_> {
    /// Layer 4 — every-frame overlay: cursor HUD, in-progress previews,
    /// placement ghosts, and drag guides, composed in the original order.
    pub(in crate::canvas) fn draw_overlay(
        &self,
        state: &CanvasState,
        renderer: &Renderer,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> canvas::Geometry {
        let cam = self.camera();
        let mut frame = canvas::Frame::new(renderer, bounds.size());

        if let Some(cursor_pos) = cursor.position_in(bounds) {
            let hotspot = self.wire_hotspot_under(cursor_pos, bounds);
            // Snap cursor visuals to the grid so they match where the click
            // will commit.
            // On a hotspot hit the click lands there, not on the grid dot.
            let cursor_pos = if let Some(h) = hotspot {
                cam.world_to_screen(iced::Point::new(h.x as f32, h.y as f32), bounds)
            } else if self.prefs.snap_enabled && self.prefs.snap_grid_mm > 0.0 {
                let w = cam.screen_to_world(cursor_pos, bounds);
                let g = self.prefs.snap_grid_mm as f32;
                let snapped_w = iced::Point::new((w.x / g).round() * g, (w.y / g).round() * g);
                cam.world_to_screen(snapped_w, bounds)
            } else {
                cursor_pos
            };

            self.draw_placement_x(&mut frame, cursor_pos);
            self.draw_net_color_pen(&mut frame, cursor_pos);
            self.draw_shape_preview(&mut frame, bounds, cursor_pos);
            self.draw_polyline_preview(&mut frame, bounds, cursor_pos);
            self.draw_arc_preview(&mut frame, bounds, cursor_pos);
            self.draw_lasso_preview(&mut frame, bounds, cursor_pos);
            self.draw_wire_preview(&mut frame, bounds, cursor_pos, hotspot);
            self.draw_hotspot_marker(&mut frame, bounds, hotspot);
            self.draw_ghost_symbol(&mut frame, bounds, cursor_pos);
            self.draw_placement_ghost(&mut frame, bounds, cursor_pos);
            self.draw_ghost_text(&mut frame, bounds, cursor_pos);
            self.draw_ghost_bus_entry(&mut frame, bounds, cursor_pos);
            self.draw_ghost_label(&mut frame, bounds, cursor_pos);
            self.draw_tool_chip(&mut frame, cursor_pos);
        }

        self.draw_move_guides(&mut frame, state, bounds);
        self.draw_select_rect(&mut frame, state, bounds);

        frame.into_geometry()
    }

    /// The electrical hotspot a wire click at the cursor would land on, when
    /// the electrical-grid toggle is on and a wire is being placed. Same
    /// helper and radius as the click path, so the marker never disagrees
    /// with where the wire ends up.
    fn wire_hotspot_under(
        &self,
        cursor_pos: iced::Point,
        bounds: Rectangle,
    ) -> Option<signex_types::schematic::Point> {
        if !self.prefs.snap_hotspots || !self.prefs.wire_tool_active {
            return None;
        }
        let grid_mm = if self.prefs.snap_enabled {
            self.prefs.snap_grid_mm
        } else {
            0.0
        };
        let w = self.camera().screen_to_world(cursor_pos, bounds);
        signex_types::schematic::snap_to_hotspot(
            self.active_snapshot()?,
            signex_types::schematic::Point::new(w.x as f64, w.y as f64),
            signex_types::schematic::hotspot_capture_radius(grid_mm),
        )
    }

    /// Ring around the hotspot the next wire click will land on.
    fn draw_hotspot_marker(
        &self,
        frame: &mut canvas::Frame,
        bounds: Rectangle,
        hotspot: Option<signex_types::schematic::Point>,
    ) {
        let Some(h) = hotspot else {
            return;
        };
        let at = self
            .camera()
            .world_to_screen(iced::Point::new(h.x as f32, h.y as f32), bounds);
        let ring = canvas::Path::circle(at, HOTSPOT_RING_RADIUS_PX);
        frame.stroke(
            &ring,
            canvas::Stroke::default()
                .with_color(crate::render_config::to_iced(
                    &self.prefs.canvas_colors.cursor,
                ))
                .with_width(HOTSPOT_RING_WIDTH_PX),
        );
    }

    /// Unified gray placement crosshair shown for every tool/placement mode.
    pub(in crate::canvas) fn draw_placement_x(
        &self,
        frame: &mut canvas::Frame,
        cursor_pos: iced::Point,
    ) {
        // Altium-style placement crosshair: a cyan diagonal
        // X at the cursor, ~28 px across (double the earlier
        // +). Same shape, size, and colour everywhere a
        // placement / tool mode is active so the cursor
        // affordance is uniform.
        let placement_active = self.pending_net_color.is_some()
            || self.lasso_polygon.is_some()
            || self.drawing_mode
            || self.tool_preview.is_some()
            || self.ghost_label.is_some()
            || self.ghost_symbol.is_some()
            || self.prefs.placement.is_some()
            || self.ghost_text.is_some()
            || !self.arc_points.is_empty()
            || !self.polyline_points.is_empty()
            || self.reorder_picker_armed
            || self.move_pick.is_some()
            || self.shape_anchor.is_some();
        if placement_active {
            let len = 14.0_f32;
            let a = canvas::Path::line(
                iced::Point::new(cursor_pos.x - len, cursor_pos.y - len),
                iced::Point::new(cursor_pos.x + len, cursor_pos.y + len),
            );
            let b = canvas::Path::line(
                iced::Point::new(cursor_pos.x - len, cursor_pos.y + len),
                iced::Point::new(cursor_pos.x + len, cursor_pos.y - len),
            );
            // Plain gray X — no outline. Neutral so it reads
            // on both the dark canvas background and the
            // yellow paper fill without competing with the
            // theme's accent colours.
            let stroke = canvas::Stroke::default()
                .with_color(Color::from_rgba(0.55, 0.55, 0.58, 0.9))
                .with_width(1.5);
            frame.stroke(&a, stroke);
            frame.stroke(&b, stroke);
        }
    }

    /// Net-color "pencil" affordance drawn while a net color is armed.
    pub(in crate::canvas) fn draw_net_color_pen(
        &self,
        frame: &mut canvas::Frame,
        cursor_pos: iced::Point,
    ) {
        // Net-color pen affordance — a diagonal "pencil" mark
        // anchored to the cursor, filled with the armed color.
        // iced's mouse cursor set only exposes Crosshair, so we
        // paint our own pencil glyph on the canvas to make the
        // mode visually obvious.
        if let Some(c) = self.pending_net_color {
            let body = if c.a == 0 {
                // Clear-mode sentinel — render a grey pencil so
                // the user still sees the armed state.
                Color::from_rgb(0.75, 0.75, 0.75)
            } else {
                Color::from_rgb8(c.r, c.g, c.b)
            };
            let tip = iced::Point::new(cursor_pos.x + 4.0, cursor_pos.y + 4.0);
            let butt = iced::Point::new(cursor_pos.x + 22.0, cursor_pos.y + 22.0);
            // Shaft (fat colored line)
            let shaft = canvas::Path::line(tip, butt);
            frame.stroke(
                &shaft,
                canvas::Stroke::default().with_color(body).with_width(6.0),
            );
            // Dark outline for contrast on light backgrounds
            frame.stroke(
                &shaft,
                canvas::Stroke::default()
                    .with_color(Color::from_rgba(0.0, 0.0, 0.0, 0.45))
                    .with_width(1.0),
            );
            // Small triangle at tip to look like a pencil nib
            let nib = canvas::Path::new(|b| {
                b.move_to(tip);
                b.line_to(iced::Point::new(tip.x + 3.0, tip.y - 2.0));
                b.line_to(iced::Point::new(tip.x - 2.0, tip.y + 3.0));
                b.close();
            });
            frame.fill(&nib, Color::from_rgb(0.15, 0.15, 0.15));
        }
    }

    /// Tool-name chip beside the snapped placement point for line tools.
    pub(in crate::canvas) fn draw_tool_chip(
        &self,
        frame: &mut canvas::Frame,
        cursor_pos: iced::Point,
    ) {
        // Tool-specific cursor marker: a bright X that locks onto
        // the grid dot the next click will commit to (when snap is
        // enabled), so the user can see exactly where the wire/bus
        // endpoint will land. Altium's placement tag follows the
        // snapped point, not the raw cursor.
        //
        // Only show the X for "line-drawing" tools (wire/bus/etc.)
        // that DON'T already have a ghost preview of what's being
        // placed — the ghost shows the click target for those,
        // and doubling up with an X clutters the cursor.
        let has_ghost = self.ghost_label.is_some()
            || self.ghost_symbol.is_some()
            || self.ghost_text.is_some()
            || self.prefs.placement.is_some();
        if let Some(ref label) = self.tool_preview
            && !has_ghost
        {
            // `cursor_pos` is already the landing point (grid dot or hotspot).
            // No cyan X here — the unified gray placement X
            // painted earlier already sits at the cursor for
            // every tool. Keep only the tool-name chip.
            // Tool-name tag beside the marker. Dark text on a
            // semi-opaque light chip so it reads on any canvas bg.
            let tag = super::hud_tag::layout(cursor_pos, label);
            let chip = canvas::Path::rectangle(tag.chip.position(), tag.chip.size());
            frame.fill(&chip, Color::from_rgba(0.0, 0.0, 0.0, 0.65));
            frame.fill_text(canvas::Text {
                content: label.clone(),
                position: tag.text_origin,
                color: Color::from_rgba(1.0, 1.0, 1.0, 0.95),
                size: iced::Pixels(super::hud_tag::FONT_SIZE),
                font: crate::render_config::IOSEVKA,
                ..canvas::Text::default()
            });
        }
    }
}
