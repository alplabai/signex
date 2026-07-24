//! Review #308 findings 1 + 2 — Preferences dirty-tracking must not miss an
//! imperative edit, and no dismiss route may discard unsaved changes without
//! the user being asked (or at least without being allowed to silently
//! proceed). See `crate::app::state::UiState::preferences_draft_differs` /
//! `preferences_has_unsaved_changes` and
//! `handle_preferences_close_requested` / `WindowMsg::WindowCloseRequested`.

use signex_app::app::{Message, PreferencesMsg, Signex, WindowMsg};
use signex_app::preferences::PrefMsg;
use signex_types::theme::{CustomThemeFile, ThemeId, canvas_colors, theme_tokens};

fn custom_theme_json(name: &str) -> String {
    let custom = CustomThemeFile {
        name: name.to_string(),
        tokens: theme_tokens(ThemeId::Signex),
        canvas: canvas_colors(ThemeId::Signex),
    };
    serde_json::to_string(&custom).expect("serialise a fixture CustomThemeFile")
}

/// Finding 1 (data-loss): `preferences_draft_differs()` used to compare only
/// the 7 appearance drafts, so an imported theme (which sets
/// `preferences_dirty` imperatively via `custom_theme`, invisible to that
/// comparator) got silently clobbered back to "clean" by the next unrelated
/// appearance-draft recompute — exactly reproducing the review's repro:
/// theme ALREADY Custom (so `preferences_draft_theme` reads `Custom` both
/// before and after the import — the enum-tag comparison alone can't see a
/// same-tag content swap) → Import Theme replaces `custom_theme`'s content →
/// toggle an unrelated appearance draft.
#[test]
fn theme_import_dirty_flag_survives_an_unrelated_appearance_toggle() {
    let (mut app, _t) = Signex::new();

    // Precondition: theme is ALREADY Custom (live + draft agree) BEFORE the
    // dialog opens, so the real open flow (`seed_preferences_drafts_from_live`)
    // seeds `preferences_draft_theme = Custom` from the live value — the
    // draft-theme enum comparison is then a no-op before AND after the
    // import, isolating the sticky flag as the only thing that can catch a
    // same-tag content swap.
    let original = CustomThemeFile {
        name: "Original".to_string(),
        tokens: theme_tokens(ThemeId::Signex),
        canvas: canvas_colors(ThemeId::Signex),
    };
    app.ui_state.theme_id = ThemeId::Custom;
    app.ui_state.custom_theme = Some(original);
    let _ = app.update(Message::Preferences(PreferencesMsg::Open));
    assert!(
        !app.ui_state.preferences_dirty,
        "fixture precondition: opening must seed a clean dialog"
    );
    assert!(
        !app.ui_state.preferences_draft_differs(),
        "fixture precondition: nothing differs right after opening"
    );

    let _ = app.update(Message::Preferences(PreferencesMsg::Inner(
        PrefMsg::ThemeFileLoaded(custom_theme_json("Imported Replacement")),
    )));
    assert!(
        app.ui_state.preferences_dirty,
        "importing a theme must mark the dialog dirty even when the draft \
         theme ID was already Custom"
    );
    assert_eq!(app.ui_state.preferences_draft_theme, ThemeId::Custom);

    // An unrelated appearance-draft change the old bare comparator would see
    // as "nothing differs" (grid style set to its own current value).
    let current_grid_style = app.ui_state.preferences_draft_grid_style;
    let _ = app.update(Message::Preferences(PreferencesMsg::Inner(
        PrefMsg::DraftGridStyle(current_grid_style),
    )));

    assert!(
        app.ui_state.preferences_dirty,
        "an unrelated appearance-draft recompute must not clobber the \
         imported-theme dirty flag back to clean"
    );

    // And the close guard (in-app X / Esc) must still refuse to discard it.
    let _ = app.update(Message::Preferences(PreferencesMsg::Close));
    assert!(
        app.ui_state.preferences_open,
        "Close must refuse to dismiss while the import is still unsaved"
    );
    assert_eq!(
        app.ui_state.custom_theme.as_ref().map(|c| c.name.as_str()),
        Some("Imported Replacement"),
        "a refused Close must not revert the unsaved import either"
    );
}

/// Finding 1, keymap half: a pending keyboard-shortcut rebind (which never
/// touches the 7 appearance-draft fields at all) must also keep the dialog
/// dirty across an appearance toggle.
#[test]
fn keymap_rebind_dirty_flag_survives_an_unrelated_appearance_toggle() {
    let (mut app, _t) = Signex::new();
    app.ui_state.preferences_open = true;

    // Fork the active profile into an editable custom one — a real,
    // pending keymap-editor change the appearance comparator can't see.
    let _ = app.update(Message::Preferences(PreferencesMsg::Inner(
        PrefMsg::KeymapCreateCustomProfile,
    )));
    assert!(
        app.ui_state.preferences_dirty,
        "forking a custom keymap profile must mark the dialog dirty"
    );

    let current_grid_style = app.ui_state.preferences_draft_grid_style;
    let _ = app.update(Message::Preferences(PreferencesMsg::Inner(
        PrefMsg::DraftGridStyle(current_grid_style),
    )));

    assert!(
        app.ui_state.preferences_dirty,
        "an unrelated appearance-draft recompute must not clobber a pending \
         keymap-editor change back to clean"
    );
}

/// Finding 2 (data-loss): an OS close request (Alt+F4 / native ✕) on a
/// dirty, detached Preferences window must be refused rather than silently
/// closing (which would let unsaved edits vanish with no confirmation and
/// no way back). `Task::units()` is iced's own public count of the actions a
/// `Task` carries — `Task::none()` (refused) is 0 units, `window::close(id)`
/// (proceeding) is 1 — so this observes the *actual* returned command
/// without needing the iced runtime.
#[test]
fn os_close_request_is_refused_while_preferences_is_dirty() {
    let (mut app, _t) = Signex::new();

    // Real detach flow — synchronously registers the window entry
    // `handle_detach_modal` needs (see its own doc comment: "Stash the
    // mapping right away").
    let _ = app.update(Message::Preferences(PreferencesMsg::Open));
    let window_id = *app
        .ui_state
        .windows
        .keys()
        .next()
        .expect("Preferences Open must register a detached window");

    app.ui_state.preferences_dirty = true;
    app.ui_state.preferences_draft_theme = ThemeId::Custom;

    let task = app.update(Message::Window(WindowMsg::WindowCloseRequested(window_id)));

    assert_eq!(
        task.units(),
        0,
        "an OS close request on a dirty Preferences window must be refused \
         (Task::none()), not proceed to iced::window::close"
    );
    // Refusing to close must not itself touch the drafts either.
    assert_eq!(app.ui_state.preferences_draft_theme, ThemeId::Custom);
    assert!(app.ui_state.preferences_open);
}

/// Control for the test above: the same OS close request on a CLEAN
/// Preferences window must proceed (a real `window::close` command), so the
/// guard is a dirty-only gate, not a Preferences-window-never-closes bug.
#[test]
fn os_close_request_proceeds_when_preferences_is_clean() {
    let (mut app, _t) = Signex::new();

    let _ = app.update(Message::Preferences(PreferencesMsg::Open));
    let window_id = *app
        .ui_state
        .windows
        .keys()
        .next()
        .expect("Preferences Open must register a detached window");
    assert!(!app.ui_state.preferences_dirty, "freshly opened is clean");

    let task = app.update(Message::Window(WindowMsg::WindowCloseRequested(window_id)));

    assert_eq!(
        task.units(),
        1,
        "a clean Preferences window must be allowed to close normally"
    );
}
