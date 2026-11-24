//! Mockup row 61 (#2822): the Selection Filter opens as a compact card
//! anchored under its bar button, and stays inside a window too narrow for
//! the bar.

use super::*;
use crate::active_bar::{ActiveBarMenu, bar_left_in_window, dropdown_x_offset};
use crate::layout_probe::lay_out_texts;

/// Texts every Selection Filter card shows, left to right and top to bottom.
const CONTROLS: [&str; 5] = ["All", "None", "Sheet Symbols", "Other", "Save current"];

fn app_with_filter_open(window: iced::Size) -> Signex {
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.window_size = (window.width, window.height);
    app.interaction_state.active_bar_menu = Some(ActiveBarMenu::Filter);
    app.interaction_state.active_bar_menu_window = None;
    app
}

/// Where each of [`CONTROLS`] lands in the laid-out overlay.
fn control_bounds(app: &Signex, window: iced::Size) -> Vec<(&'static str, iced::Rectangle)> {
    let mut layers = app.active_bar_menu_overlay(iced::window::Id::unique());
    assert_eq!(layers.len(), 1, "one layer: dismiss layer and card");
    let layer = scroll_guard(layers.pop().expect("one layer"));
    let texts = lay_out_texts(layer, window);
    CONTROLS
        .iter()
        .map(|label| (*label, crate::layout_probe::find(&texts, label).bounds))
        .collect()
}

/// The Filter trigger's left edge in window space, as the bar draws it.
fn trigger_left(app: &Signex, window: iced::Size) -> f32 {
    let m = signex_widgets::active_bar::Metrics::GRAPHITE;
    dropdown_x_offset(ActiveBarMenu::Filter, &m)
        + bar_left_in_window(app.interaction_state.current_tool, window.width, &m)
}

#[test]
fn the_filter_card_opens_under_its_button_at_1440_by_900() {
    let window = iced::Size::new(1440.0, 900.0);
    let app = app_with_filter_open(window);
    let left = trigger_left(&app, window);
    let card = crate::active_bar::filter_card_width();
    let m = signex_widgets::active_bar::Metrics::GRAPHITE;
    let bar_bottom = crate::active_bar::bar_top_margin() + crate::active_bar::bar_height_for(&m);
    for (label, at) in control_bounds(&app, window) {
        assert!(at.x >= left, "{label} left of the button: {at:?} < {left}");
        assert!(
            at.x + at.width <= left + card,
            "{label} past the compact card: {at:?}, card {left}..{}",
            left + card
        );
        assert!(at.y >= bar_bottom, "{label} over the bar: {at:?}");
        assert!(at.y + at.height <= window.height, "{label}: {at:?}");
    }
}

#[test]
fn the_filter_card_stays_inside_a_window_narrower_than_the_bar() {
    let window = iced::Size::new(360.0, 600.0);
    let app = app_with_filter_open(window);
    // The bar is centred, so here its left edge is off the window.
    assert!(trigger_left(&app, window) < 0.0, "the window is too wide");
    let card = crate::active_bar::filter_card_width();
    let shown = control_bounds(&app, window);
    for (label, at) in &shown {
        assert!(at.x >= 0.0, "{label} off the left edge: {at:?}");
        assert!(
            at.x + at.width <= window.width,
            "{label} off the right edge: {at:?}"
        );
    }
    // Still the compact card, not stretched to the window.
    let lefts = shown.iter().map(|(_, at)| at.x).fold(f32::MAX, f32::min);
    let rights = shown
        .iter()
        .map(|(_, at)| at.x + at.width)
        .fold(f32::MIN, f32::max);
    assert!(rights - lefts <= card, "{lefts}..{rights} wider than {card}");
}
