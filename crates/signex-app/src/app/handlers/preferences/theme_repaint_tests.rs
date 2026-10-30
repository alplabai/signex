//! #1724 - a theme pick must repaint the schematic paper and grid, which live
//! in the canvas background cache, not only the content layer.

use crate::app::{Message, PreferencesMsg, Signex};
use crate::canvas::CanvasSlot;
use crate::preferences::PrefMsg;
use signex_types::theme::ThemeId;

fn send(app: &mut Signex, msg: PrefMsg) {
    let _ = app.update(Message::Preferences(PreferencesMsg::Inner(msg)));
}

#[test]
fn drafting_a_theme_drops_the_active_canvas_background_cache() {
    let (mut app, _boot) = Signex::new();
    let before = app.interaction_state.active_canvas().bg_cache_clears;

    send(&mut app, PrefMsg::DraftTheme(ThemeId::SolarizedLight));

    assert_eq!(
        app.interaction_state.active_canvas().bg_cache_clears,
        before + 1
    );
}

#[test]
fn saving_a_theme_drops_the_background_cache_of_every_schematic_canvas() {
    let (mut app, _boot) = Signex::new();
    let undocked = iced::window::Id::unique();
    app.interaction_state
        .canvases
        .insert(undocked, CanvasSlot::new());
    send(&mut app, PrefMsg::DraftTheme(ThemeId::GitHubDark));
    let active = app.interaction_state.active_canvas().bg_cache_clears;
    let other = app.interaction_state.canvases[&undocked].bg_cache_clears;

    send(&mut app, PrefMsg::Save);

    assert_eq!(
        app.interaction_state.active_canvas().bg_cache_clears,
        active + 1
    );
    assert_eq!(
        app.interaction_state.canvases[&undocked].bg_cache_clears,
        other + 1
    );
}
