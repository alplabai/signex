use super::super::*;

impl SchematicCanvas<'_> {
    /// Ghost power-port / symbol preview following the cursor.
    pub(in crate::canvas) fn draw_ghost_symbol(
        &self,
        frame: &mut canvas::Frame,
        bounds: Rectangle,
        cursor_pos: iced::Point,
    ) {
        let cam = self.camera();
        // Ghost power-port symbol preview at cursor position.
        // While placement is paused (TAB → properties form open),
        // hide the ghosts so the user isn't distracted by a preview
        // that can't be committed until they confirm.
        if let Some(ref ghost_sym) = self.ghost_symbol
            && !self.placement_paused
        {
            let cursor_world = cam.screen_to_world(cursor_pos, bounds);
            let (sx, sy) = if self.prefs.snap_enabled && self.prefs.snap_grid_mm > 0.0 {
                let g = self.prefs.snap_grid_mm;
                (
                    (cursor_world.x as f64 / g).round() * g,
                    (cursor_world.y as f64 / g).round() * g,
                )
            } else {
                (cursor_world.x as f64, cursor_world.y as f64)
            };
            let mut preview = ghost_sym.clone();
            preview.position = signex_types::schematic::Point::new(sx, sy);
            let ghost_transform = crate::schematic_runtime::ScreenTransform {
                offset_x: cam.offset.x,
                offset_y: cam.offset.y,
                scale: cam.scale,
                device_px_per_logical_px: self.prefs.device_px_per_logical_px,
            };
            let ghost = self.power_port_ghost(&preview, &ghost_transform);
            crate::schematic_runtime::draw_power_port_preview(
                &mut *frame,
                &ghost,
                &ghost_transform,
                &self.prefs.canvas_colors,
            );
        }
    }

    /// The power-port ghost at `preview`, stroked with this sheet's project
    /// defaults so it matches the placed port (#2026).
    pub(in crate::canvas) fn power_port_ghost(
        &self,
        preview: &signex_types::schematic::Symbol,
        transform: &crate::schematic_runtime::ScreenTransform,
    ) -> signex_renderer::schematic::SchematicSnapshot {
        crate::schematic_runtime::power_port_preview_snapshot(
            preview,
            transform,
            Color::from_rgba(0.3, 0.8, 1.0, 0.7),
            &self.prefs.schematic_defaults,
        )
    }

    /// The content attached to the cursor (a library part waiting for its
    /// click), drawn at the grid point a click would place it on. Hidden
    /// while the Tab form is open, like the other ghosts.
    pub(in crate::canvas) fn draw_placement_ghost(
        &self,
        frame: &mut canvas::Frame,
        bounds: Rectangle,
        cursor_pos: iced::Point,
    ) {
        let Some(ghost) = super::super::placement_ghost::visible_ghost(
            self.prefs.placement,
            self.placement_paused,
        ) else {
            return;
        };
        let cam = self.camera();
        let world = cam.screen_to_world(cursor_pos, bounds);
        let anchor = super::super::placement_ghost::ghost_anchor(
            (world.x as f64, world.y as f64),
            self.prefs.snap_enabled,
            self.prefs.snap_grid_mm,
        );
        let camera = crate::schematic_runtime::ScreenTransform {
            offset_x: cam.offset.x,
            offset_y: cam.offset.y,
            scale: cam.scale,
            device_px_per_logical_px: self.prefs.device_px_per_logical_px,
        };
        ghost.draw(frame, &camera, anchor, &self.prefs.canvas_colors);
    }

    /// Ghost text-note preview following the cursor.
    pub(in crate::canvas) fn draw_ghost_text(
        &self,
        frame: &mut canvas::Frame,
        bounds: Rectangle,
        cursor_pos: iced::Point,
    ) {
        let cam = self.camera();
        // Ghost text-note preview at cursor position.
        if let Some(ref ghost_tn) = self.ghost_text
            && !self.placement_paused
        {
            let cursor_world = cam.screen_to_world(cursor_pos, bounds);
            let (sx, sy) = if self.prefs.snap_enabled && self.prefs.snap_grid_mm > 0.0 {
                let g = self.prefs.snap_grid_mm;
                (
                    (cursor_world.x as f64 / g).round() * g,
                    (cursor_world.y as f64 / g).round() * g,
                )
            } else {
                (cursor_world.x as f64, cursor_world.y as f64)
            };
            let mut preview = ghost_tn.clone();
            preview.position = signex_types::schematic::Point::new(sx, sy);
            let ghost_transform = crate::schematic_runtime::ScreenTransform {
                offset_x: cam.offset.x,
                offset_y: cam.offset.y,
                scale: cam.scale,
                device_px_per_logical_px: self.prefs.device_px_per_logical_px,
            };
            let ghost_color = Color::from_rgba(0.3, 0.8, 1.0, 0.7);
            crate::schematic_runtime::text::draw_text_note_preview(
                &mut *frame,
                &preview,
                &ghost_transform,
                ghost_color,
                &self.prefs.canvas_colors,
                self.active_snapshot()
                    .and_then(|sheet| sheet.font_family.as_deref()),
            );
        }
    }

    /// The width a placed bus entry draws at: the stroke law at wire weight,
    /// on this window's device-pixel grid.
    pub(crate) fn placed_bus_entry_stroke_px(&self, scale: f32) -> f32 {
        signex_types::schematic::stroke_px(
            self.prefs.schematic_defaults.wire_mm(0.0, None) as f32,
            scale,
            self.prefs.device_px_per_logical_px,
        )
    }

    /// A placed bus entry (or its ghost) between screen points: the placed
    /// width and the whole-pixel snap.
    pub(crate) fn placed_bus_entry_line(
        &self,
        from: iced::Point,
        to: iced::Point,
        scale: f32,
    ) -> (iced::Point, iced::Point, f32) {
        self.snap_screen_line(from, to, self.placed_bus_entry_stroke_px(scale))
    }

    /// Ghost bus entry: the bus end follows the cursor, snapping onto the
    /// nearest bus within one grid step exactly as a click would, and the
    /// other end follows the stored orientation.
    pub(in crate::canvas) fn draw_ghost_bus_entry(
        &self,
        frame: &mut canvas::Frame,
        bounds: Rectangle,
        cursor_pos: iced::Point,
    ) {
        let Some((from, to, width)) = self.ghost_bus_entry_line(bounds, cursor_pos) else {
            return;
        };
        frame.stroke(
            &canvas::Path::line(from, to),
            canvas::Stroke::default()
                .with_color(Color::from_rgba(0.3, 0.8, 1.0, 0.7))
                .with_width(width),
        );
    }

    /// Screen end points and width the ghost bus entry draws at, or `None`
    /// when there is no ghost to show.
    pub(crate) fn ghost_bus_entry_line(
        &self,
        bounds: Rectangle,
        cursor_pos: iced::Point,
    ) -> Option<(iced::Point, iced::Point, f32)> {
        use signex_types::schematic::{Point as SchPoint, snap_to_bus};
        let size = self.ghost_bus_entry?;
        if self.placement_paused {
            return None;
        }
        let cam = self.camera();
        let world = cam.screen_to_world(cursor_pos, bounds);
        let g = self.prefs.snap_grid_mm;
        let (sx, sy) = if self.prefs.snap_enabled && g > 0.0 {
            (
                (world.x as f64 / g).round() * g,
                (world.y as f64 / g).round() * g,
            )
        } else {
            (world.x as f64, world.y as f64)
        };
        let at = SchPoint::new(sx, sy);
        let start = self
            .active_snapshot()
            .and_then(|snapshot| snap_to_bus(&snapshot.buses, at, g))
            .unwrap_or(at);
        let to_screen =
            |p: SchPoint| cam.world_to_screen(iced::Point::new(p.x as f32, p.y as f32), bounds);
        Some(self.placed_bus_entry_line(
            to_screen(start),
            to_screen(SchPoint::new(start.x + size.0, start.y + size.1)),
            cam.scale,
        ))
    }

    /// Ghost label / port preview following the cursor.
    pub(in crate::canvas) fn draw_ghost_label(
        &self,
        frame: &mut canvas::Frame,
        bounds: Rectangle,
        cursor_pos: iced::Point,
    ) {
        let cam = self.camera();
        // Ghost label/port preview at cursor position
        if let Some(ref ghost) = self.ghost_label
            && !self.placement_paused
        {
            let cursor_world = cam.screen_to_world(cursor_pos, bounds);
            let snap_world = if self.prefs.snap_enabled && self.prefs.snap_grid_mm > 0.0 {
                let g = self.prefs.snap_grid_mm;
                (
                    (cursor_world.x as f64 / g).round() * g,
                    (cursor_world.y as f64 / g).round() * g,
                )
            } else {
                (cursor_world.x as f64, cursor_world.y as f64)
            };
            let mut preview_label = ghost.clone();
            preview_label.position =
                signex_types::schematic::Point::new(snap_world.0, snap_world.1);
            let ghost_transform = crate::schematic_runtime::ScreenTransform {
                offset_x: cam.offset.x,
                offset_y: cam.offset.y,
                scale: cam.scale,
                device_px_per_logical_px: self.prefs.device_px_per_logical_px,
            };
            let ghost_color = Color::from_rgba(0.3, 0.8, 1.0, 0.7);
            let ghost_fill = Color::from_rgba(0.3, 0.8, 1.0, 0.15);
            crate::schematic_runtime::label::draw_label_preview(
                &mut *frame,
                &preview_label,
                &ghost_transform,
                ghost_color,
                ghost_fill,
                &self.prefs.canvas_colors,
                self.active_snapshot()
                    .and_then(|sheet| sheet.font_family.as_deref()),
            );
        }
    }
}
