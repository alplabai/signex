use super::tests::empty_snapshot;
use super::*;
use signex_types::theme::{ThemeId, canvas_colors};

fn sheet_with_wire() -> SchematicRenderSnapshot {
    let mut sheet = empty_snapshot();
    sheet.wires.push(signex_types::schematic::Wire {
        uuid: uuid::Uuid::new_v4(),
        start: Point::new(10.0, 10.0),
        end: Point::new(60.0, 10.0),
        stroke_width: 0.15,
        ..Default::default()
    });
    sheet
}

fn bounds() -> Rectangle {
    Rectangle::new(iced::Point::ORIGIN, iced::Size::new(800.0, 600.0))
}

fn at(cache: &mut ContentSceneCache, sheet: &SchematicRenderSnapshot, x: f32, scale: f32) {
    let colors = canvas_colors(ThemeId::Signex);
    let transform = ScreenTransform {
        offset_x: x,
        offset_y: 0.0,
        scale,
    };
    cache.scene_for(sheet, &transform, &colors, bounds(), None, None);
}

/// Pan frames move only the camera offset; the snapshot -> scene build must
/// run once, not once per frame (#171 item 6).
#[test]
fn camera_only_pan_does_not_rebuild_the_scene() {
    let sheet = sheet_with_wire();
    let mut cache = ContentSceneCache::default();
    at(&mut cache, &sheet, 0.0, 4.0);
    for step in 1..=20 {
        at(&mut cache, &sheet, step as f32 * 5.0, 4.0);
    }
    assert_eq!(cache.builds(), 1);
}

#[test]
fn panning_past_the_built_margin_rebuilds_once() {
    let sheet = sheet_with_wire();
    let mut cache = ContentSceneCache::default();
    at(&mut cache, &sheet, 0.0, 4.0);
    at(&mut cache, &sheet, -5000.0, 4.0);
    at(&mut cache, &sheet, -5010.0, 4.0);
    assert_eq!(cache.builds(), 2);
}

#[test]
fn a_zoom_step_rebuilds_because_marker_sizes_are_pixel_defined() {
    let sheet = sheet_with_wire();
    let mut cache = ContentSceneCache::default();
    at(&mut cache, &sheet, 0.0, 4.0);
    at(&mut cache, &sheet, 0.0, 5.0);
    at(&mut cache, &sheet, 3.0, 5.0);
    assert_eq!(cache.builds(), 2);
}

#[test]
fn invalidate_forces_a_rebuild_and_a_theme_change_does_too() {
    let sheet = sheet_with_wire();
    let mut cache = ContentSceneCache::default();
    at(&mut cache, &sheet, 0.0, 4.0);
    cache.invalidate();
    at(&mut cache, &sheet, 0.0, 4.0);
    assert_eq!(cache.builds(), 2);

    let transform = ScreenTransform {
        offset_x: 0.0,
        offset_y: 0.0,
        scale: 4.0,
    };
    let other = canvas_colors(ThemeId::Nord);
    cache.scene_for(&sheet, &transform, &other, bounds(), None, None);
    assert_eq!(cache.builds(), 3);
}

#[test]
fn a_panned_scene_still_contains_the_wire() {
    let sheet = sheet_with_wire();
    let mut cache = ContentSceneCache::default();
    let colors = canvas_colors(ThemeId::Signex);
    for x in [0.0_f32, 40.0] {
        let transform = ScreenTransform {
            offset_x: x,
            offset_y: 0.0,
            scale: 4.0,
        };
        let scene = cache.scene_for(&sheet, &transform, &colors, bounds(), None, None);
        assert!(scene.is_some_and(|s| !s.lines.is_empty()));
    }
    assert_eq!(cache.builds(), 1);
}

fn sheet_with_wire_between(start: (f64, f64), end: (f64, f64)) -> SchematicRenderSnapshot {
    let mut sheet = empty_snapshot();
    sheet.wires.push(signex_types::schematic::Wire {
        uuid: uuid::Uuid::new_v4(),
        start: Point::new(start.0, start.1),
        end: Point::new(end.0, end.1),
        stroke_width: 0.15,
        ..Default::default()
    });
    sheet
}

/// Build at offset (0, 0), then replay at `(dx, dy)`; returns whether the
/// replayed scene holds the wire and how many builds happened.
fn replay_after_pan(sheet: &SchematicRenderSnapshot, dx: f32, dy: f32) -> (bool, u32) {
    let mut cache = ContentSceneCache::default();
    let colors = canvas_colors(ThemeId::Signex);
    let mut has_line = false;
    for (x, y) in [(0.0, 0.0), (dx, dy)] {
        let transform = ScreenTransform {
            offset_x: x,
            offset_y: y,
            scale: 4.0,
        };
        let scene = cache.scene_for(sheet, &transform, &colors, bounds(), None, None);
        has_line = scene.is_some_and(|s| !s.lines.is_empty());
    }
    (has_line, cache.builds())
}

/// A wire left of the start viewport (screen -200..-40) is inside the built
/// margin; a pan that moves content right must reveal it without a rebuild.
#[test]
fn wire_left_of_the_start_viewport_survives_a_rightward_pan() {
    let sheet = sheet_with_wire_between((-50.0, 10.0), (-10.0, 10.0));
    assert_eq!(replay_after_pan(&sheet, 100.0, 0.0), (true, 1));
}

#[test]
fn wire_above_the_start_viewport_survives_a_downward_pan() {
    let sheet = sheet_with_wire_between((10.0, -50.0), (10.0, -10.0));
    assert_eq!(replay_after_pan(&sheet, 0.0, 100.0), (true, 1));
}

#[test]
fn wire_right_of_and_below_the_start_viewport_survives_negative_pans() {
    let right = sheet_with_wire_between((250.0, 10.0), (290.0, 10.0));
    assert_eq!(replay_after_pan(&right, -300.0, 0.0), (true, 1));
    let below = sheet_with_wire_between((10.0, 200.0), (10.0, 240.0));
    assert_eq!(replay_after_pan(&below, 0.0, -300.0), (true, 1));
}

/// Beyond one viewport of pan the margin no longer covers the window.
#[test]
fn a_pan_past_the_margin_in_either_axis_rebuilds() {
    let sheet = sheet_with_wire_between((-50.0, 10.0), (-10.0, 10.0));
    assert_eq!(replay_after_pan(&sheet, 900.0, 0.0).1, 2);
    assert_eq!(replay_after_pan(&sheet, 0.0, 700.0).1, 2);
    assert_eq!(replay_after_pan(&sheet, -900.0, 0.0).1, 2);
}

/// The widget rect is window-space (x = 250 with a dock on the left) while
/// the frame is local; the drag path culls with the frame-local rect, so a
/// wire at local x 40..160 must survive (#171 review).
#[test]
fn drag_path_bounds_at_a_non_zero_origin_keep_a_left_edge_wire() {
    let sheet = sheet_with_wire_between((10.0, 10.0), (40.0, 10.0));
    let colors = canvas_colors(ThemeId::Signex);
    let transform = ScreenTransform {
        offset_x: 0.0,
        offset_y: 0.0,
        scale: 4.0,
    };
    let widget = Rectangle::new(iced::Point::new(250.0, 90.0), iced::Size::new(800.0, 600.0));
    let raw = build_schematic_scene(&sheet, &transform, &colors, widget, None, None);
    assert!(raw.is_none(), "raw window-space bounds cull the wire");
    let local = build_schematic_scene(
        &sheet,
        &transform,
        &colors,
        frame_local_bounds(widget),
        None,
        None,
    );
    assert!(local.is_some_and(|s| !s.lines.is_empty()));
}

#[test]
fn setting_the_render_cache_drops_the_cached_scene() {
    let sheet = sheet_with_wire();
    let mut slot = crate::canvas::CanvasSlot::default();
    let colors = canvas_colors(ThemeId::Signex);
    let transform = ScreenTransform {
        offset_x: 0.0,
        offset_y: 0.0,
        scale: 4.0,
    };
    let _ =
        slot.content_scene_for_test()
            .scene_for(&sheet, &transform, &colors, bounds(), None, None);
    assert_eq!(slot.content_scene_for_test().builds(), 1);
    slot.set_render_cache(None);
    let _ =
        slot.content_scene_for_test()
            .scene_for(&sheet, &transform, &colors, bounds(), None, None);
    assert_eq!(slot.content_scene_for_test().builds(), 2);
}
