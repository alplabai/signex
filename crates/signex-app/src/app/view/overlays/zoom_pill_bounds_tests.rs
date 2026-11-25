//! #2911: the zoom pill sits inside the canvas it floats over, beside an open
//! right dock and with no dock, at the size the window is laid out at. The
//! headless renders draw at 1280x800 while the model still holds the
//! 1400x900 it starts with; the pill used the model's size and landed over
//! the Properties dock.

use iced::{Point, Rectangle, Size};

use super::zoom_pill::pill_origin;
use crate::app::Signex;
use crate::canvas::overlay_camera_tests::graphite_app_with_selected_wire;
use crate::canvas::zoom_pill::pill_size;
use crate::dock::PanelPosition;
use crate::layout_probe::{LaidOutText, lay_out_texts};

/// The size the headless renders lay the window out at.
const LAID_OUT: Size = Size::new(1280.0, 800.0);
/// The size `Signex::new` starts the model with, before any resize event.
const MODEL_DEFAULT: (f32, f32) = (1400.0, 900.0);

fn pill_rect(app: &Signex, size: Size) -> Rectangle {
    let (w, h) = pill_size();
    let (x, y) = pill_origin(app.canvas_bounds_sized(None, (size.width, size.height)));
    Rectangle::new(Point::new(x, y), Size::new(w, h))
}

fn holds(outer: Rectangle, inner: Rectangle) -> bool {
    inner.x >= outer.x
        && inner.y >= outer.y
        && inner.x + inner.width <= outer.x + outer.width
        && inner.y + inner.height <= outer.y + outer.height
}

/// `-`, the percentage, `+` and Fit.
fn is_pill_text(text: &str) -> bool {
    matches!(text, "\u{2212}" | "+" | "Fit") || text.ends_with('%')
}

/// The main window laid out at `size` draws the pill inside the canvas
/// rectangle, with no other label under it.
fn assert_pill_inside_canvas(app: &Signex, main: iced::window::Id, size: Size, case: &str) {
    let canvas = app.canvas_bounds_sized(None, (size.width, size.height));
    let pill = pill_rect(app, size);
    assert!(
        holds(canvas, pill),
        "{case}: pill {pill:?} leaves canvas {canvas:?}"
    );
    let texts = lay_out_texts(app.view(main), size);
    let fit = texts
        .iter()
        .find(|t| t.text == "Fit")
        .unwrap_or_else(|| panic!("{case}: no zoom pill: {texts:#?}"));
    assert!(
        pill.contains(fit.bounds.center()),
        "{case}: Fit at {:?} is not in the pill at {pill:?}",
        fit.bounds
    );
    assert!(
        canvas.contains(fit.bounds.center()),
        "{case}: Fit at {:?} is outside canvas {canvas:?}",
        fit.bounds
    );
    let covered: Vec<&LaidOutText> = texts
        .iter()
        .filter(|t| !is_pill_text(&t.text) && t.bounds.intersects(&pill))
        .collect();
    assert!(covered.is_empty(), "{case}: the pill covers {covered:#?}");
}

fn assert_right_dock_open(app: &Signex) {
    let dock = &app.document_state.dock;
    assert!(dock.has_panels(PanelPosition::Right), "no right dock");
    assert!(!dock.is_collapsed(PanelPosition::Right), "right dock collapsed");
}

/// The issue's case: laid out smaller than the model's size, the pill still
/// anchors to the canvas the layout draws, not the one the model expects.
#[test]
fn the_pill_stays_inside_the_canvas_when_the_window_size_is_stale() {
    let (mut app, main) = graphite_app_with_selected_wire();
    app.ui_state.window_size = MODEL_DEFAULT;
    assert_right_dock_open(&app);
    assert_pill_inside_canvas(&app, main, LAID_OUT, "stale model size");
    let model = Size::new(MODEL_DEFAULT.0, MODEL_DEFAULT.1);
    assert_pill_inside_canvas(&app, main, model, "model size");
}

/// With a stale model size the preset menu still opens over the drawn pill:
/// its rows sit in the card `presets_anchor` places against the laid-out
/// canvas, above the pill and inside the canvas.
#[test]
fn the_presets_follow_the_pill_when_the_window_size_is_stale() {
    let (mut app, main) = graphite_app_with_selected_wire();
    app.ui_state.window_size = MODEL_DEFAULT;
    let _ = app.update(crate::app::Message::Ui(
        crate::app::UiMsg::ZoomPresetsToggled,
    ));
    assert!(app.zoom_presets_shown(), "the presets did not open");
    let canvas = app.canvas_bounds_sized(None, (LAID_OUT.width, LAID_OUT.height));
    let pill = pill_rect(&app, LAID_OUT);
    let zoom = app.interaction_state.active_canvas().zoom_percent();
    let has_selection = app.command_context(None).has_selection;
    let (_, extent) = app.zoom_presets_card(zoom, has_selection);
    let (x, y) = super::zoom_presets::presets_anchor(canvas, extent);
    let card = Rectangle::new(Point::new(x, y), Size::new(extent.0, extent.1));
    assert!(holds(canvas, card), "card {card:?} leaves canvas {canvas:?}");
    assert!(
        card.y + card.height <= pill.y,
        "card {card:?} overlaps pill {pill:?}"
    );
    let texts = lay_out_texts(app.view(main), LAID_OUT);
    for label in ["50%", "200%", "Fit Selection"] {
        let row = texts
            .iter()
            .find(|t| t.text == label && !pill.contains(t.bounds.center()))
            .unwrap_or_else(|| panic!("no {label:?} row: {texts:#?}"));
        assert!(
            card.contains(row.bounds.center()),
            "{label:?} at {:?} is not in the card at {card:?}",
            row.bounds
        );
    }
}

#[test]
fn the_pill_stays_inside_the_canvas_at_every_right_dock_width() {
    let (mut app, main) = graphite_app_with_selected_wire();
    app.ui_state.window_size = (LAID_OUT.width, LAID_OUT.height);
    assert_right_dock_open(&app);
    for width in [crate::dock::MIN_SIDE_DOCK_WIDTH, 320.0, 480.0] {
        app.ui_state.right_width = width;
        assert_pill_inside_canvas(&app, main, LAID_OUT, &format!("right dock {width} px"));
    }
}

#[test]
fn the_pill_stays_inside_the_canvas_with_no_dock() {
    let (mut app, main) = graphite_app_with_selected_wire();
    app.ui_state.window_size = MODEL_DEFAULT;
    let positions = [
        PanelPosition::Left,
        PanelPosition::Right,
        PanelPosition::Bottom,
    ];
    for pos in positions {
        for kind in app.document_state.dock.panel_kinds(pos).to_vec() {
            assert!(app.document_state.dock.close_panel(kind), "{kind:?}");
        }
    }
    for pos in positions {
        assert!(!app.document_state.dock.has_panels(pos), "{pos:?} still docked");
    }
    assert_pill_inside_canvas(&app, main, LAID_OUT, "no dock, stale model size");
    let canvas = app.canvas_bounds_sized(None, (LAID_OUT.width, LAID_OUT.height));
    assert_eq!(
        (canvas.x, canvas.width),
        (0.0, LAID_OUT.width),
        "with no dock the canvas spans the window"
    );
}
