//! The rotated-text layer of a GPU-mounted sheet is retained across pans like
//! the CPU content layer (#2128): a whole-pixel pan moves it by a translation
//! instead of laying out and tessellating every rotated text again.

use iced::Vector;
use iced::widget::Widget as _;

use super::gpu_mount_tests::{HEIGHT, WIDTH, fixture, gpu, render, slot_with};
use super::gpu_text_layer::SchematicStaticTextLayer;
use super::tests::test_prefs;
use super::*;

fn bounds() -> Rectangle {
    Rectangle::with_size(iced::Size::new(WIDTH as f32, HEIGHT as f32))
}

fn pan(slot: &CanvasSlot, dx: f32, dy: f32) {
    let mut camera = slot.camera_mut();
    camera.offset.x += dx;
    camera.offset.y += dy;
}

#[test]
fn a_whole_pixel_pan_moves_the_gpu_text_layer_instead_of_rebuilding_it() {
    let slot = slot_with(&fixture());
    let renderer = crate::layout_probe::headless_renderer();
    let (overrides, filters) = (Default::default(), Default::default());
    let prefs = test_prefs(&overrides, &filters);
    let layer = || {
        SchematicCanvas::new(&slot, prefs)
            .gpu_overlay()
            .static_text_layer(&renderer, bounds())
    };
    let rebuilds = || slot.gpu_text_surface.borrow().rebuilds();

    assert_eq!(layer().shift, Vector::ZERO);
    assert_eq!(rebuilds(), 1, "the first frame builds");
    for step in 1..=10 {
        pan(&slot, 3.0, 2.0);
        let shift = layer().shift;
        assert_eq!(rebuilds(), 1, "pan step {step} rebuilt the text layer");
        assert!((shift.x - 3.0 * step as f32).abs() < 1e-3, "{shift:?}");
        assert!((shift.y - 2.0 * step as f32).abs() < 1e-3, "{shift:?}");
    }
}

fn text_layer(slot: &CanvasSlot) -> iced::Element<'_, crate::app::Message> {
    let overrides = Box::leak(Box::new(std::collections::HashMap::new()));
    let filters = Box::leak(Box::new(std::collections::HashSet::new()));
    SchematicStaticTextLayer::new(slot, test_prefs(overrides, filters)).boxed()
}

/// After whole-pixel pans the shifted, retained text layer reads like one laid
/// out fresh at that camera: the translation lands where the camera went.
#[test]
fn a_panned_gpu_text_layer_matches_a_fresh_one() {
    let mut gpu = gpu();
    let panned = slot_with(&fixture());
    let _ = render(&mut gpu, text_layer(&panned));
    for _ in 0..5 {
        pan(&panned, 3.0, 2.0);
        let _ = render(&mut gpu, text_layer(&panned));
    }
    let panned_frame = render(&mut gpu, text_layer(&panned));
    assert_eq!(
        panned.gpu_text_surface.borrow().rebuilds(),
        1,
        "the pan rebuilt"
    );

    let fresh = slot_with(&fixture());
    let (x, y) = (panned.camera().offset.x, panned.camera().offset.y);
    fresh.camera_mut().offset = iced::Point::new(x, y);
    let fresh_frame = render(&mut gpu, text_layer(&fresh));

    let drawn = fresh_frame.data.chunks(4).filter(|px| px[3] != 0).count();
    assert!(drawn > 20, "the fixture's rotated text is not in the frame");
    let diff = panned_frame
        .data
        .iter()
        .zip(&fresh_frame.data)
        .map(|(a, b)| a.abs_diff(*b))
        .max()
        .unwrap_or(0);
    assert_eq!(
        diff, 0,
        "the panned layer differs from a fresh one at the same camera"
    );
}

#[test]
fn a_document_change_rebuilds_the_gpu_text_layer() {
    let mut slot = slot_with(&fixture());
    let renderer = crate::layout_probe::headless_renderer();
    let (overrides, filters) = (Default::default(), Default::default());
    let prefs = test_prefs(&overrides, &filters);
    SchematicCanvas::new(&slot, prefs)
        .gpu_overlay()
        .static_text_layer(&renderer, bounds());
    slot.clear_content_cache();
    SchematicCanvas::new(&slot, prefs)
        .gpu_overlay()
        .static_text_layer(&renderer, bounds());
    assert_eq!(slot.gpu_text_surface.borrow().rebuilds(), 2);
}

/// The CPU content layer and the GPU text layer draw different things from the
/// same slot; one must never be reused as the other.
#[test]
fn the_cpu_content_layer_and_the_gpu_text_layer_are_retained_apart() {
    let slot = slot_with(&fixture());
    let renderer = crate::layout_probe::headless_renderer();
    let (overrides, filters) = (Default::default(), Default::default());
    let prefs = test_prefs(&overrides, &filters);
    SchematicCanvas::new(&slot, prefs).draw_surface(
        &CanvasState::default(),
        &renderer,
        bounds(),
        mouse::Cursor::Unavailable,
        true,
    );
    SchematicCanvas::new(&slot, prefs)
        .gpu_overlay()
        .static_text_layer(&renderer, bounds());
    assert_eq!(slot.content_surface.borrow().rebuilds(), 1);
    assert_eq!(slot.gpu_text_surface.borrow().rebuilds(), 1);
}
