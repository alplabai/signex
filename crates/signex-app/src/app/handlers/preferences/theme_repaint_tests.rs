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

#[test]
fn importing_a_theme_file_repaints_every_open_canvas() {
    let (mut app, _boot) = Signex::new();
    let undocked = iced::window::Id::unique();
    app.interaction_state
        .canvases
        .insert(undocked, CanvasSlot::new());
    app.interaction_state
        .pcb_canvases
        .insert(undocked, crate::pcb_canvas::PcbCanvas::default());
    let canvas = signex_types::theme::canvas_colors(ThemeId::SolarizedLight);
    let file = signex_types::theme::CustomThemeFile {
        name: "Imported".to_string(),
        tokens: signex_types::theme::theme_tokens(ThemeId::SolarizedLight),
        canvas,
    };
    let bg = app.interaction_state.canvases[&undocked].bg_cache_clears;

    send(
        &mut app,
        PrefMsg::ThemeFileLoaded(serde_json::to_string(&file).unwrap()),
    );

    assert_eq!(
        app.interaction_state.canvases[&undocked].bg_cache_clears,
        bg + 1
    );
    for pcb in app.interaction_state.all_pcb_canvases_mut() {
        assert_eq!(pcb.canvas_colors, canvas);
    }
}

fn drafted_other_than_saved(app: &Signex) -> ThemeId {
    if app.ui_state.theme_id == ThemeId::SolarizedLight {
        ThemeId::Nord
    } else {
        ThemeId::SolarizedLight
    }
}

/// #1769 - the icon tint (`panel_ctx.theme_id`) moves with the tokens the
/// moment a theme is drafted, not only after the next panel refresh.
#[test]
fn drafting_a_theme_moves_the_icon_theme_without_a_refresh() {
    let (mut app, _boot) = Signex::new();
    let drafted = drafted_other_than_saved(&app);
    app.ui_state.preferences_open = true;

    send(&mut app, PrefMsg::DraftTheme(drafted));

    let ctx = &app.document_state.panel_ctx;
    assert_eq!(ctx.theme_id, drafted);
    assert_eq!(ctx.tokens, signex_types::theme::theme_tokens(drafted));
}

/// #1769 - draft, refresh, discard: tokens and icon theme both return to
/// the saved theme.
#[test]
fn discarding_after_a_refresh_restores_the_tokens_and_the_icon_theme() {
    let (mut app, _boot) = Signex::new();
    let saved = app.ui_state.theme_id;
    let drafted = drafted_other_than_saved(&app);
    app.ui_state.preferences_open = true;
    send(&mut app, PrefMsg::DraftTheme(drafted));
    app.refresh_panel_ctx();

    send(&mut app, PrefMsg::DiscardAndClose);

    let ctx = &app.document_state.panel_ctx;
    assert_eq!(ctx.theme_id, saved);
    assert_eq!(ctx.tokens, signex_types::theme::theme_tokens(saved));
}

fn custom_theme_file(source: ThemeId) -> String {
    let file = signex_types::theme::CustomThemeFile {
        name: "Imported".to_string(),
        tokens: signex_types::theme::theme_tokens(source),
        canvas: signex_types::theme::canvas_colors(source),
    };
    serde_json::to_string(&file).unwrap()
}

/// #1769 - importing a theme file previews it on the panels at once, and
/// importing a second file replaces the first one's tokens.
#[test]
fn importing_a_theme_file_moves_the_panel_theme_without_a_refresh() {
    let (mut app, _boot) = Signex::new();
    app.ui_state.preferences_open = true;

    send(
        &mut app,
        PrefMsg::ThemeFileLoaded(custom_theme_file(ThemeId::SolarizedLight)),
    );
    let ctx = &app.document_state.panel_ctx;
    assert_eq!(ctx.theme_id, ThemeId::Custom);
    assert_eq!(
        ctx.tokens,
        signex_types::theme::theme_tokens(ThemeId::SolarizedLight)
    );

    send(
        &mut app,
        PrefMsg::ThemeFileLoaded(custom_theme_file(ThemeId::Nord)),
    );
    assert_eq!(
        app.document_state.panel_ctx.tokens,
        signex_types::theme::theme_tokens(ThemeId::Nord),
        "a second import replaces the first one's tokens"
    );
}
