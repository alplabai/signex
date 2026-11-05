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

/// What a frame is drawn with, besides the slot.
struct Look {
    size: iced::Size,
    dpr: f32,
    theme: signex_types::theme::ThemeId,
    auto_focus: bool,
    wire_overrides: std::collections::HashMap<uuid::Uuid, signex_types::theme::Color>,
    allow_shift: bool,
}

/// One draw of `slot` as the widget does.
fn draw_with(
    slot: &CanvasSlot,
    renderer: &iced::Renderer,
    state: &CanvasState,
    look: &Look,
) -> SurfaceLayers {
    let filters = Default::default();
    let mut prefs = test_prefs(&look.wire_overrides, &filters);
    prefs.device_px_per_logical_px = look.dpr;
    prefs.canvas_colors = signex_types::theme::canvas_colors(look.theme);
    prefs.auto_focus = look.auto_focus;
    SchematicCanvas::new(slot, prefs).draw_surface(
        state,
        renderer,
        Rectangle::with_size(look.size),
        mouse::Cursor::Unavailable,
        look.allow_shift,
    )
}

struct Sheet {
    slot: CanvasSlot,
    renderer: iced::Renderer,
    look: Look,
    state: CanvasState,
    /// A wire of the sheet.
    wire: uuid::Uuid,
}

impl Sheet {
    fn new(dpr: f32) -> Self {
        let sheet = fixture();
        Self {
            wire: sheet.wires[0].uuid,
            slot: slot_with(&sheet),
            renderer: crate::layout_probe::headless_renderer(),
            look: Look {
                size: iced::Size::new(WIDTH as f32, HEIGHT as f32),
                dpr,
                theme: signex_types::theme::ThemeId::Signex,
                auto_focus: false,
                wire_overrides: Default::default(),
                allow_shift: true,
            },
            state: CanvasState::default(),
        }
    }

    fn draw(&self) -> SurfaceLayers {
        draw_with(&self.slot, &self.renderer, &self.state, &self.look)
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
    sheet.look.theme = signex_types::theme::ThemeId::Nord;
    sheet.slot.clear_content_cache();
    assert_eq!(sheet.frame(), (1, None));

    // And on a pan frame, without the clear.
    sheet.look.theme = signex_types::theme::ThemeId::Signex;
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

#[test]
fn a_viewport_resize_between_build_and_pan_rebuilds_once() {
    let mut sheet = Sheet::new(1.0);
    sheet.frame();
    sheet.look.size = iced::Size::new(WIDTH as f32 - 40.0, HEIGHT as f32);
    sheet.pan(3.0, 0.0);
    assert_eq!(sheet.frame(), (1, None));
    assert_eq!(sheet.frame(), (0, None));
}

#[test]
fn a_display_scale_change_rebuilds_once() {
    let mut sheet = Sheet::new(1.0);
    sheet.frame();
    sheet.look.dpr = 2.0;
    assert_eq!(sheet.frame(), (1, None));
    // Moving between displays while panning is the same input change.
    sheet.look.dpr = 1.0;
    sheet.pan(4.0, 0.0);
    assert_eq!(sheet.frame(), (1, None));
}

#[test]
fn a_caller_that_does_not_allow_a_shift_never_gets_one() {
    let mut sheet = Sheet::new(1.0);
    sheet.frame();
    sheet.pan(6.0, 0.0);
    sheet.look.allow_shift = false;
    assert_eq!(sheet.frame(), (1, None), "a pan rebuilds instead");
    assert_eq!(sheet.frame(), (0, None));
}

#[test]
fn a_move_drag_does_not_touch_the_retained_layer() {
    let mut sheet = Sheet::new(1.0);
    sheet.frame();
    let built = sheet.slot.content_surface.borrow().rebuilds();
    sheet.state.move_dragging = true;
    sheet.state.move_origin = Some((0.0, 0.0));
    sheet.state.move_current = Some((3.0, 0.0));

    // Nothing selected: the content draws live, shifted by the drag.
    sheet.pan(6.0, 0.0);
    assert_eq!(sheet.draw().content_shift, None);

    // Something selected: the drag layers take over.
    sheet.slot.selected = vec![signex_types::schematic::SelectedItem::new(
        sheet.wire,
        signex_types::schematic::SelectedKind::Wire,
    )];
    assert_eq!(sheet.draw().content_shift, None);
    assert_eq!(sheet.slot.content_surface.borrow().rebuilds(), built);
}

#[test]
fn a_focus_change_on_a_pan_frame_rebuilds_once() {
    let mut sheet = Sheet::new(1.0);
    sheet.slot.selected = vec![signex_types::schematic::SelectedItem::new(
        sheet.wire,
        signex_types::schematic::SelectedKind::Wire,
    )];
    sheet.frame();
    sheet.look.auto_focus = true;
    sheet.pan(3.0, 0.0);
    assert_eq!(sheet.frame(), (1, None));
}

#[test]
fn a_net_highlight_change_rebuilds_once() {
    let mut sheet = Sheet::new(1.0);
    sheet.frame();
    sheet.slot.set_net_highlight(Some(std::sync::Arc::new(
        [sheet.wire].into_iter().collect(),
    )));
    sheet.pan(3.0, 0.0);
    assert_eq!(sheet.frame(), (1, None));
}

#[test]
fn a_wire_style_change_on_a_pan_frame_rebuilds_once() {
    let mut sheet = Sheet::new(1.0);
    sheet.frame();
    sheet
        .look
        .wire_overrides
        .insert(sheet.wire, signex_types::theme::Color::new(255, 0, 0, 255));
    sheet.pan(3.0, 0.0);
    assert_eq!(sheet.frame(), (1, None));
}

#[test]
fn a_pan_far_from_the_origin_still_reuses() {
    // Zoomed far into a big sheet the offsets pass 1e5 px, where one step of
    // an f32 is far above 1e-3.
    let sheet = Sheet::new(1.0);
    sheet.slot.camera_mut().offset = iced::Point::new(-300_000.0, 120_000.0);
    sheet.frame();
    sheet.pan(3.0, -2.0);
    let (rebuilds, shift) = sheet.frame();
    assert_eq!(rebuilds, 0);
    assert_eq!(shift, Some(Vector::new(3.0, -2.0)));
}

#[test]
fn pans_in_device_pixel_steps_reuse_at_every_common_scale() {
    for dpr in [1.0, 1.25, 1.5, 1.75, 2.0, 2.5] {
        let sheet = Sheet::new(dpr);
        sheet.frame();
        for step in 1..=60 {
            // A cursor-driven pan: whole device pixels, not whole logical ones.
            sheet.pan(4.0 / dpr, 3.0 / dpr);
            let (rebuilds, shift) = sheet.frame();
            assert_eq!(rebuilds, 0, "dpr {dpr}, step {step}");
            assert!(shift.is_some(), "dpr {dpr}, step {step}");
        }
    }
}
