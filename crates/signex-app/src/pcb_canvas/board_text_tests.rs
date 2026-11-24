use std::collections::BTreeSet;

use iced::{Point, Rectangle, Size};
use signex_renderer::pcb::{FootprintTextInput, PcbSnapshot};
use signex_types::pcb_layer::{PcbLayer, TechnicalLayer};
use uuid::Uuid;

use super::*;

const VIEW: Rectangle = Rectangle {
    x: 0.0,
    y: 0.0,
    width: 800.0,
    height: 600.0,
};

fn text(content: &str, at: [f32; 2]) -> FootprintTextInput {
    FootprintTextInput {
        content: content.into(),
        position: at,
        rotation_deg: 0.0,
        size_mm: 1.0,
        mirrored: false,
        layer: PcbLayer::Technical(TechnicalLayer::SilkscreenTop),
        justify: HAlign::Center,
        designator: false,
    }
}

/// #2821: Show only unrouted fades the board's texts with the rest of it.
#[test]
fn a_dimmed_board_draws_its_texts_faint() {
    let layer = PcbLayer::Technical(TechnicalLayer::SilkscreenTop);
    let plain = BoardPaint::default();
    let dimmed = BoardPaint::default().with_board_dimmed(true);
    assert_eq!(
        text_colour(&plain, layer),
        painted_layer_colour(&plain, layer)
    );
    let faint = text_colour(&dimmed, layer);
    let full = painted_layer_colour(&plain, layer);
    assert!((faint.a - full.a * DIMMED_ALPHA).abs() < 1e-6);
    assert_eq!((faint.r, faint.g, faint.b), (full.r, full.g, full.b));
}

#[test]
fn the_line_is_anchored_at_its_justified_end() {
    use iced::advanced::text::Alignment;
    assert_eq!(align_of(HAlign::Left), Alignment::Left);
    assert_eq!(align_of(HAlign::Center), Alignment::Center);
    assert_eq!(align_of(HAlign::Right), Alignment::Right);
}

#[test]
fn a_text_is_placed_at_its_anchor_at_true_size() {
    let camera = BoardCamera::default();
    let r1 = text("R1", [10.0, 21.0]);
    let placed = place(&r1, &camera, VIEW).expect("in view");
    assert_eq!(
        placed.anchor,
        camera.world_to_screen(Point::new(10.0, 21.0))
    );
    let em_mm = 1.0 / signex_types::font::mm_per_em(None);
    assert!((placed.size_px - em_mm * camera.scale()).abs() < 1e-4);
}

#[test]
fn a_text_out_of_view_is_left_out() {
    let camera = BoardCamera::default();
    let far = camera.screen_to_world(Point::new(5_000.0, 5_000.0));
    assert!(place(&text("R1", [far.x, far.y]), &camera, VIEW).is_none());
}

#[test]
fn a_text_too_small_to_see_is_left_out() {
    let mut camera = BoardCamera::default();
    camera.zoom_by_factor(Point::ORIGIN, 1e-4);
    let at = camera.screen_to_world(Point::new(100.0, 100.0));
    // 0.1 mm at the widest zoom is about a tenth of a pixel.
    let tiny = FootprintTextInput {
        size_mm: 0.1,
        ..text("R1", [at.x, at.y])
    };
    assert!(place(&tiny, &camera, VIEW).is_none());
    let readable = text("R1", [at.x, at.y]);
    assert!(place(&readable, &camera, VIEW).is_some());
}

#[test]
fn the_text_layer_is_retained_until_the_camera_or_the_board_changes() {
    let mut canvas = PcbCanvas::new();
    canvas.set_renderer_snapshot(Some(PcbSnapshot {
        texts: vec![text("R1", [1.0, 1.0])],
        ..PcbSnapshot::default()
    }));
    let key = TextKey::live(&canvas, &canvas.camera_ref());
    assert_eq!(key, TextKey::live(&canvas, &canvas.camera_ref()));
    canvas.camera.borrow_mut().pan(3.0, 0.0);
    let panned = TextKey::live(&canvas, &canvas.camera_ref());
    assert_ne!(key, panned, "a pan redraws");
    canvas.clear_content_cache();
    assert_ne!(
        panned,
        TextKey::live(&canvas, &canvas.camera_ref()),
        "a board edit redraws"
    );
}

#[test]
fn the_carried_texts_move_with_the_drag_preview() {
    let mut canvas = PcbCanvas::new();
    canvas.set_renderer_snapshot(Some(PcbSnapshot::default()));
    let carried: BTreeSet<Uuid> = [Uuid::from_u128(1)].into();
    canvas.begin_drag_preview(carried, false, Vec::new(), PcbSnapshot::default());
    assert!(canvas.drag_preview_texts().is_empty());
    let moved = signex_types::pcb::Point::new(2.0, 0.0);
    canvas.move_drag_preview(moved, || PcbSnapshot {
        texts: vec![text("R1", [3.0, 1.0])],
        ..PcbSnapshot::default()
    });
    assert_eq!(canvas.drag_preview_texts(), [text("R1", [3.0, 1.0])]);
    canvas.end_drag_preview();
    assert!(canvas.drag_preview_texts().is_empty());
}

#[test]
fn the_view_size_does_not_change_a_placement() {
    let camera = BoardCamera::default();
    let r1 = text("R1", [10.0, 21.0]);
    let small = Rectangle::with_size(Size::new(2_000.0, 2_000.0));
    assert_eq!(place(&r1, &camera, VIEW), place(&r1, &camera, small));
}
