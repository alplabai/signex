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
