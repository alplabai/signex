//! #2345 in the symbol editor: the canvas reports a new size through
//! `ViewportMeasured` (a window resize or a dock drag alike), and the camera
//! follows by the schematic rule: a view still at its fit is fitted again,
//! any other keeps its centre and zoom.

use iced::widget::canvas::Program;
use iced::{Event, Point, Rectangle, mouse, window};
use std::path::{Path, PathBuf};

use super::canvas::CanvasState;
use crate::app::*;
use crate::app::{TabInfo, TabKind};
use crate::library::editor::standalone::symbol::{symbol_action_to_primitive_msg, symbol_canvas};
use crate::library::messages::{LibraryMessage, PrimitiveEdit, SymbolEditorMsg};
use crate::library::state::LibraryDisplaySettings;

const SMALL: Rectangle = Rectangle {
    x: 0.0,
    y: 0.0,
    width: 800.0,
    height: 500.0,
};
const LARGE: Rectangle = Rectangle {
    x: 0.0,
    y: 0.0,
    width: 1400.0,
    height: 900.0,
};

/// A main window showing a `.snxsym` tab of a two-pin symbol.
fn symbol_app() -> (Signex, PathBuf) {
    let (mut app, _task) = Signex::new();
    app.ui_state.main_window_id = Some(window::Id::unique());
    let path = PathBuf::from("resize.snxsym");
    let mut symbol = signex_library::Symbol::empty("s");
    for (number, x, y) in [("1", -10.0, 5.0), ("2", 10.0, -5.0)] {
        let mut pin = signex_library::SymbolPin::new(number, number);
        pin.position = [x, y];
        symbol.pins.push(pin);
    }
    let file = signex_library::SymbolFile::from_symbol(symbol);
    app.document_state
        .symbol_editors
        .insert(path.clone(), SymbolDocument::new(path.clone(), file));
    app.document_state.tabs.push(TabInfo {
        title: "resize".into(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::SymbolEditor(path.clone()),
    });
    app.document_state.active_tab = 0;
    (app, path)
}

fn send(app: &mut Signex, path: &Path, msg: SymbolEditorMsg) {
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.to_path_buf(),
        msg: PrimitiveEdit::Symbol(msg),
    }));
}

/// One redraw of the canvas the tab draws, laid out at `bounds`; what it
/// publishes goes through `Signex::update`. Whether it published anything.
fn redraw(app: &mut Signex, path: &Path, bounds: Rectangle) -> bool {
    let published = {
        let editor = &app.document_state.symbol_editors[path];
        let canvas = symbol_canvas(
            editor,
            &app.document_state.panel_ctx,
            LibraryDisplaySettings::default(),
            1.0,
        );
        let event = Event::Window(window::Event::RedrawRequested(std::time::Instant::now()));
        canvas
            .update(
                &mut CanvasState::default(),
                &event,
                bounds,
                mouse::Cursor::Unavailable,
            )
            .and_then(|action| action.into_inner().0)
    };
    let Some(action) = published else {
        return false;
    };
    send(app, path, symbol_action_to_primitive_msg(action));
    true
}

fn camera(app: &Signex, path: &Path) -> (Point, f32) {
    let camera = &app.document_state.symbol_editors[path].camera;
    (camera.offset, camera.scale)
}

fn centre_world(app: &Signex, path: &Path, bounds: Rectangle) -> Point {
    let camera = &app.document_state.symbol_editors[path].camera;
    camera.screen_to_world(Point::new(bounds.width / 2.0, bounds.height / 2.0), bounds)
}

#[test]
fn growing_the_canvas_refits_a_fitted_symbol() {
    let (mut app, path) = symbol_app();
    assert!(redraw(&mut app, &path, SMALL), "the first size is measured");
    assert!(
        !redraw(&mut app, &path, SMALL),
        "an unchanged size is not reported again"
    );
    let fitted_small = camera(&app, &path);

    assert!(redraw(&mut app, &path, LARGE), "the new size is reported");

    let (mut fresh, fresh_path) = symbol_app();
    redraw(&mut fresh, &fresh_path, LARGE);
    let followed = camera(&app, &path);
    assert_ne!(followed, fitted_small, "the view stayed where it was");
    assert_eq!(
        followed,
        camera(&fresh, &fresh_path),
        "not the fit of the new size"
    );
    assert_eq!(
        app.document_state.symbol_editors[&path].viewport,
        LARGE.size()
    );
}

#[test]
fn growing_the_canvas_keeps_the_centre_and_zoom_of_a_zoomed_symbol() {
    let (mut app, path) = symbol_app();
    redraw(&mut app, &path, SMALL);
    send(
        &mut app,
        &path,
        SymbolEditorMsg::ZoomFactor {
            sx: 300.0,
            sy: 200.0,
            factor: 1.5,
        },
    );
    let (centre, scale) = (centre_world(&app, &path, SMALL), camera(&app, &path).1);

    assert!(redraw(&mut app, &path, LARGE));

    let after = centre_world(&app, &path, LARGE);
    assert!(
        (after.x - centre.x).abs() < 1e-3 && (after.y - centre.y).abs() < 1e-3,
        "the centre moved: {centre:?} -> {after:?}"
    );
    assert_eq!(camera(&app, &path).1, scale, "the zoom changed");
}
