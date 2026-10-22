// Shared private unit-test definitions for the Gerber viewer.
#![allow(unused_imports, unused_macros)]

macro_rules! gerber_canvas_tests
{
    () => {
        #[cfg(test)]
        mod canvas_tests
        {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn bundled_palette_contains_material_colors_for_every_layer_slot()
    {
        let palette = material_layer_palette();

        assert_eq!(palette.len(), 64);
        assert_eq!(palette[0], Color::from_rgb8(211, 47, 47));
    }

    #[test]
    fn default_grid_markers_have_visible_contrast_on_the_dark_canvas()
    {
        let background = Color::from_rgb8(45, 45, 48);
        let marker = grid_render_color(material_grid_color());
        let blend = |foreground: f32, background: f32| {
            foreground * marker.a + background * (1.0 - marker.a)
        };
        let marker_luminance = 0.2126 * blend(marker.r, background.r)
            + 0.7152 * blend(marker.g, background.g)
            + 0.0722 * blend(marker.b, background.b);
        let background_luminance = 0.2126 * background.r
            + 0.7152 * background.g
            + 0.0722 * background.b;

        assert!((marker_luminance - background_luminance).abs() >= 0.12);
    }

    #[test]
    fn display_units_convert_coordinates_and_bounds_from_millimetres()
    {
        let point = signex_gerber::Point { x: 25.4, y: 12.7 };
        let bounds = Bounds {
            min: signex_gerber::Point { x: 0.0, y: -12.7 },
            max: point,
        };

        assert_eq!(
            format_cartesian_coordinate_in_unit(
                point,
                GerberDisplayUnit::Millimetres,
                ".",
            ),
            "X: 25.4000  Y: 12.7000 mm"
        );
        assert_eq!(
            format_cartesian_coordinate_in_unit(
                point,
                GerberDisplayUnit::Inches,
                ".",
            ),
            "X: 1.0000  Y: 0.5000 in"
        );
        assert_eq!(
            format_cartesian_coordinate_in_unit(
                point,
                GerberDisplayUnit::Mils,
                ".",
            ),
            "X: 1000.00  Y: 500.00 mils"
        );
        assert_eq!(
            format_bounds_in_unit(bounds, GerberDisplayUnit::Inches, "."),
            "Bounds: X 0.0000…1.0000  Y -0.5000…0.5000 in"
        );
    }

    #[test]
    fn polar_coordinates_include_degrees_and_radians_in_selected_unit()
    {
        let point = signex_gerber::Point { x: 25.4, y: 25.4 };

        assert_eq!(
            format_polar_coordinate_in_unit(
                point,
                GerberDisplayUnit::Inches,
                ".",
            ),
            "R: 1.4142 in  θ: 45.00° / 0.7854 rad"
        );
        assert_eq!(
            format_polar_coordinate_in_unit(
                signex_gerber::Point::default(),
                GerberDisplayUnit::Millimetres,
                ".",
            ),
            "R: 0.0000 mm  θ: 0.00° / 0.0000 rad"
        );
        assert_eq!(
            format_polar_coordinate_in_unit(
                signex_gerber::Point { x: 0.0, y: -25.4 },
                GerberDisplayUnit::Mils,
                ".",
            ),
            "R: 1000.00 mils  θ: -90.00° / -1.5708 rad"
        );
    }

    #[test]
    fn full_window_crosshair_spans_viewport_and_toggle_requests_redraw()
    {
        let bounds = Rectangle::new(
            Point::ORIGIN,
            iced::Size::new(640.0, 480.0),
        );
        let segments =
            full_window_crosshair_segments(bounds, Point::new(120.0, 75.0));

        assert_eq!(
            segments,
            [
                (Point::new(0.0, 75.0), Point::new(640.0, 75.0)),
                (Point::new(120.0, 0.0), Point::new(120.0, 480.0)),
            ]
        );

        let mut state = GerberViewerState::default();
        let initial_generation = state.redraw_generation;
        state.set_full_window_crosshair(true);

        assert_eq!(state.crosshair_mode, GerberCrosshairMode::Full);
        assert_eq!(state.redraw_generation, initial_generation + 1);

        state.set_full_window_crosshair(true);
        assert_eq!(state.redraw_generation, initial_generation + 1);
    }

    #[test]
    fn crosshair_cycles_none_short_full_and_uses_matching_segments()
    {
        let bounds = Rectangle::new(
            Point::ORIGIN,
            iced::Size::new(640.0, 480.0),
        );
        let position = Point::new(120.0, 75.0);

        assert!(crosshair_segments(
            bounds,
            position,
            GerberCrosshairMode::None,
        )
        .is_empty());
        assert_eq!(
            crosshair_segments(bounds, position, GerberCrosshairMode::Short),
            vec![
                (Point::new(112.0, 75.0), Point::new(128.0, 75.0)),
                (Point::new(120.0, 67.0), Point::new(120.0, 83.0)),
            ],
        );

        let mut state = GerberViewerState::default();
        assert_eq!(state.crosshair_mode, GerberCrosshairMode::Short);
        state.cycle_crosshair_mode();
        assert_eq!(state.crosshair_mode, GerberCrosshairMode::Full);
        state.cycle_crosshair_mode();
        assert_eq!(state.crosshair_mode, GerberCrosshairMode::None);
        state.cycle_crosshair_mode();
        assert_eq!(state.crosshair_mode, GerberCrosshairMode::Short);
    }

    #[test]
    fn display_units_cycle_millimetres_mils_inches()
    {
        let mut state = GerberViewerState::default();

        assert_eq!(state.display_unit, GerberDisplayUnit::Millimetres);
        state.cycle_display_unit();
        assert_eq!(state.display_unit, GerberDisplayUnit::Mils);
        state.cycle_display_unit();
        assert_eq!(state.display_unit, GerberDisplayUnit::Inches);
        state.cycle_display_unit();
        assert_eq!(state.display_unit, GerberDisplayUnit::Millimetres);
    }

    #[test]
    fn changing_display_unit_does_not_modify_source_geometry()
    {
        let layer = signex_gerber::load_gerber_reader(
            "copper.gbr",
            Cursor::new(
                b"%FSLAX46Y46*%\n%MOMM*%\n%ADD10C,1.000*%\nD10*\nX0Y0D03*\nM02*\n",
            ),
        )
        .expect("test Gerber must parse");
        let mut state = GerberViewerState::default();
        state.apply_load_batch(GerberLoadBatch {
            layers: vec![layer.clone()],
            failures: Vec::new(),
        });
        let zoom = state.zoom;
        let pan = state.pan;

        state.set_display_unit(GerberDisplayUnit::Inches);
        state.set_cursor_world_position(Some(signex_gerber::Point {
            x: 25.4,
            y: 12.7,
        }));

        assert_eq!(state.display_unit, GerberDisplayUnit::Inches);
        assert_eq!(state.layers[0].layer, layer);
        assert_eq!(state.zoom, zoom);
        assert_eq!(state.pan, pan);
    }

    #[test]
    fn redraw_changes_only_the_viewport_generation()
    {
        let mut state = GerberViewerState::default();
        let status_before = state.status.clone();
        let layer_count_before = state.layers.len();

        state.redraw_viewport();

        assert_eq!(state.redraw_generation, 1);
        assert_eq!(state.layers.len(), layer_count_before);
        assert_ne!(state.status, status_before);
    }

    #[test]
    fn zoom_updates_scale_and_clamps_to_safe_limits()
    {
        let mut state = GerberViewerState::default();

        state.zoom_by(2.0);
        assert_eq!(state.zoom, 2.0);

        state.zoom_by(100.0);
        assert_eq!(state.zoom, MAX_ZOOM);

        state.zoom_by(0.0001);
        assert_eq!(state.zoom, MIN_ZOOM);

        state.zoom_by(f32::NAN);
        assert_eq!(state.zoom, MIN_ZOOM);
    }

    #[test]
    fn fit_page_resets_navigation_and_computes_valid_landscape_transform()
    {
        let bounds = Bounds {
            min: signex_gerber::Point { x: 10.0, y: 20.0 },
            max: signex_gerber::Point { x: 110.0, y: 70.0 },
        };
        let viewport = Rectangle::new(Point::ORIGIN, iced::Size::new(1000.0, 700.0));
        let (scale, world_center, screen_center) =
            fit_transform(bounds, viewport, 1.0, iced::Vector::default());

        assert!(scale.is_finite() && scale > 0.0);
        assert_eq!(world_center, Point::new(60.0, 45.0));
        assert_eq!(screen_center, Point::new(500.0, 350.0));
        assert!(bounds.width() as f32 * scale <= viewport.width - CANVAS_MARGIN * 2.0 + 0.01);
        assert!(bounds.height() as f32 * scale <= viewport.height - CANVAS_MARGIN * 2.0 + 0.01);

        let mut state = GerberViewerState::default();
        state.zoom = 4.0;
        state.pan = iced::Vector::new(100.0, -50.0);
        state.fit_page();
        assert_eq!(state.zoom, 1.0);
        assert_eq!(state.pan, iced::Vector::default());
    }

    #[test]
    fn page_size_updates_boundary_and_print_layout()
    {
        let artwork = Bounds {
            min: signex_gerber::Point { x: 10.0, y: 20.0 },
            max: signex_gerber::Point { x: 110.0, y: 70.0 },
        };
        assert_eq!(
            page_bounds(Some(artwork), GerberPageSize::FullSize),
            Some(artwork),
        );
        let a4 = page_bounds(Some(artwork), GerberPageSize::A4)
            .expect("fixed page bounds");
        assert!((a4.width() - 297.0).abs() < f64::EPSILON);
        assert!((a4.height() - 210.0).abs() < f64::EPSILON);
        assert!(((a4.min.x + a4.max.x) * 0.5 - 60.0).abs() < f64::EPSILON);
        assert!(((a4.min.y + a4.max.y) * 0.5 - 45.0).abs() < f64::EPSILON);

        let layer = signex_gerber::load_gerber_reader(
            "page.gbr",
            Cursor::new(
                b"%FSLAX46Y46*%\n%MOMM*%\n%ADD10C,1.000*%\nD10*\nX0Y0D03*\nM02*\n",
            ),
        )
        .expect("test Gerber must parse");
        let mut state = GerberViewerState::default();
        state.apply_load_batch(GerberLoadBatch {
            layers: vec![layer],
            failures: Vec::new(),
        });
        let initial_generation = state.redraw_generation;
        state.set_page_size_with(GerberPageSize::B, |_| Ok(()));

        let layout = state.print_layout().expect("print layout");
        assert_eq!(layout.page_size, GerberPageSize::B);
        assert!((layout.bounds.width() - 431.8).abs() < 0.000_001);
        assert!((layout.bounds.height() - 279.4).abs() < 0.000_001);
        assert_eq!(state.redraw_generation, initial_generation + 1);
        assert_eq!(state.status, "Page size: ANSI B.");
    }

    #[test]
    fn failed_page_size_persistence_keeps_existing_selection()
    {
        let mut state = GerberViewerState::default();
        state.page_size = GerberPageSize::A4;

        state.set_page_size_with(GerberPageSize::A3, |_| {
            Err("settings unavailable".to_owned())
        });

        assert_eq!(state.page_size, GerberPageSize::A4);
        assert!(state.status.contains("settings unavailable"));
    }

    #[test]
    fn zoom_area_normalizes_reverse_drag_and_rejects_degenerate_drag()
    {
        let forward = normalized_screen_rectangle(
            Point::new(20.0, 30.0),
            Point::new(120.0, 80.0),
        )
        .expect("forward selection");
        let reverse = normalized_screen_rectangle(
            Point::new(120.0, 80.0),
            Point::new(20.0, 30.0),
        )
        .expect("reverse selection");

        assert_eq!(forward, reverse);
        assert_eq!(forward.x, 20.0);
        assert_eq!(forward.y, 30.0);
        assert_eq!(forward.width, 100.0);
        assert_eq!(forward.height, 50.0);
        assert!(normalized_screen_rectangle(
            Point::new(10.0, 10.0),
            Point::new(12.0, 40.0),
        )
        .is_none());
        assert!(normalized_screen_rectangle(
            Point::new(10.0, 10.0),
            Point::new(40.0, 12.0),
        )
        .is_none());
    }

    #[test]
    fn zoom_selection_centres_and_fits_selected_world_bounds()
    {
        let base = Bounds {
            min: signex_gerber::Point { x: 0.0, y: 0.0 },
            max: signex_gerber::Point { x: 200.0, y: 100.0 },
        };
        let selection = Bounds {
            min: signex_gerber::Point { x: 50.0, y: 25.0 },
            max: signex_gerber::Point { x: 100.0, y: 75.0 },
        };
        let viewport = Rectangle::new(
            Point::ORIGIN,
            iced::Size::new(900.0, 600.0),
        );

        let (zoom, pan) = zoom_transform_for_selection(base, selection, viewport)
            .expect("valid zoom transform");
        let (scale, world_center, screen_center) =
            fit_transform(base, viewport, zoom, pan);
        let selection_center = Point::new(75.0, 50.0);
        let selected_screen_center = Point::new(
            screen_center.x + (selection_center.x - world_center.x) * scale,
            screen_center.y - (selection_center.y - world_center.y) * scale,
        );

        assert!((selected_screen_center.x - viewport.width * 0.5).abs() < 0.01);
        assert!((selected_screen_center.y - viewport.height * 0.5).abs() < 0.01);
        assert!(selection.width() as f32 * scale <= viewport.width);
        assert!(selection.height() as f32 * scale <= viewport.height);
        assert!(zoom > 1.0);
        assert!(zoom_transform_for_selection(
            base,
            Bounds {
                min: selection.min,
                max: selection.min,
            },
            viewport,
        )
        .is_none());
    }

    #[test]
    fn zoom_area_mode_deactivates_after_successful_selection()
    {
        let layer = signex_gerber::load_gerber_reader(
            "zoom.gbr",
            Cursor::new(
                b"%FSLAX46Y46*%\n%MOMM*%\n%ADD10C,1.000*%\nD10*\nX0Y0D03*\nX100000000Y50000000D03*\nM02*\n",
            ),
        )
        .expect("test Gerber must parse");
        let mut state = GerberViewerState::default();
        state.apply_load_batch(GerberLoadBatch {
            layers: vec![layer],
            failures: Vec::new(),
        });
        state.toggle_zoom_selection();
        assert!(state.zoom_selection_active);

        state.zoom_to_selection(
            Bounds {
                min: signex_gerber::Point { x: 20.0, y: 10.0 },
                max: signex_gerber::Point { x: 60.0, y: 30.0 },
            },
            Rectangle::new(Point::ORIGIN, iced::Size::new(800.0, 500.0)),
        );

        assert!(!state.zoom_selection_active);
        assert!(state.zoom > 1.0);
        assert_eq!(state.status, "Zoomed to selected area.");
    }

    #[test]
    fn hidden_layer_remains_loaded_but_is_excluded_from_visible_bounds()
    {
        let layer = signex_gerber::load_gerber_reader(
            "copper.gbr",
            Cursor::new(
                b"%FSLAX46Y46*%\n%MOMM*%\n%ADD10C,1.000*%\nD10*\nX0Y0D03*\nM02*\n",
            ),
        )
        .expect("test Gerber must parse");
        let mut state = GerberViewerState::default();
        state.apply_load_batch(GerberLoadBatch {
            layers: vec![layer],
            failures: Vec::new(),
        });

        state.set_layer_visible(0, false);
        assert_eq!(state.layers.len(), 1);
        assert!(!state.layers[0].visible);
        assert!(visible_bounds(&state.layers).is_none());

        state.set_layer_visible(0, true);
        assert!(visible_bounds(&state.layers).is_some());
    }
        }
    };
}

pub(crate) use gerber_canvas_tests;
