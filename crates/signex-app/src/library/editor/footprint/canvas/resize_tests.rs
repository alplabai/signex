//! #2345 in the footprint editor: its camera lives in the canvas widget, so
//! the canvas follows a new size itself at the first event laid out at it
//! (a window resize or a dock drag alike). A view still at its fit is fitted
//! again; any other view keeps its centre and zoom.

use iced::widget::canvas::{self, Program};
use iced::{Color, Event, Point, Rectangle, mouse, window};

use crate::app::FootprintDocument;
use crate::library::editor::footprint::state::CourtyardRect;
use crate::library::messages::{EditorMsg, FootprintEditorMsg, LibraryMessage};
use crate::library::state::EditorAddress;

use super::{FootprintCanvas, FootprintCanvasState, MIN_SCALE};

const SMALL: Rectangle = Rectangle {
    x: 0.0,
    y: 0.0,
    width: 800.0,
    height: 600.0,
};
const LARGE: Rectangle = Rectangle {
    x: 0.0,
    y: 0.0,
    width: 1400.0,
    height: 900.0,
};

/// A `.snxfpt` tab's editor with a courtyard to fit.
fn document() -> FootprintDocument {
    let file = signex_library::FootprintFile::from_footprint(signex_library::Footprint::empty("f"));
    let mut doc = FootprintDocument::new(std::path::PathBuf::from("resize.snxfpt"), file);
    doc.state.courtyard_mm = Some(CourtyardRect {
        min_x: -3.0,
        min_y: -2.0,
        max_x: 3.0,
        max_y: 2.0,
    });
    doc
}

fn canvas_for<'a>(doc: &'a FootprintDocument, cache: &'a canvas::Cache) -> FootprintCanvas<'a> {
    FootprintCanvas {
        state: &doc.state,
        address: EditorAddress::new(
            doc.path.clone(),
            String::new(),
            signex_library::RowId::from_uuid(uuid::Uuid::nil()),
        ),
        bg_color: Color::WHITE,
        grid_color: Color::BLACK,
        cache,
        sketch: None,
        silk_f: &[],
        silk_b: &[],
        generated_silk: &[],
        zoom_step: 1.1,
        trackpad_pan: false,
        wheel_mode: Default::default(),
        mirror_card: None,
    }
}

/// One redraw of the canvas laid out at `bounds`; what it published.
fn redraw(
    fpc: &FootprintCanvas<'_>,
    cstate: &mut FootprintCanvasState,
    bounds: Rectangle,
) -> Option<LibraryMessage> {
    let event = Event::Window(window::Event::RedrawRequested(std::time::Instant::now()));
    fpc.update(cstate, &event, bounds, mouse::Cursor::Unavailable)
        .and_then(|a| a.into_inner().0)
}

/// A camera that fitted the footprint at its first frame, laid out at `bounds`.
fn fitted_at(fpc: &FootprintCanvas<'_>, bounds: Rectangle) -> FootprintCanvasState {
    let mut cstate = FootprintCanvasState::default();
    redraw(fpc, &mut cstate, bounds);
    assert!(cstate.did_initial_fit);
    cstate
}

fn centre(bounds: Rectangle) -> Point {
    Point::new(bounds.width / 2.0, bounds.height / 2.0)
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3 * a.abs().max(1.0)
}

#[test]
fn growing_the_canvas_refits_a_fitted_footprint() {
    let doc = document();
    let cache = canvas::Cache::new();
    let fpc = canvas_for(&doc, &cache);
    let mut cstate = fitted_at(&fpc, SMALL);

    let published = redraw(&fpc, &mut cstate, LARGE);

    let expected = fitted_at(&fpc, LARGE);
    assert!(
        close(cstate.scale, expected.scale)
            && close(cstate.offset.x, expected.offset.x)
            && close(cstate.offset.y, expected.offset.y),
        "not the fit of the new size: {cstate:?} vs {expected:?}"
    );
    assert!(
        matches!(
            published,
            Some(LibraryMessage::EditorEvent {
                msg: EditorMsg::Footprint(FootprintEditorMsg::ViewScaleChanged { px_per_mm }),
                ..
            }) if px_per_mm == cstate.scale
        ),
        "the refit scale reaches the status bar: {published:?}"
    );
}

#[test]
fn growing_the_canvas_keeps_the_centre_and_zoom_of_a_zoomed_footprint() {
    let doc = document();
    let cache = canvas::Cache::new();
    let fpc = canvas_for(&doc, &cache);
    let mut cstate = fitted_at(&fpc, SMALL);
    cstate.zoom_about(centre(SMALL), 1.5, MIN_SCALE);
    let (world, scale) = (cstate.screen_to_world(centre(SMALL)), cstate.scale);

    redraw(&fpc, &mut cstate, LARGE);

    let after = cstate.screen_to_world(centre(LARGE));
    assert!(
        (after.0 - world.0).abs() < 1e-3 && (after.1 - world.1).abs() < 1e-3,
        "the centre moved: {world:?} -> {after:?}"
    );
    assert_eq!(cstate.scale, scale, "the zoom changed");
}

#[test]
fn a_canvas_of_the_same_size_leaves_the_camera() {
    let doc = document();
    let cache = canvas::Cache::new();
    let fpc = canvas_for(&doc, &cache);
    let mut cstate = fitted_at(&fpc, SMALL);
    cstate.pan_by(iced::Vector::new(5.0, 7.0));
    let (offset, scale) = (cstate.offset, cstate.scale);

    redraw(&fpc, &mut cstate, SMALL);

    assert_eq!((cstate.offset, cstate.scale), (offset, scale));
}

/// The widget state outlives a tab switch; another footprint's fit is not
/// this one's, so its view keeps its centre instead of refitting.
#[test]
fn another_footprints_fit_is_not_refitted() {
    let doc = document();
    let cache = canvas::Cache::new();
    let mut cstate = fitted_at(&canvas_for(&doc, &cache), SMALL);
    let other = FootprintCanvas {
        address: EditorAddress::new(
            std::path::PathBuf::from("other.snxfpt"),
            String::new(),
            signex_library::RowId::from_uuid(uuid::Uuid::nil()),
        ),
        ..canvas_for(&doc, &cache)
    };
    let world = cstate.screen_to_world(centre(SMALL));
    let scale = cstate.scale;

    redraw(&other, &mut cstate, LARGE);

    let after = cstate.screen_to_world(centre(LARGE));
    assert!((after.0 - world.0).abs() < 1e-3 && (after.1 - world.1).abs() < 1e-3);
    assert_eq!(cstate.scale, scale);
}
