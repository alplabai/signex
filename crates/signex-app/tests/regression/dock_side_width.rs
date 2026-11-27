//! #2347 — a side dock is limited by the window, not a fixed 500 px cap:
//! it grows until the canvas keeps its minimum beside the other dock, and a
//! smaller window or the other dock opening pulls the wider dock back in.

use signex_app::app::{DragTarget, Message, Signex, UiMsg, WindowMsg};
use signex_app::dock::{DOCK_HANDLE, DockMessage, MIN_CANVAS_WIDTH, PanelPosition};
use signex_app::panels::PanelKind;

fn main_window(app: &Signex) -> iced::window::Id {
    app.ui_state.main_window_id.expect("main window")
}

/// Resize the main window the way the runtime does.
fn resize_main(app: &mut Signex, w: f32, h: f32) {
    let id = main_window(app);
    let _ = app.update(Message::Window(WindowMsg::WindowResizedFor(id, w, h)));
}

fn app_in_window(w: f32, h: f32) -> Signex {
    let (mut app, _boot) = Signex::new();
    resize_main(&mut app, w, h);
    for (pos, kind) in [
        (PanelPosition::Left, PanelKind::Projects),
        (PanelPosition::Right, PanelKind::Properties),
    ] {
        if !app.document_view.dock.has_panels(pos) {
            app.document_view.dock.add_panel(pos, kind);
        }
        assert!(
            !app.document_view.dock.is_collapsed(pos),
            "{pos:?} collapsed"
        );
    }
    app
}

fn drag_right_dock(app: &mut Signex, from: f32, to: f32) {
    let main = main_window(app);
    let _ = app.update(Message::Ui(UiMsg::DragStart(DragTarget::RightPanel)));
    let _ = app.update(Message::Ui(UiMsg::DragMove(from, 0.0)));
    let _ = app.update(Message::Ui(UiMsg::DragMove(to, 0.0)));
    let _ = app.update(Message::Ui(UiMsg::DragEnd(main)));
}

fn toggle_collapse(app: &mut Signex, side: PanelPosition) {
    let _ = app.update(Message::Dock(DockMessage::ToggleCollapse(side)));
}

/// Canvas width between the two open docks and their resize handles.
fn canvas_width(app: &Signex, window_w: f32) -> f32 {
    window_w - (app.ui_state.left_width + DOCK_HANDLE) - (app.ui_state.right_width + DOCK_HANDLE)
}

fn assert_canvas_at_minimum(app: &Signex, window_w: f32) {
    let canvas = canvas_width(app, window_w);
    assert!(
        (canvas - MIN_CANVAS_WIDTH).abs() < 0.5,
        "canvas {canvas} px, minimum {MIN_CANVAS_WIDTH} px"
    );
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
    assert_canvas_at_minimum(&app, 2560.0);
}

/// The repro: a right dock dragged wide on a 2560 px window, then the window
/// shrunk to 1200 px. The right dock gives way; the left keeps its width.
#[test]
fn a_smaller_window_pulls_the_widened_dock_back_in() {
    let mut app = app_in_window(2560.0, 1440.0);
    let left = app.ui_state.left_width;
    drag_right_dock(&mut app, 2000.0, -5000.0);
    resize_main(&mut app, 1200.0, 800.0);
    assert_eq!(app.ui_state.left_width, left, "the left dock was shrunk");
    assert_canvas_at_minimum(&app, 1200.0);
}

#[test]
fn another_windows_resize_leaves_the_docks_alone() {
    let mut app = app_in_window(2560.0, 1440.0);
    drag_right_dock(&mut app, 2000.0, -5000.0);
    let before = (app.ui_state.left_width, app.ui_state.right_width);
    let other = iced::window::Id::unique();
    let _ = app.update(Message::Window(WindowMsg::WindowResizedFor(
        other, 600.0, 400.0,
    )));
    assert_eq!(app.ui_state.window_size, (2560.0, 1440.0));
    assert_eq!((app.ui_state.left_width, app.ui_state.right_width), before);
}

/// The repro: the left dock collapsed, the right dragged as wide as it goes,
/// then the left expanded again. The right gives way; the left keeps its
/// width and the canvas keeps its minimum.
#[test]
fn expanding_the_other_dock_pulls_the_wide_dock_back_in() {
    let mut app = app_in_window(2560.0, 1440.0);
    let left = app.ui_state.left_width;
    toggle_collapse(&mut app, PanelPosition::Left);
    assert!(app.document_view.dock.is_collapsed(PanelPosition::Left));
    drag_right_dock(&mut app, 2000.0, -5000.0);
    toggle_collapse(&mut app, PanelPosition::Left);
    assert!(!app.document_view.dock.is_collapsed(PanelPosition::Left));
    assert_eq!(app.ui_state.left_width, left, "the left dock was shrunk");
    assert_canvas_at_minimum(&app, 2560.0);
}

#[test]
fn a_minimised_window_keeps_the_dock_widths() {
    let mut app = app_in_window(2560.0, 1440.0);
    drag_right_dock(&mut app, 2000.0, 1200.0);
    let wide = app.ui_state.right_width;
    resize_main(&mut app, 0.0, 0.0);
    assert_eq!(app.ui_state.right_width, wide);
}

fn drag_left_dock(app: &mut Signex, from: f32, to: f32) {
    let main = main_window(app);
    let _ = app.update(Message::Ui(UiMsg::DragStart(DragTarget::LeftPanel)));
    let _ = app.update(Message::Ui(UiMsg::DragMove(from, 0.0)));
    let _ = app.update(Message::Ui(UiMsg::DragMove(to, 0.0)));
    let _ = app.update(Message::Ui(UiMsg::DragEnd(main)));
}

/// Drag the `side` dock to `width` px wide.
fn drag_dock_to(app: &mut Signex, side: PanelPosition, width: f32) {
    match side {
        PanelPosition::Right => {
            let start = app.ui_state.right_width;
            drag_right_dock(app, 1000.0, 1000.0 - (width - start));
        }
        _ => {
            let start = app.ui_state.left_width;
            drag_left_dock(app, 300.0, 300.0 + (width - start));
        }
    }
}

fn shown_width(app: &Signex, side: PanelPosition) -> f32 {
    match side {
        PanelPosition::Right => app.ui_state.right_width,
        _ => app.ui_state.left_width,
    }
}

/// #2395: a dock widened to 420 px, the window shrunk to 300 px and back.
/// The small window narrows the dock; the restored window gives the user's
/// 420 px back.
fn a_window_shrunk_and_restored_returns_the_chosen_width(side: PanelPosition) {
    let mut app = app_in_window(1400.0, 900.0);
    drag_dock_to(&mut app, side, 420.0);
    assert_eq!(shown_width(&app, side), 420.0);
    assert_eq!(app.document_view.dock.preferred_width(side), 420.0);

    resize_main(&mut app, 300.0, 900.0);
    assert!(
        shown_width(&app, side) < 420.0,
        "the small window narrows it"
    );
    assert_eq!(
        app.document_view.dock.preferred_width(side),
        420.0,
        "a resize never changes the chosen width"
    );

    resize_main(&mut app, 1400.0, 900.0);
    assert_eq!(shown_width(&app, side), 420.0);
}

#[test]
fn the_right_dock_returns_to_its_width_after_a_small_window() {
    a_window_shrunk_and_restored_returns_the_chosen_width(PanelPosition::Right);
}

#[test]
fn the_left_dock_returns_to_its_width_after_a_small_window() {
    a_window_shrunk_and_restored_returns_the_chosen_width(PanelPosition::Left);
}

/// The repro: a minimising window first reports a tiny non-zero size, then
/// 0 x 0, then the restored size. Both docks come back to their widths.
#[test]
fn minimise_and_restore_return_both_docks_to_their_widths() {
    let mut app = app_in_window(1400.0, 900.0);
    drag_dock_to(&mut app, PanelPosition::Right, 420.0);
    drag_dock_to(&mut app, PanelPosition::Left, 300.0);
    resize_main(&mut app, 160.0, 28.0);
    resize_main(&mut app, 0.0, 0.0);
    resize_main(&mut app, 1400.0, 900.0);
    assert_eq!(app.ui_state.right_width, 420.0);
    assert_eq!(app.ui_state.left_width, 300.0);
}

/// Maximise grows nothing past the chosen widths; restore keeps them.
#[test]
fn maximise_and_restore_keep_both_dock_widths() {
    let mut app = app_in_window(1400.0, 900.0);
    drag_dock_to(&mut app, PanelPosition::Right, 420.0);
    drag_dock_to(&mut app, PanelPosition::Left, 300.0);
    resize_main(&mut app, 2560.0, 1440.0);
    assert_eq!(
        (app.ui_state.left_width, app.ui_state.right_width),
        (300.0, 420.0)
    );
    resize_main(&mut app, 1400.0, 900.0);
    assert_eq!(
        (app.ui_state.left_width, app.ui_state.right_width),
        (300.0, 420.0)
    );
}

/// A drag in a narrowed window sets a new chosen width, which a larger
/// window then keeps instead of the old one.
#[test]
fn a_dock_drag_updates_the_chosen_width() {
    let mut app = app_in_window(1400.0, 900.0);
    drag_dock_to(&mut app, PanelPosition::Right, 420.0);
    drag_dock_to(&mut app, PanelPosition::Right, 360.0);
    assert_eq!(
        app.document_view.dock.preferred_width(PanelPosition::Right),
        360.0
    );
    resize_main(&mut app, 300.0, 900.0);
    resize_main(&mut app, 1400.0, 900.0);
    assert_eq!(app.ui_state.right_width, 360.0);
}

/// The saved layout carries the chosen widths, not the fitted ones.
#[test]
fn the_saved_layout_holds_the_chosen_widths() {
    let mut app = app_in_window(1400.0, 900.0);
    drag_dock_to(&mut app, PanelPosition::Right, 420.0);
    resize_main(&mut app, 300.0, 900.0);
    let saved = signex_app::dock::DockLayoutSnapshot::capture(&app.document_view.dock);
    assert_eq!(saved.right.width, Some(420));
    assert_eq!(saved.bottom.width, None);
}
