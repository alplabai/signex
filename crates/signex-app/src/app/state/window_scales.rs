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

    /// Record a window's OS scale. Only a window the app tracks is stored,
    /// so a reading that arrives after the window closed does not bring its
    /// entry back. A non-finite or non-positive reading is dropped.
    pub fn set_window_os_scale(&mut self, window: iced::window::Id, os_scale: f32) {
        let tracked = self.main_window_id == Some(window) || self.windows.contains_key(&window);
        if tracked && os_scale.is_finite() && os_scale > 0.0 {
            self.window_os_scales.insert(window, os_scale);
        }
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
