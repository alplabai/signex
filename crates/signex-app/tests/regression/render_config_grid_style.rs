//! #630 — the visible-grid style must reach the canvases as app state,
//! not through a process global.
//!
//! `render_config` used to keep a `OnceLock<RwLock<CanvasTextConfig>>`
//! that the schematic and symbol `draw` paths read directly. One global
//! write reached every window for free, which is exactly what made the
//! duplication invisible: nothing here could have failed, because there
//! was nothing per-canvas to get out of step. Now the value is a field on
//! each `CanvasSlot` plus `PanelContext::symbol_grid_style`, so the
//! sweep is code that can be wrong — these tests are what stops a second
//! window (or a Discard) from silently rendering a stale glyph.
//!
//! **No test here touches the filesystem** — same reason as
//! `preferences_symbol_drafts.rs`: `PrefMsg::Save` writes the per-process
//! prefs path guarded by another module's `Mutex`, and the regression
//! tests are one binary with no lock shared across modules.

use signex_app::app::{Message, PreferencesMsg, Signex, UiMsg};
use signex_app::preferences::PrefMsg;
use signex_app::render_config::GridStyle;

fn inner(msg: PrefMsg) -> Message {
    Message::Preferences(PreferencesMsg::Inner(msg))
}

/// Pick the variant the app is not currently showing, so each test asserts
/// on a real change rather than a no-op assignment that would also pass
/// against a handler that dropped the message entirely.
fn other_grid_style(current: GridStyle) -> GridStyle {
    match current {
        GridStyle::Dots => GridStyle::Lines,
        GridStyle::Lines | GridStyle::SmallCrosses => GridStyle::Dots,
    }
}

/// Boot has to leave the render input equal to the saved preference. A
/// draft that is never seeded stays on `GridStyle::Dots` forever — a user
/// whose `prefs.json` says `lines` would open to dots.
///
/// #631 — the canvas no longer holds a copy; `preferences_draft_grid_style`
/// IS the value `view` hands to the `Program` each frame, so that is what
/// has to be right at boot.
#[test]
fn boot_seeds_the_grid_style_render_input_from_the_saved_pref() {
    // Arrange / Act
    let (app, _t) = Signex::new();

    // Assert
    assert_eq!(
        app.ui_state.preferences_draft_grid_style, app.ui_state.grid_style,
        "the render input must start on the saved schematic grid style"
    );
    assert_eq!(
        app.document_state.panel_ctx.symbol_grid_style, app.ui_state.symbol_grid_style,
        "the symbol editor must start on the saved symbol grid style"
    );
}

/// Moving the picker previews immediately without committing — the same
/// draft contract every other setting under that footer follows.
#[test]
fn drafting_the_grid_style_previews_on_the_canvas_without_committing() {
    // Arrange
    let (mut app, _t) = Signex::new();
    let _ = app.update(Message::Preferences(PreferencesMsg::Open));
    let committed = app.ui_state.grid_style;
    let wanted = other_grid_style(committed);

    // Act
    let _ = app.update(inner(PrefMsg::DraftGridStyle(wanted)));

    // Assert
    assert_eq!(
        app.ui_state.preferences_draft_grid_style, wanted,
        "the preview must reach the render input, or the picker looks dead"
    );
    assert_eq!(
        app.ui_state.grid_style, committed,
        "a draft must not touch the committed value"
    );
    assert!(
        app.ui_state.preferences_dirty,
        "the Save/Discard footer must light up"
    );
}

/// The regression the global hid: Discard restores the committed style on
/// the canvas. With a per-canvas field, forgetting the push-back in
/// `revert_preferences_drafts` leaves the abandoned glyph rendering with
/// the picker showing the old value.
#[test]
fn discarding_puts_the_canvas_grid_style_back() {
    // Arrange
    let (mut app, _t) = Signex::new();
    let _ = app.update(Message::Preferences(PreferencesMsg::Open));
    let committed = app.ui_state.grid_style;
    let _ = app.update(inner(PrefMsg::DraftGridStyle(other_grid_style(committed))));

    // Act
    let _ = app.update(inner(PrefMsg::DiscardAndClose));

    // Assert
    assert_eq!(
        app.ui_state.preferences_draft_grid_style, committed,
        "Cancel must put the rendered grid style back"
    );
    assert_eq!(
        app.ui_state.preferences_draft_grid_style, committed,
        "Cancel must put the draft back too"
    );
}

/// #631 — an undocked window used to render from its own `CanvasSlot`
/// copy of the grid style, so a preview only reached it if the write swept
/// the whole `canvases` map. There is no copy any more: every window's
/// `Program` is built in `view` from the one `UiState` value, so a second
/// window cannot disagree with the first by construction. This test pins
/// that no per-window grid-style copy comes back.
#[test]
fn no_per_window_copy_of_the_grid_style_exists() {
    // Arrange
    let (mut app, _t) = Signex::new();
    let undocked_id = iced::window::Id::unique();
    app.interaction_state
        .canvases
        .insert(undocked_id, signex_app::canvas::CanvasSlot::new());
    let _ = app.update(Message::Preferences(PreferencesMsg::Open));
    let wanted = other_grid_style(app.ui_state.grid_style);

    // Act
    let _ = app.update(inner(PrefMsg::DraftGridStyle(wanted)));

    // Assert — one value, read by every window.
    assert_eq!(
        app.ui_state.preferences_draft_grid_style, wanted,
        "the single render input must carry the preview"
    );

    // Act — and back again on Discard.
    let _ = app.update(inner(PrefMsg::DiscardAndClose));

    // Assert
    assert_eq!(
        app.ui_state.preferences_draft_grid_style, app.ui_state.grid_style,
        "Cancel must restore the one value every window reads"
    );
}

/// The Symbol Editor's grid style previews through `panel_ctx`, and
/// `refresh_panel_ctx` rebuilds that struct from scratch on all sorts of
/// unrelated messages. If the rebuild re-seeded from `ui_state`, an
/// in-flight preview would snap back mid-dialog.
#[test]
fn a_panel_ctx_rebuild_keeps_the_symbol_grid_preview() {
    // Arrange
    let (mut app, _t) = Signex::new();
    let _ = app.update(Message::Preferences(PreferencesMsg::Open));
    let wanted = other_grid_style(app.ui_state.symbol_grid_style);
    let _ = app.update(inner(PrefMsg::DraftSymbolGridStyle(wanted)));

    // Act — `GridPickerSelect` calls `refresh_panel_ctx` unconditionally,
    // with or without a footprint editor open, so it is the cheapest
    // public route to a full panel_ctx rebuild.
    let _ = app.update(Message::Ui(UiMsg::GridPickerSelect(1.27)));

    // Assert
    assert_eq!(
        app.document_state.panel_ctx.symbol_grid_style, wanted,
        "a panel_ctx rebuild must not drop the live preview"
    );
}
