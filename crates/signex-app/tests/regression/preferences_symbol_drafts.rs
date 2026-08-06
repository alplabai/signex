//! #629 — the three Symbol Editor appearance settings must behave like
//! drafts, because that is what the pane they live in promises.
//!
//! They used to write `prefs.json` the moment the picker moved, while
//! every other setting under the same Save/Cancel footer waited for Save.
//! Worse, they had no committed `UiState` field at all — the
//! `preferences_draft_*` field WAS the saved value, seeded from disk at
//! boot — so `revert_preferences_drafts` had nothing to restore from and
//! Cancel silently kept the change.
//!
//! **No test here touches the filesystem.** `PrefMsg::Save` writes the
//! per-process prefs path that `preferences_prefs_recovery.rs` guards with
//! its own module `Mutex`, and the regression tests are one binary with no
//! shared lock between modules (see `tests/regression.rs`), so driving
//! Save from here would race that guard. The Save arm's three new commit
//! lines are therefore NOT covered by a test — they sit directly beneath
//! the four identical lines for `power_port_style`, `label_style`,
//! `multisheet_style` and `grid_style`. Closing that gap needs a lock
//! shared across modules first.

use signex_app::app::{Message, PreferencesMsg, Signex};
use signex_app::preferences::PrefMsg;
use signex_app::render_config::{GridStyle, PinSelectionMode};

fn inner(msg: PrefMsg) -> Message {
    Message::Preferences(PreferencesMsg::Inner(msg))
}

/// Pick the variant the app is not currently showing, so the test asserts
/// on a real change rather than a no-op assignment that would pass against
/// a handler that dropped the message entirely.
fn other_grid_style(current: GridStyle) -> GridStyle {
    match current {
        GridStyle::Dots => GridStyle::Lines,
        GridStyle::Lines | GridStyle::SmallCrosses => GridStyle::Dots,
    }
}

fn other_pin_selection(current: PinSelectionMode) -> PinSelectionMode {
    match current {
        PinSelectionMode::PinOnly => PinSelectionMode::TextAndPin,
        PinSelectionMode::TextAndPin => PinSelectionMode::PinOnly,
    }
}

/// A draft is a draft: moving the picker must leave the committed value
/// alone and light up the Save/Discard footer. Before the fix the handler
/// wrote straight to `prefs.json` and never called
/// `recompute_preferences_dirty`, so the footer stayed dark while the
/// change was already permanent.
#[test]
fn changing_a_symbol_setting_marks_the_dialog_dirty_without_committing() {
    // Arrange
    let (mut app, _t) = Signex::new();
    let _ = app.update(Message::Preferences(PreferencesMsg::Open));
    let committed = app.ui_state.symbol_grid_style;
    let wanted = other_grid_style(committed);

    // Act
    let _ = app.update(inner(PrefMsg::DraftSymbolGridStyle(wanted)));

    // Assert
    assert_eq!(
        app.ui_state.preferences_draft_symbol_grid_style, wanted,
        "the picker must move the draft"
    );
    assert_eq!(
        app.ui_state.symbol_grid_style, committed,
        "an unsaved draft must not touch the committed value"
    );
    assert!(
        app.ui_state.preferences_dirty,
        "an unsaved draft must light up the Save/Discard footer"
    );
}

/// The bug as the user met it: change the Symbol Editor grid style, press
/// Cancel, and the change survived. `revert_preferences_drafts` restored
/// the four schematic appearance settings and skipped these three.
#[test]
fn discarding_puts_all_three_symbol_drafts_back() {
    // Arrange
    let (mut app, _t) = Signex::new();
    let _ = app.update(Message::Preferences(PreferencesMsg::Open));
    let committed_style = app.ui_state.symbol_grid_style;
    let committed_size = app.ui_state.symbol_grid_size_mm;
    let committed_pins = app.ui_state.symbol_pin_selection;
    let _ = app.update(inner(PrefMsg::DraftSymbolGridStyle(other_grid_style(
        committed_style,
    ))));
    let _ = app.update(inner(PrefMsg::DraftSymbolGridSize(committed_size + 1.0)));
    let _ = app.update(inner(PrefMsg::DraftSymbolPinSelection(
        other_pin_selection(committed_pins),
    )));
    assert!(
        app.ui_state.preferences_dirty,
        "fixture precondition: three changed drafts must read as dirty"
    );

    // Act
    let _ = app.update(inner(PrefMsg::DiscardAndClose));

    // Assert
    assert_eq!(
        app.ui_state.preferences_draft_symbol_grid_style, committed_style,
        "Cancel must put the grid style back"
    );
    assert_eq!(
        app.ui_state.preferences_draft_symbol_grid_size_mm, committed_size,
        "Cancel must put the grid size back"
    );
    assert_eq!(
        app.ui_state.preferences_draft_symbol_pin_selection, committed_pins,
        "Cancel must put the pin-selection mode back"
    );
    assert!(
        !app.ui_state.preferences_dirty,
        "nothing is pending once the drafts are back"
    );
    assert_eq!(
        signex_app::render_config::symbol_grid_style(),
        committed_style,
        "Cancel must also put the live-preview global back, or the editor \
         keeps rendering the discarded style"
    );
}

/// `UiState::preferences_draft_differs` is the single predicate behind the
/// footer and every dirty-close guard, and its doc comment claims to cover
/// ALL draft state. Each of the three has to be in it individually — a
/// predicate that happens to catch one of them would let the other two be
/// lost on close.
#[test]
fn the_dirty_predicate_covers_each_symbol_setting_on_its_own() {
    /// Open a fresh dialog, move exactly one draft, and require the
    /// predicate to notice. One setting per call so a predicate that
    /// happens to catch a different term cannot carry this one.
    fn assert_dirty_on(label: &str, mutate: impl FnOnce(&mut Signex)) {
        // Arrange
        let (mut app, _t) = Signex::new();
        let _ = app.update(Message::Preferences(PreferencesMsg::Open));
        assert!(
            !app.ui_state.preferences_draft_differs(),
            "fixture precondition: a freshly seeded dialog is clean ({label})"
        );

        // Act
        mutate(&mut app);

        // Assert
        assert!(
            app.ui_state.preferences_draft_differs(),
            "an unsaved {label} change must read as dirty on its own"
        );
    }

    assert_dirty_on("grid style", |app| {
        app.ui_state.preferences_draft_symbol_grid_style =
            other_grid_style(app.ui_state.symbol_grid_style);
    });
    assert_dirty_on("grid size", |app| {
        app.ui_state.preferences_draft_symbol_grid_size_mm = app.ui_state.symbol_grid_size_mm + 1.0;
    });
    assert_dirty_on("pin selection", |app| {
        app.ui_state.preferences_draft_symbol_pin_selection =
            other_pin_selection(app.ui_state.symbol_pin_selection);
    });
}
