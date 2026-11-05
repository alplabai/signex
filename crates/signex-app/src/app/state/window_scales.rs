//! Per-window display scale (#1887): the one answer to "device pixels per
//! logical pixel in window W", which the stroke floor and the hairline snap
//! of every canvas in that window work in.

use super::UiState;

impl UiState {
    /// Device pixels per logical pixel in `window`: its OS scale times the
    /// interface scale. A window not measured yet counts as 1.0.
    #[must_use]
    pub fn device_px_per_logical_px(&self, window: iced::window::Id) -> f32 {
        let os_scale = self.window_os_scales.get(&window).copied().unwrap_or(1.0);
        crate::ui_scale::render_scale(os_scale, self.ui_scale)
    }

    /// Device pixels per logical pixel of the main window, 1.0 before it opens.
    #[must_use]
    pub fn main_device_px_per_logical_px(&self) -> f32 {
        self.main_window_id.map_or(
            signex_types::schematic::UNSCALED_DEVICE_PX_PER_LOGICAL_PX,
            |id| self.device_px_per_logical_px(id),
        )
    }

    /// Record a window's OS scale. Only a window the app tracks is stored,
    /// so a reading that arrives after the window closed does not bring its
    /// entry back. A non-finite or non-positive reading is dropped. Returns
    /// whether the stored scale changed.
    pub fn set_window_os_scale(&mut self, window: iced::window::Id, os_scale: f32) -> bool {
        let tracked = self.main_window_id == Some(window) || self.windows.contains_key(&window);
        if !tracked || !os_scale.is_finite() || os_scale <= 0.0 {
            return false;
        }
        self.window_os_scales.insert(window, os_scale) != Some(os_scale)
    }

    /// Forget a closed window's scale.
    pub fn forget_window_os_scale(&mut self, window: iced::window::Id) {
        self.window_os_scales.remove(&window);
    }
}

#[cfg(test)]
mod tests {
    use crate::app::state::WindowKind;
    use crate::app::{Message, Signex, WindowMsg};
    use crate::schematic_runtime::{ScreenTransform, schematic_draw_options};

    fn scale_changed(app: &mut Signex, id: iced::window::Id, scale: f32) {
        let _ = app.update(Message::Window(WindowMsg::WindowScaleChanged(id, scale)));
    }

    fn app_with_main() -> (Signex, iced::window::Id) {
        let (mut app, _t) = Signex::new();
        let main = iced::window::Id::unique();
        app.ui_state.main_window_id = Some(main);
        (app, main)
    }

    fn undocked(app: &mut Signex) -> iced::window::Id {
        let id = iced::window::Id::unique();
        app.ui_state.windows.insert(
            id,
            WindowKind::UndockedTab {
                path: std::path::PathBuf::from("sheet.snxsch"),
                title: String::new(),
            },
        );
        id
    }

    #[test]
    fn a_window_opened_at_2x_reports_two_device_pixels_per_logical_pixel() {
        let (mut app, main) = app_with_main();
        assert_eq!(app.ui_state.device_px_per_logical_px(main), 1.0);
        scale_changed(&mut app, main, 2.0);
        assert_eq!(app.ui_state.device_px_per_logical_px(main), 2.0);
    }

    #[test]
    fn the_interface_scale_multiplies_the_os_scale() {
        let (mut app, main) = app_with_main();
        app.ui_state.ui_scale = 1.25;
        scale_changed(&mut app, main, 2.0);
        assert_eq!(app.ui_state.device_px_per_logical_px(main), 2.5);
    }

    #[test]
    fn a_resize_onto_another_monitor_replaces_the_scale() {
        let (mut app, main) = app_with_main();
        scale_changed(&mut app, main, 2.0);
        scale_changed(&mut app, main, 1.5);
        assert_eq!(app.ui_state.device_px_per_logical_px(main), 1.5);
    }

    #[test]
    fn an_undocked_window_has_its_own_scale() {
        let (mut app, main) = app_with_main();
        let tab = undocked(&mut app);
        scale_changed(&mut app, main, 2.0);
        scale_changed(&mut app, tab, 1.25);
        assert_eq!(app.ui_state.device_px_per_logical_px(main), 2.0);
        assert_eq!(app.ui_state.device_px_per_logical_px(tab), 1.25);
    }

    #[test]
    fn closing_a_window_drops_its_entry_and_a_late_reading_does_not_bring_it_back() {
        let (mut app, main) = app_with_main();
        let tab = undocked(&mut app);
        scale_changed(&mut app, main, 2.0);
        scale_changed(&mut app, tab, 1.25);
        let _ = app.update(Message::Window(WindowMsg::SecondaryWindowClosed(tab)));
        assert!(!app.ui_state.window_os_scales.contains_key(&tab));
        scale_changed(&mut app, tab, 1.25);
        assert!(!app.ui_state.window_os_scales.contains_key(&tab));
        assert_eq!(app.ui_state.window_os_scales.len(), 1);
    }

    fn clears(app: &Signex, id: iced::window::Id) -> (u32, u32) {
        let slot = match app.interaction_state.canvases.get(&id) {
            Some(slot) => slot,
            None => &app.interaction_state.canvas,
        };
        (slot.bg_cache_clears, slot.content_cache_clears)
    }

    /// The CPU canvas keeps its geometry in caches that a move to another
    /// screen does not invalidate by itself (the logical bounds stay), so a
    /// changed scale must clear them, for that window's canvas alone.
    #[test]
    fn a_changed_scale_clears_that_windows_canvas_caches_and_an_unchanged_one_does_not() {
        let (mut app, main) = app_with_main();
        let tab = undocked(&mut app);
        app.interaction_state
            .canvases
            .insert(tab, crate::canvas::CanvasSlot::new());
        scale_changed(&mut app, main, 1.0);
        scale_changed(&mut app, tab, 1.0);
        let (main_before, tab_before) = (clears(&app, main), clears(&app, tab));

        scale_changed(&mut app, tab, 1.0);
        assert_eq!(clears(&app, tab), tab_before, "same value: nothing cleared");

        scale_changed(&mut app, tab, 2.0);
        let tab_after = clears(&app, tab);
        assert!(tab_after.0 > tab_before.0 && tab_after.1 > tab_before.1);
        assert_eq!(clears(&app, main), main_before, "other windows untouched");

        let panel = {
            let id = iced::window::Id::unique();
            app.ui_state.windows.insert(
                id,
                WindowKind::DetachedPanel(crate::panels::PanelKind::Properties),
            );
            id
        };
        scale_changed(&mut app, panel, 2.0);
        assert_eq!(
            clears(&app, main),
            main_before,
            "a detached window has no canvas and leaves main's caches alone"
        );

        scale_changed(&mut app, main, 1.5);
        let main_after = clears(&app, main);
        assert!(main_after.0 > main_before.0 && main_after.1 > main_before.1);
        assert_eq!(clears(&app, tab), tab_after);
    }

    /// The sheet frame and the wire preview read the scale of the window the
    /// canvas is in, through `canvas_view_prefs`.
    #[test]
    fn the_sheet_frame_and_wire_preview_use_the_windows_scale() {
        let (mut app, main) = app_with_main();
        let tab = undocked(&mut app);
        app.interaction_state
            .canvases
            .insert(tab, crate::canvas::CanvasSlot::new());
        scale_changed(&mut app, main, 1.0);
        scale_changed(&mut app, tab, 2.0);
        let zoomed_out = 0.1;
        let probe = |window| {
            let canvas = crate::canvas::SchematicCanvas::new(
                app.interaction_state.canvas_for_window(window),
                app.canvas_view_prefs(window),
            );
            let (from, to) = (iced::Point::new(0.0, 5.3), iced::Point::new(9.0, 5.3));
            let (a, _, frame_width) = canvas.sheet_frame_line(from, to, 0.0, zoomed_out);
            let wire = canvas.placed_wire_stroke_px(zoomed_out);
            let entry = canvas.placed_bus_entry_stroke_px(zoomed_out);
            ((wire, entry), frame_width, a.y)
        };
        let (wire_1x, frame_1x, _) = probe(main);
        assert_eq!((wire_1x, frame_1x), ((1.0, 1.0), 1.0));
        let (wire_2x, frame_2x, y_2x) = probe(tab);
        assert_eq!((wire_2x, frame_2x), ((0.5, 0.5), 0.5));
        assert_eq!((y_2x * 2.0).fract(), 0.5, "snapped on the device grid");
    }

    /// The wire preview, the bus-entry ghost and the sheet frame are drawn
    /// straight to the frame, outside the scene lock. The functions the draw
    /// sites stroke from (`wire_preview_strokes`, `ghost_bus_entry_line`,
    /// `sheet_frame_line`) must give the width the scene replay gives a placed
    /// wire at every zoom, in 1x and 2x windows: whole device pixels for an
    /// axis-aligned segment (400 % unsnapped was 1.8 px against a placed
    /// 2 px), the exact stroke law for the 45 degree bus entry (#1888).
    #[test]
    fn previews_ghost_and_frame_draw_at_the_placed_wire_width() {
        use iced::{Point as P, Rectangle, Size};
        use signex_types::schematic::{Point as SchPoint, SCHEMATIC_DEFAULT_WIRE_STROKE_MM};
        let (mut app, main) = app_with_main();
        let tab = undocked(&mut app);
        scale_changed(&mut app, main, 1.0);
        scale_changed(&mut app, tab, 2.0);
        let bounds = Rectangle::new(P::ORIGIN, Size::new(2000.0, 2000.0));
        let cursor = P::new(900.0, 700.0);
        for window in [main, tab] {
            let dpr = app.ui_state.device_px_per_logical_px(window);
            for zoom in [0.25, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0] {
                let scale = signex_types::schematic::SCHEMATIC_ZOOM_100_SCALE * zoom;
                let transform = ScreenTransform {
                    offset_x: 0.0,
                    offset_y: 0.0,
                    scale,
                    device_px_per_logical_px: dpr,
                };
                let law = schematic_draw_options(&transform)
                    .stroke_px(SCHEMATIC_DEFAULT_WIRE_STROKE_MM as f32);
                // What the replay gives a placed wire of this direction.
                let placed = |a: P, b: P| crate::scene_line_snap::snap_hairline(a, b, law, dpr).2;
                let mut slot = crate::canvas::CanvasSlot::new();
                slot.set_camera_scale_for_test(scale);
                slot.drawing_mode = true;
                slot.wire_preview = vec![
                    SchPoint::new(5.0, 5.0),
                    SchPoint::new(40.0, 5.0),
                    SchPoint::new(40.0, 30.0),
                ];
                slot.ghost_bus_entry = Some((2.54, 2.54));
                let canvas =
                    crate::canvas::SchematicCanvas::new(&slot, app.canvas_view_prefs(window));
                let context = format!("{zoom}x zoom, {dpr}x window");

                let strokes = canvas
                    .wire_preview_strokes(bounds, cursor, None)
                    .expect("a wire is in progress");
                assert!(strokes.placed.len() == 2 && !strokes.rubber.is_empty());
                for (a, b, width) in strokes.placed.iter().chain(&strokes.rubber) {
                    assert_eq!(*width, placed(*a, *b), "wire preview, {context}");
                }
                let (a, b, width) = strokes.placed[0];
                assert_eq!(a.y, b.y, "a horizontal run stays horizontal");
                if law * dpr < 4.0 {
                    assert_eq!((width * dpr).fract(), 0.0, "whole device px, {context}");
                }

                let (a, b, width) = canvas
                    .ghost_bus_entry_line(bounds, cursor)
                    .expect("a ghost is showing");
                assert_ne!((a.x, a.y), (b.x, b.y));
                assert_eq!(width, placed(a, b), "bus-entry ghost, {context}");
                assert_eq!(
                    width, law,
                    "a 45 degree entry keeps the exact law, {context}"
                );

                let (a, b) = (P::new(3.3, 5.3), P::new(90.1, 5.3));
                let frame = canvas.sheet_frame_line(a, b, SCHEMATIC_DEFAULT_WIRE_STROKE_MM, scale);
                assert_eq!(frame.2, placed(a, b), "sheet frame, {context}");
            }
        }
    }

    #[test]
    fn a_bad_reading_is_ignored() {
        let (mut app, main) = app_with_main();
        scale_changed(&mut app, main, 2.0);
        for bad in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            scale_changed(&mut app, main, bad);
        }
        assert_eq!(app.ui_state.device_px_per_logical_px(main), 2.0);
    }

    /// A canvas in a 2x window floors strokes at one device pixel (0.5
    /// logical) and snaps on the device-pixel grid; in a 1x window the
    /// floor is a whole logical pixel (#1887).
    #[test]
    fn a_canvas_in_a_2x_window_floors_at_one_device_pixel() {
        let (mut app, main) = app_with_main();
        let tab = undocked(&mut app);
        scale_changed(&mut app, main, 1.0);
        scale_changed(&mut app, tab, 2.0);
        let options_in = |app: &Signex, window| {
            let transform = ScreenTransform {
                offset_x: 0.0,
                offset_y: 0.0,
                scale: 1.0,
                device_px_per_logical_px: app.canvas_view_prefs(window).device_px_per_logical_px,
            };
            schematic_draw_options(&transform)
        };
        assert_eq!(options_in(&app, main).stroke_px(0.0), 1.0);
        let options = options_in(&app, tab);
        assert_eq!(options.stroke_px(0.0), 0.5);
        let (a, _, width) = crate::scene_line_snap::snap_hairline(
            iced::Point::new(0.0, 5.3),
            iced::Point::new(9.0, 5.3),
            options.stroke_px(0.0),
            options.device_px_per_logical_px,
        );
        assert_eq!((a.y * 2.0).fract(), 0.5, "centre sits on a device pixel");
        assert_eq!(width, 0.5);
    }
}
