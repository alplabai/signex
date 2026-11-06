//! A pan moves the retained background (paper, grid, sheet frame) instead of
//! tessellating it again (#2102).
//!
//! The counter tests need no GPU. The pixel tests render the widget through
//! iced's wgpu renderer (the harness of `gpu_mount_tests`): a panned frame
//! must equal a fresh one at the same camera, and the plain canvas's own
//! background must agree with it away from the canvas edge.

use iced::Vector;
use iced::widget::Widget as _;

use super::gpu_mount_tests::{Frame, HEIGHT, WIDTH, fixture, gpu, render_at, slot_with};
use super::tests::test_prefs;
use super::*;

const START: f32 = 20.0;

fn slot_with_frame() -> CanvasSlot {
    let mut slot = slot_with(&fixture());
    slot.sheet_frame = signex_output::template::lower_sheet_frame(
        f64::from(slot.paper_width_mm),
        f64::from(slot.paper_height_mm),
        &signex_output::template::Frame::default(),
        Some(&signex_output::template::standard_title_block()),
        |text| text.to_string(),
    );
    slot
}

fn pan(slot: &CanvasSlot, dx: f32, dy: f32) {
    let mut camera = slot.camera_mut();
    camera.offset.x += dx;
    camera.offset.y += dy;
}

/// One surface draw at `dpr`; the background's tessellation count and shift.
fn draw(
    slot: &CanvasSlot,
    renderer: &iced::Renderer,
    dpr: f32,
    bg: Option<iced::Color>,
) -> (u32, Option<Vector>) {
    let (overrides, filters) = Default::default();
    let mut prefs = test_prefs(&overrides, &filters);
    prefs.device_px_per_logical_px = dpr;
    if let Some(bg) = bg {
        prefs.theme_bg = bg;
    }
    let before = slot.bg_tessellations.get();
    let layers = SchematicCanvas::new(slot, prefs).draw_surface(
        &CanvasState::default(),
        renderer,
        Rectangle::with_size(iced::Size::new(WIDTH as f32, HEIGHT as f32)),
        mouse::Cursor::Unavailable,
        true,
    );
    (
        slot.bg_tessellations.get() - before,
        layers.background_shift,
    )
}

#[test]
fn a_whole_pixel_pan_inside_the_margin_tessellates_no_background() {
    let slot = slot_with_frame();
    let renderer = crate::layout_probe::headless_renderer();
    assert_eq!(
        draw(&slot, &renderer, 1.0, None),
        (1, None),
        "first frame builds"
    );
    assert_eq!(
        draw(&slot, &renderer, 1.0, None),
        (0, None),
        "idle frame reuses"
    );
    for step in 1..=20 {
        pan(&slot, 3.0, 2.0);
        let (built, shift) = draw(&slot, &renderer, 1.0, None);
        assert_eq!(built, 0, "pan step {step} tessellated the background");
        let shift = shift.expect("a moved camera shifts the retained background");
        assert!((shift.x - 3.0 * step as f32).abs() < 1e-3, "{shift:?}");
        assert!((shift.y - 2.0 * step as f32).abs() < 1e-3, "{shift:?}");
    }
}

#[test]
fn what_the_background_is_drawn_from_rebuilds_it() {
    let slot = slot_with_frame();
    let renderer = crate::layout_probe::headless_renderer();
    draw(&slot, &renderer, 1.0, None);

    pan(&slot, 0.5, 0.0);
    assert_eq!(
        draw(&slot, &renderer, 1.0, None),
        (1, None),
        "a sub-pixel pan"
    );
    pan(&slot, 4.0, 0.0);
    assert_eq!(
        draw(&slot, &renderer, 1.0, None).0,
        0,
        "whole pixels from the anchor"
    );

    pan(&slot, 3.0 * WIDTH as f32, 0.0);
    assert_eq!(
        draw(&slot, &renderer, 1.0, None),
        (1, None),
        "past the margin"
    );

    slot.camera_mut().scale *= 1.1;
    assert_eq!(draw(&slot, &renderer, 1.0, None).0, 1, "a zoom");

    assert_eq!(
        draw(&slot, &renderer, 2.0, None).0,
        1,
        "another display scale"
    );

    let other = Some(iced::Color::from_rgb(0.2, 0.3, 0.4));
    assert_eq!(
        draw(&slot, &renderer, 2.0, other).0,
        1,
        "a new theme colour"
    );
    assert_eq!(draw(&slot, &renderer, 2.0, other).0, 0, "and then it holds");

    let mut slot = slot;
    slot.sheet_frame.pop();
    assert_eq!(draw(&slot, &renderer, 2.0, other).0, 1, "a new sheet frame");
}

/// Each frame of `offsets` drawn on one slot, the last one returned; the
/// pans between them are whole pixels, so all but the first reuse.
fn last_frame(gpu: &mut super::gpu_mount_tests::Gpu, dpr: f32, offsets: &[(f32, f32)]) -> Frame {
    frames(gpu, dpr, offsets, false)
}

/// The same through the plain canvas mount (trunk's background, content as
/// trunk drew it) when `plain`.
fn frames(
    gpu: &mut super::gpu_mount_tests::Gpu,
    dpr: f32,
    offsets: &[(f32, f32)],
    plain: bool,
) -> Frame {
    let mut slot = slot_with_frame();
    slot.trunk_content.set(plain);
    let (overrides, filters) = Default::default();
    let mut prefs = test_prefs(&overrides, &filters);
    prefs.device_px_per_logical_px = dpr;
    let mut last = None;
    for &(x, y) in offsets {
        *slot.camera_mut() = Camera {
            offset: iced::Point::new(x, y),
            scale: 8.0,
        };
        // What a ViewMoved does to the old cache.
        slot.clear_bg_cache();
        slot.clear_overlay_cache();
        let program = SchematicCanvas::new(&slot, prefs);
        let element = if plain {
            iced::widget::canvas(program)
                .width(iced::Length::Fill)
                .height(iced::Length::Fill)
                .boxed()
        } else {
            SchematicSurface::new(program).boxed()
        };
        last = Some(render_at(gpu, element, dpr));
    }
    last.expect("a frame")
}

#[test]
fn a_panned_background_is_the_fresh_one_pixel_for_pixel() {
    let mut gpu = gpu();
    for dpr in [1.0, 1.5] {
        let end = (START + 38.0, START - 24.0);
        let panned = last_frame(
            &mut gpu,
            dpr,
            &[
                (START, START),
                (START + 10.0, START - 5.0),
                (START + 25.0, START - 14.0),
                end,
            ],
        );
        let fresh = last_frame(&mut gpu, dpr, &[end]);
        let differing = panned
            .data
            .iter()
            .zip(&fresh.data)
            .filter(|(a, b)| a != b)
            .count();
        assert_eq!(differing, 0, "dpr {dpr}: {differing} bytes differ");
    }
}

/// Away from the canvas edge the retained background is the plain canvas's.
/// At the edge the plain canvas drops a grid dot that would straddle it; the
/// retained one, built over a larger area, shows the part inside.
#[test]
fn the_retained_background_matches_the_plain_canvas_inside_the_edge_band() {
    let mut gpu = gpu();
    const BAND: u32 = 3;
    for dpr in [1.0, 1.5] {
        let end = (START + 38.0, START - 24.0);
        let retained = frames(&mut gpu, dpr, &[end], false);
        let plain = frames(&mut gpu, dpr, &[end], true);
        let band = (BAND as f32 * dpr).ceil() as u32;
        let mut differing = 0;
        for y in band..retained.height - band {
            for x in band..retained.width - band {
                let (a, b) = (retained.px(x, y), plain.px(x, y));
                if (0..3).any(|c| a[c].abs_diff(b[c]) > 24) {
                    differing += 1;
                }
            }
        }
        assert_eq!(differing, 0, "dpr {dpr}: {differing} pixels differ");
    }
}
