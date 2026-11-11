//! #2262: a fit keeps the footprint clear of the floating chrome, and the
//! zoom pill's requests zoom the camera exactly once.

use iced::widget::canvas::{self, Program};
use iced::{Color, Event, Point, Rectangle, mouse, window};

use crate::library::editor::footprint::state::{CourtyardRect, FootprintEditorState};
use crate::library::messages::{EditorMsg, FootprintEditorMsg, LibraryMessage};
use crate::library::state::EditorAddress;

use super::input::chrome_insets;
use super::{FootprintCanvas, FootprintCanvasState};

const BOUNDS: Rectangle = Rectangle {
    x: 0.0,
    y: 0.0,
    width: 800.0,
    height: 600.0,
};

fn canvas_for<'a>(
    state: &'a FootprintEditorState,
    cache: &'a canvas::Cache,
) -> FootprintCanvas<'a> {
    FootprintCanvas {
        state,
        address: EditorAddress::new(
            std::path::PathBuf::from("/tmp/lib.snxlib"),
            "footprints".to_string(),
            signex_library::RowId::new(),
        ),
        bg_color: Color::WHITE,
        grid_color: Color::BLACK,
        cache,
        sketch: None,
        silk_f: &[],
        silk_b: &[],
        zoom_step: 1.1,
        trackpad_pan: false,
        wheel_mode: Default::default(),
    }
}

/// A tall footprint, so the fit is height-bound and touches the top and
/// bottom of whatever area it fits into.
fn tall_footprint() -> FootprintEditorState {
    let mut state = FootprintEditorState::empty();
    state.courtyard_mm = Some(CourtyardRect {
        min_x: -1.0,
        min_y: -4.0,
        max_x: 1.0,
        max_y: 4.0,
    });
    state
}

fn redraw(fpc: &FootprintCanvas<'_>, cstate: &mut FootprintCanvasState) -> Option<LibraryMessage> {
    let event = Event::Window(window::Event::RedrawRequested(std::time::Instant::now()));
    fpc.update(
        cstate,
        &event,
        BOUNDS,
        mouse::Cursor::Available(Point::new(1.0, 1.0)),
    )
    .and_then(|a| a.into_inner().0)
}

fn published_scale(message: Option<LibraryMessage>) -> f32 {
    match message {
        Some(LibraryMessage::EditorEvent {
            msg: EditorMsg::Footprint(FootprintEditorMsg::ViewScaleChanged { px_per_mm }),
            ..
        }) => px_per_mm,
        other => panic!("expected a ViewScaleChanged, got {other:?}"),
    }
}

#[test]
fn the_first_fit_keeps_content_out_from_under_the_chips_and_the_pill() {
    let state = tall_footprint();
    let cache = canvas::Cache::new();
    let fpc = canvas_for(&state, &cache);
    let mut cstate = FootprintCanvasState::default();
    redraw(&fpc, &mut cstate);

    let (min_x, min_y, max_x, max_y) = state.content_bbox_mm().expect("content");
    let top_left = cstate.world_to_screen((min_x, min_y));
    let bottom_right = cstate.world_to_screen((max_x, max_y));
    let insets = chrome_insets();
    assert!(
        top_left.y >= insets.top - 0.5,
        "top edge {} sits under the chips (inset {})",
        top_left.y,
        insets.top
    );
    assert!(
        bottom_right.y <= BOUNDS.height - insets.bottom + 0.5,
        "bottom edge {} sits under the zoom pill (inset {})",
        bottom_right.y,
        insets.bottom
    );
}

#[test]
fn a_pending_fit_keeps_content_clear_of_the_chrome_too() {
    let mut state = tall_footprint();
    state.fit_pending = true;
    let cache = canvas::Cache::new();
    let fpc = canvas_for(&state, &cache);
    let mut cstate = FootprintCanvasState::with_view(5.0, Point::ORIGIN);
    fpc.apply_pending_fit(&mut cstate, BOUNDS);

    let (_, min_y, _, _) = state.content_bbox_mm().expect("content");
    let top = cstate.world_to_screen((0.0, min_y)).y;
    assert!(
        top >= chrome_insets().top - 0.5,
        "top edge {top} under the chips"
    );
}

#[test]
fn a_zoom_request_zooms_once_about_the_centre() {
    let mut state = tall_footprint();
    state.zoom_request.step(2.0);
    let cache = canvas::Cache::new();
    let fpc = canvas_for(&state, &cache);
    let mut cstate = FootprintCanvasState::with_view(10.0, Point::new(400.0, 300.0));

    let scale = published_scale(redraw(&fpc, &mut cstate));
    assert_eq!(scale, 20.0);
    assert_eq!(cstate.scale, 20.0);
    // The world point at the canvas centre stays there.
    assert_eq!(cstate.world_to_screen((0.0, 0.0)), Point::new(400.0, 300.0));

    // A second event before the model hears back must not zoom again.
    let again = redraw(&fpc, &mut cstate);
    assert!(
        !matches!(
            again,
            Some(LibraryMessage::EditorEvent {
                msg: EditorMsg::Footprint(FootprintEditorMsg::ViewScaleChanged { .. }),
                ..
            })
        ),
        "the same request zoomed twice: {again:?}"
    );
    assert_eq!(cstate.scale, 20.0);
}

#[test]
fn a_new_camera_does_not_replay_an_old_zoom_request() {
    let mut state = tall_footprint();
    state.zoom_request.step(2.0);
    let cache = canvas::Cache::new();
    let fpc = canvas_for(&state, &cache);
    let mut cstate = FootprintCanvasState::default();
    redraw(&fpc, &mut cstate);
    let fitted = cstate.scale;

    assert!(fpc.apply_pending_zoom(&mut cstate, BOUNDS).is_none());
    assert_eq!(cstate.scale, fitted);
}

/// The canvas a standalone tab for `path` draws.
fn tab_canvas<'a>(
    path: &str,
    state: &'a FootprintEditorState,
    cache: &'a canvas::Cache,
) -> FootprintCanvas<'a> {
    FootprintCanvas {
        address: EditorAddress::new(
            std::path::PathBuf::from(path),
            String::new(),
            signex_library::RowId::from_uuid(uuid::Uuid::nil()),
        ),
        ..canvas_for(state, cache)
    }
}

/// The canvas widget keeps its state across a tab switch. Footprint B's
/// request number is not compared with A's, so the switch does not zoom,
/// and B's next click zooms once.
#[test]
fn switching_footprint_tabs_does_not_replay_a_zoom() {
    let mut a = tall_footprint();
    a.zoom_request.step(2.0);
    let mut b = tall_footprint();
    for _ in 0..3 {
        b.zoom_request.step(1.25);
    }
    let cache = canvas::Cache::new();
    let mut cstate = FootprintCanvasState::default();
    redraw(&tab_canvas("a.snxfpt", &a, &cache), &mut cstate);
    a.zoom_request.step(2.0);
    published_scale(redraw(&tab_canvas("a.snxfpt", &a, &cache), &mut cstate));
    let on_a = cstate.scale;

    let tab_b = tab_canvas("b.snxfpt", &b, &cache);
    assert!(redraw(&tab_b, &mut cstate).is_none());
    assert!(redraw(&tab_b, &mut cstate).is_none());
    assert_eq!(cstate.scale, on_a, "switching to B zoomed");

    b.zoom_request.step(2.0);
    let scale = published_scale(redraw(&tab_canvas("b.snxfpt", &b, &cache), &mut cstate));
    assert_eq!(scale, on_a * 2.0);
}

/// Two clicks before the canvas runs zoom by both.
#[test]
fn two_pending_zoom_steps_both_apply() {
    let mut state = tall_footprint();
    state.zoom_request.step(2.0);
    state.zoom_request.step(1.5);
    let cache = canvas::Cache::new();
    let fpc = canvas_for(&state, &cache);
    let mut cstate = FootprintCanvasState::with_view(10.0, Point::new(400.0, 300.0));

    assert_eq!(published_scale(redraw(&fpc, &mut cstate)), 30.0);
}

/// Fit replaces a zoom the pill asked for before it.
#[test]
fn a_fit_clears_a_pending_zoom() {
    let mut state = tall_footprint();
    let cache = canvas::Cache::new();
    let mut cstate = FootprintCanvasState::default();
    redraw(&tab_canvas("f.snxfpt", &state, &cache), &mut cstate);
    let fitted = cstate.scale;

    state.zoom_request.step(2.0);
    state.fit_pending = true;
    let fpc = tab_canvas("f.snxfpt", &state, &cache);
    redraw(&fpc, &mut cstate);
    assert!(fpc.apply_pending_zoom(&mut cstate, BOUNDS).is_none());
    assert_eq!(cstate.scale, fitted);
}
