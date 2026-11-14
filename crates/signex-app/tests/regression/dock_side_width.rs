//! #2347 — a side dock is limited by the window, not a fixed 500 px cap:
//! it grows until the canvas keeps its minimum beside the other dock, and a
//! smaller window pulls it back in.

use signex_app::app::{DragTarget, Message, Signex, UiMsg};
use signex_app::dock::PanelPosition;
use signex_app::panels::PanelKind;
use signex_app::styles::TypeScale;

const BODY: f32 = TypeScale::DEFAULT.body;

fn app_in_window(w: f32, h: f32) -> Signex {
    let (mut app, _boot) = Signex::new();
    let _ = app.update(Message::Ui(UiMsg::WindowResized(w, h)));
    for (pos, kind) in [
        (PanelPosition::Left, PanelKind::Projects),
        (PanelPosition::Right, PanelKind::Properties),
    ] {
        if !app.document_state.dock.has_panels(pos) {
            app.document_state.dock.add_panel(pos, kind);
        }
        assert!(
            !app.document_state.dock.is_collapsed(pos),
            "{pos:?} collapsed"
        );
    }
    app
}

fn drag_right_dock(app: &mut Signex, from: f32, to: f32) {
    let _ = app.update(Message::Ui(UiMsg::DragStart(DragTarget::RightPanel)));
    let _ = app.update(Message::Ui(UiMsg::DragMove(from, 0.0)));
    let _ = app.update(Message::Ui(UiMsg::DragMove(to, 0.0)));
    let _ = app.update(Message::Ui(UiMsg::DragEnd(iced::window::Id::unique())));
}

/// Canvas width between the two open docks and their resize handles.
fn canvas_width(app: &Signex, window_w: f32) -> f32 {
    const HANDLE: f32 = 5.0;
    window_w - (app.ui_state.left_width + HANDLE) - (app.ui_state.right_width + HANDLE)
}

#[test]
fn a_side_dock_drags_wider_than_the_old_cap_on_a_wide_window() {
    let mut app = app_in_window(2560.0, 1440.0);
    let start = app.ui_state.right_width;
    drag_right_dock(&mut app, 2000.0, 1200.0);
    assert_eq!(app.ui_state.right_width, start + 800.0);
    assert!(app.ui_state.right_width > 500.0);
}

#[test]
fn a_side_dock_stops_where_the_canvas_keeps_its_minimum() {
    let mut app = app_in_window(2560.0, 1440.0);
    let left = app.ui_state.left_width;
    drag_right_dock(&mut app, 2000.0, -5000.0);
    assert_eq!(
        app.ui_state.left_width, left,
        "the other dock keeps its width"
    );
    let canvas = canvas_width(&app, 2560.0);
    assert!(canvas >= BODY * 20.0, "canvas squeezed to {canvas} px");
    assert!(
        canvas < BODY * 40.0,
        "dock stopped early, canvas {canvas} px"
    );
}

#[test]
fn a_smaller_window_pulls_a_wide_dock_back_in() {
    let mut app = app_in_window(2560.0, 1440.0);
    drag_right_dock(&mut app, 2000.0, -5000.0);
    let _ = app.update(Message::Ui(UiMsg::WindowResized(1200.0, 800.0)));
    let canvas = canvas_width(&app, 1200.0);
    assert!(canvas >= BODY * 20.0, "canvas squeezed to {canvas} px");
}

#[test]
fn a_minimised_window_keeps_the_dock_widths() {
    let mut app = app_in_window(2560.0, 1440.0);
    drag_right_dock(&mut app, 2000.0, 1200.0);
    let wide = app.ui_state.right_width;
    let _ = app.update(Message::Ui(UiMsg::WindowResized(0.0, 0.0)));
    assert_eq!(app.ui_state.right_width, wide);
}
