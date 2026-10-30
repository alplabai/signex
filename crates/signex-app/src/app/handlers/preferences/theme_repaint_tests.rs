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

/// #1769 - while a theme is only drafted, every theme-derived input must
/// follow the draft. A panel-context rebuild (any selection, tab or
/// project event) used to reset the tokens to the saved theme while the
/// iced palette stayed on the draft, giving dark surfaces under light text.
#[test]
fn a_panel_context_rebuild_keeps_the_drafted_palette() {
    let (mut app, _boot) = Signex::new();
    let saved = app.ui_state.theme_id;
    let drafted = if saved == ThemeId::SolarizedLight {
        ThemeId::Nord
    } else {
        ThemeId::SolarizedLight
    };
    app.ui_state.preferences_open = true;
    send(&mut app, PrefMsg::DraftTheme(drafted));

    app.refresh_panel_ctx();

    let ctx = &app.document_state.panel_ctx;
    assert_eq!(ctx.tokens, signex_types::theme::theme_tokens(drafted));
    assert_eq!(ctx.theme_id, drafted);
    let window = iced::window::Id::unique();
    assert_eq!(
        app.theme(window).map(|t| t.palette()),
        Some(Signex::id_to_iced_theme(drafted, None).palette()),
        "the iced palette and the panel tokens must come from the same theme"
    );
    assert_eq!(app.ui_state.theme_id, saved, "a draft must not commit");
}
