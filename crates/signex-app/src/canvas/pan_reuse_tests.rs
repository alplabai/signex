//! A pan moves the retained content geometry instead of tessellating it
//! again (#2078).
//!
//! The rebuild counter tests need no GPU. The pixel test renders the widget
//! through iced's wgpu renderer (the harness of `gpu_mount_tests`) and
//! compares a panned frame with a fresh one at the same camera.

use iced::Vector;
use iced::widget::Widget as _;

use super::gpu_mount_tests::{Frame, HEIGHT, WIDTH, fixture, gpu, render, slot_with};
use super::surface::SurfaceLayers;
use super::tests::test_prefs;
use super::*;

const START: f32 = 20.0;

/// One draw of `slot` as the widget does, at `dpr`, in `theme`.
fn draw(
    slot: &CanvasSlot,
    renderer: &iced::Renderer,
    dpr: f32,
    theme: signex_types::theme::ThemeId,
) -> SurfaceLayers {
    let overrides = Default::default();
    let filters = Default::default();
    let mut prefs = test_prefs(&overrides, &filters);
    prefs.device_px_per_logical_px = dpr;
    prefs.canvas_colors = signex_types::theme::canvas_colors(theme);
    let bounds = Rectangle::with_size(iced::Size::new(WIDTH as f32, HEIGHT as f32));
    SchematicCanvas::new(slot, prefs).draw_surface(
        &CanvasState::default(),
        renderer,
        bounds,
        mouse::Cursor::Unavailable,
        true,
    )
}

struct Sheet {
    slot: CanvasSlot,
    renderer: iced::Renderer,
    dpr: f32,
    theme: signex_types::theme::ThemeId,
}

impl Sheet {
    fn new(dpr: f32) -> Self {
        Self {
            slot: slot_with(&fixture()),
            renderer: crate::layout_probe::headless_renderer(),
            dpr,
            theme: signex_types::theme::ThemeId::Signex,
        }
    }

    fn draw(&self) -> SurfaceLayers {
        draw(&self.slot, &self.renderer, self.dpr, self.theme)
    }

    /// Draw, and say how many content rebuilds that took and where the
    /// content layer was shifted to.
    fn frame(&self) -> (u32, Option<Vector>) {
        let before = self.slot.content_surface.borrow().rebuilds();
        let shift = self.draw().content_shift.map(|(_, shift)| shift);
        (
            self.slot.content_surface.borrow().rebuilds() - before,
            shift,
        )
    }

    fn pan(&self, dx: f32, dy: f32) {
        let mut camera = self.slot.camera_mut();
        camera.offset.x += dx;
        camera.offset.y += dy;
    }
}

#[test]
fn a_whole_pixel_pan_inside_the_margin_rebuilds_nothing() {
    let sheet = Sheet::new(1.0);
    assert_eq!(sheet.frame(), (1, None), "the first frame builds");
    assert_eq!(sheet.frame(), (0, None), "an idle frame reuses");
    for step in 1..=20 {
        sheet.pan(3.0, 2.0);
        let (rebuilds, shift) = sheet.frame();
        assert_eq!(rebuilds, 0, "pan step {step} rebuilt the content");
        let shift = shift.expect("a moved camera shifts the retained layer");
        assert!((shift.x - 3.0 * step as f32).abs() < 1e-3, "{shift:?}");
        assert!((shift.y - 2.0 * step as f32).abs() < 1e-3, "{shift:?}");
    }
}

#[test]
fn the_shift_is_the_offset_change() {
    let sheet = Sheet::new(2.0);
    sheet.frame();
    sheet.pan(-17.5, 4.0);
    let (rebuilds, shift) = sheet.frame();
    assert_eq!(rebuilds, 0);
    let shift = shift.expect("shifted");
    assert!((shift.x + 17.5).abs() < 1e-3 && (shift.y - 4.0).abs() < 1e-3);
    assert!((sheet.slot.camera().offset.x - START + 17.5).abs() < 1e-3);
}

#[test]
fn a_pan_off_the_device_pixel_grid_rebuilds_once() {
    let sheet = Sheet::new(1.0);
    sheet.frame();
    sheet.pan(0.5, 0.0);
    assert_eq!(sheet.frame(), (1, None));
    // The rebuilt geometry is the new anchor: whole pixels from it reuse again.
    sheet.pan(4.0, 0.0);
    assert_eq!(sheet.frame().0, 0);
}

#[test]
fn leaving_the_margin_rebuilds_once() {
    let sheet = Sheet::new(1.0);
    sheet.frame();
    sheet.pan(WIDTH as f32 + 40.0, 0.0);
    assert_eq!(sheet.frame(), (1, None));
    sheet.pan(-5.0, 0.0);
    assert_eq!(sheet.frame().0, 0);
}

#[test]
fn a_zoom_rebuilds_once() {
    let sheet = Sheet::new(1.0);
    sheet.frame();
    sheet.slot.camera_mut().scale *= 1.1;
    assert_eq!(sheet.frame(), (1, None));
    assert_eq!(sheet.frame(), (0, None));
}

#[test]
fn an_edit_rebuilds_once() {
    let mut sheet = Sheet::new(1.0);
    sheet.frame();
    // What the app does on an edit.
    sheet.slot.set_render_cache(Some(
        crate::schematic_runtime::SchematicRenderCache::from_sheet_at(&fixture(), 2),
    ));
    sheet.slot.clear_content_cache();
    assert_eq!(sheet.frame(), (1, None));

    // A document swapped without the clear is still caught on a pan frame,
    // by the scene it replays.
    sheet.slot.set_render_cache(Some(
        crate::schematic_runtime::SchematicRenderCache::from_sheet_at(&fixture(), 3),
    ));
    sheet.pan(3.0, 0.0);
    assert_eq!(sheet.frame(), (1, None));
}

#[test]
fn a_theme_change_rebuilds_once() {
    let mut sheet = Sheet::new(1.0);
    sheet.frame();
    sheet.theme = signex_types::theme::ThemeId::Nord;
    sheet.slot.clear_content_cache();
    assert_eq!(sheet.frame(), (1, None));

    // And on a pan frame, without the clear.
    sheet.theme = signex_types::theme::ThemeId::Signex;
    sheet.pan(3.0, 0.0);
    assert_eq!(sheet.frame(), (1, None));
}

#[test]
fn the_layers_stay_in_order_and_the_content_is_the_only_one_shifted() {
    let sheet = Sheet::new(1.0);
    let idle = sheet.draw();
    sheet.pan(6.0, 0.0);
    let panned = sheet.draw();
    assert_eq!(idle.layers.len(), panned.layers.len());
    // Background first, then the content.
    assert_eq!(panned.content_shift.map(|(index, _)| index), Some(1));
}

fn max_channel_diff(a: &Frame, b: &Frame) -> u8 {
    a.data
        .iter()
        .zip(&b.data)
        .map(|(x, y)| x.abs_diff(*y))
        .max()
        .unwrap_or(0)
}

fn widget(slot: &CanvasSlot) -> iced::Element<'_, crate::app::Message> {
    let overrides = Box::leak(Box::new(std::collections::HashMap::new()));
    let filters = Box::leak(Box::new(std::collections::HashSet::new()));
    let mut prefs = test_prefs(overrides, filters);
    prefs.grid_visible = false;
    SchematicSurface::new(SchematicCanvas::new(slot, prefs)).boxed()
}

/// After a run of whole-pixel pans the retained, shifted frame reads like a
/// frame tessellated fresh at that camera.
#[test]
fn a_panned_frame_matches_a_fresh_one() {
    let mut gpu = gpu();
    let panned = slot_with(&fixture());
    let _ = render(&mut gpu, widget(&panned));
    for _ in 0..5 {
        panned.camera_mut().offset.x += 3.0;
        panned.camera_mut().offset.y += 2.0;
        let _ = render(&mut gpu, widget(&panned));
    }
    let panned_frame = render(&mut gpu, widget(&panned));
    assert_eq!(
        panned.content_surface.borrow().rebuilds(),
        1,
        "the pan rebuilt the content"
    );

    let fresh = slot_with(&fixture());
    *fresh.camera_mut() = Camera {
        offset: iced::Point::new(START + 15.0, START + 10.0),
        scale: panned.camera().scale,
    };
    let fresh_frame = render(&mut gpu, widget(&fresh));
    assert_eq!(
        max_channel_diff(&panned_frame, &fresh_frame),
        0,
        "the panned frame differs from a fresh render at the same camera"
    );
}
