//! The footprint and symbol editors' Active Bars stop the wheel over their own
//! bounds only (#562); the canvas beside the bar still takes it. Each test
//! stacks a wheel probe (standing for the canvas) under the editor's bar
//! overlay, as `view` does.

use std::path::PathBuf;

use iced::mouse::ScrollDelta;
use iced::{Element, Point, Size};
use signex_types::theme::ThemeId;

use super::*;
use crate::app::{FootprintDocument, SymbolDocument, TabInfo, TabKind};
use crate::canvas::CanvasEvent;
use crate::layout_probe::messages_on_scroll_at;

const WINDOW: Size = Size::new(1400.0, 900.0);

fn app_showing(kind: fn(PathBuf) -> TabKind) -> (Signex, iced::window::Id, PathBuf) {
    let (mut app, _task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    let main = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(main);
    app.ui_state.theme_id = ThemeId::GraphiteLight;
    app.ui_state.preferences_draft_theme = ThemeId::GraphiteLight;
    app.sync_panel_theme();
    app.ui_state.window_size = (WINDOW.width, WINDOW.height);
    let path = PathBuf::from("bar-wheel.snxlib-file");
    app.document_state.tabs.push(TabInfo {
        title: "e".into(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: kind(path.clone()),
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    (app, main, path)
}

/// The probe under the overlays: it publishes `ViewMoved` for a wheel that
/// reaches it.
fn canvas_probe<'a>() -> Element<'a, Message> {
    iced::widget::mouse_area(
        iced::widget::Space::new()
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .on_scroll(|_| Message::CanvasEvent(CanvasEvent::ViewMoved))
    .boxed()
}

fn wheel_reaches_probe(overlays: Vec<Element<'_, Message>>, at: Point) -> bool {
    let mut layers = vec![canvas_probe()];
    layers.extend(overlays);
    let stack = iced::widget::Stack::with_children(layers).boxed();
    messages_on_scroll_at(stack, WINDOW, at, ScrollDelta::Lines { x: 0.0, y: 1.0 })
        .iter()
        .any(|m| matches!(m, Message::CanvasEvent(CanvasEvent::ViewMoved)))
}

/// Window y of the middle of the bar, `drop` px below the bar margin.
fn bar_y(app: &Signex, drop: f32) -> f32 {
    let m = signex_widgets::active_bar::Metrics::GRAPHITE;
    app.canvas_bounds().y
        + crate::active_bar::bar_top_margin()
        + drop
        + crate::active_bar::bar_height_for(&m) / 2.0
}

#[test]
fn the_footprint_bar_stops_the_wheel_but_the_canvas_beside_it_takes_it() {
    let (mut app, main, path) = app_showing(TabKind::FootprintEditor);
    let file = signex_library::FootprintFile::from_footprint(signex_library::Footprint::empty("f"));
    app.document_state
        .footprint_editors
        .insert(path.clone(), FootprintDocument::new(path, file));
    let y = bar_y(&app, 0.0);
    let beside = Point::new(app.canvas_bounds().x + 24.0, y);
    let over_bar = Point::new(WINDOW.width / 2.0, y);
    assert!(
        wheel_reaches_probe(app.footprint_active_bar_overlay(main), beside),
        "the bar's row took the wheel"
    );
    assert!(
        !wheel_reaches_probe(app.footprint_active_bar_overlay(main), over_bar),
        "the wheel went through the bar"
    );
}

#[test]
fn the_symbol_bar_stops_the_wheel_but_the_canvas_beside_it_takes_it() {
    let (mut app, main, path) = app_showing(TabKind::SymbolEditor);
    let file = signex_library::SymbolFile::from_symbol(signex_library::Symbol::empty("s"));
    app.document_state
        .symbol_editors
        .insert(path.clone(), SymbolDocument::new(path, file));
    let drop = crate::active_bar::symbol_editor_bar_drop(&app.document_state.panel_ctx.tokens);
    let y = bar_y(&app, drop);
    let beside = Point::new(app.canvas_bounds().x + 24.0, y);
    let over_bar = Point::new(WINDOW.width / 2.0, y);
    assert!(
        wheel_reaches_probe(app.symbol_editor_active_bar_overlay(main), beside),
        "the bar's row took the wheel"
    );
    assert!(
        !wheel_reaches_probe(app.symbol_editor_active_bar_overlay(main), over_bar),
        "the wheel went through the bar"
    );
}
