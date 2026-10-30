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

#[test]
fn drafting_a_theme_repaints_an_undocked_schematic_canvas() {
    let (mut app, _boot) = Signex::new();
    let undocked = iced::window::Id::unique();
    app.interaction_state
        .canvases
        .insert(undocked, CanvasSlot::new());
    let bg = app.interaction_state.canvases[&undocked].bg_cache_clears;
    let content = app.interaction_state.canvases[&undocked].content_cache_clears;

    send(&mut app, PrefMsg::DraftTheme(ThemeId::SolarizedLight));

    let slot = &app.interaction_state.canvases[&undocked];
    assert_eq!(slot.bg_cache_clears, bg + 1);
    assert_eq!(slot.content_cache_clears, content + 1);
}

#[test]
fn drafting_a_theme_recolours_every_pcb_canvas() {
    let (mut app, _boot) = Signex::new();
    let undocked = iced::window::Id::unique();
    app.interaction_state
        .pcb_canvases
        .insert(undocked, crate::pcb_canvas::PcbCanvas::default());

    send(&mut app, PrefMsg::DraftTheme(ThemeId::SolarizedLight));

    let wanted = signex_types::theme::canvas_colors(ThemeId::SolarizedLight);
    for canvas in app.interaction_state.all_pcb_canvases_mut() {
        assert_eq!(canvas.canvas_colors, wanted);
    }
}

#[test]
fn reverting_drafts_repaints_the_undocked_canvases_with_the_saved_theme() {
    let (mut app, _boot) = Signex::new();
    let undocked = iced::window::Id::unique();
    app.interaction_state
        .canvases
        .insert(undocked, CanvasSlot::new());
    app.interaction_state
        .pcb_canvases
        .insert(undocked, crate::pcb_canvas::PcbCanvas::default());
    send(&mut app, PrefMsg::DraftTheme(ThemeId::SolarizedLight));
    let bg = app.interaction_state.canvases[&undocked].bg_cache_clears;

    app.revert_preferences_drafts();

    assert_eq!(
        app.interaction_state.canvases[&undocked].bg_cache_clears,
        bg + 1
    );
    let saved = signex_types::theme::canvas_colors(app.ui_state.theme_id);
    assert_eq!(app.ui_state.preferences_draft_theme, app.ui_state.theme_id);
    for canvas in app.interaction_state.all_pcb_canvases_mut() {
        assert_eq!(canvas.canvas_colors, saved);
    }
}
