//! #170 — the "Zoom speed" preference must reach every canvas.
//!
//! The wheel-zoom step used to be two hard-coded constants (1.1 on the
//! shared `Camera`, 1.15 on the footprint canvas). Now one Preferences
//! draft (`PrefMsg::DraftZoomStepPercent`) feeds the schematic, PCB,
//! symbol and footprint canvases, so these tests drive the real
//! Preferences message and then a real zoom on a canvas.
//!
//! The canvases zoom with the draft, the same effective-value rule
//! `grid_style` follows, so most tests here only draft. The Save and
//! startup tests at the bottom do write the per-process prefs path; they
//! take the shared lock (`serial()` + `PrefsPathGuard`, see
//! `render_config_grid_style.rs`), and the file round trip itself lives in
//! `prefs.rs`.

use signex_app::app::{Message, PreferencesMsg, Signex, SymbolDocument, TabInfo, TabKind};
use signex_app::library::messages::{LibraryMessage, PrimitiveEdit, SymbolEditorMsg};
use signex_app::preferences::PrefMsg;
use signex_library::{Symbol, SymbolFile};
use std::path::PathBuf;

fn inner(msg: PrefMsg) -> Message {
    Message::Preferences(PreferencesMsg::Inner(msg))
}

fn symbol_app() -> (Signex, PathBuf) {
    let mut app = app_on_default_speed();
    let path = PathBuf::from("zoom.snxsym");
    let file = SymbolFile::from_symbol(Symbol::empty("A"));
    app.document_state
        .symbol_editors
        .insert(path.clone(), SymbolDocument::new(path.clone(), file));
    app.document_state.tabs.push(TabInfo {
        title: "zoom".into(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::SymbolEditor(path.clone()),
    });
    app.document_state.active_tab = 0;
    (app, path)
}

/// A booted app on the default speed, whatever a concurrently running
/// Save/startup test has left in the shared prefs file at that instant.
fn app_on_default_speed() -> Signex {
    let (mut app, _t) = Signex::new();
    app.ui_state.zoom_step_percent = 10;
    app.ui_state.preferences_draft_zoom_step_percent = 10;
    app
}

fn wheel_in(app: &mut Signex, path: &PathBuf, delta: f32) -> f32 {
    let before = app.document_state.symbol_editors[path].camera.scale;
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Symbol(SymbolEditorMsg::Zoom {
            sx: 100.0,
            sy: 80.0,
            delta,
        }),
    }));
    app.document_state.symbol_editors[path].camera.scale / before
}

/// Without touching the preference a notch is still x1.10.
#[test]
fn the_default_notch_on_the_symbol_editor_is_unchanged() {
    // Arrange
    let (mut app, path) = symbol_app();
    let _ = app.update(Message::Preferences(PreferencesMsg::Open));

    // Act
    let ratio = wheel_in(&mut app, &path, 1.0);

    // Assert
    assert!(
        (ratio - 1.1).abs() < 1e-4,
        "default notch = x1.1, got {ratio}"
    );
}

#[test]
fn the_symbol_editor_zooms_by_the_configured_step() {
    // Arrange
    let (mut app, path) = symbol_app();
    let _ = app.update(Message::Preferences(PreferencesMsg::Open));

    // Act
    let _ = app.update(inner(PrefMsg::DraftZoomStepPercent(25)));
    let ratio = wheel_in(&mut app, &path, 1.0);

    // Assert
    assert!(
        (ratio - 1.25).abs() < 1e-4,
        "a 25% zoom speed must scale by 1.25 per notch, got {ratio}"
    );
}

#[test]
fn a_fractional_scroll_on_the_symbol_editor_is_proportional() {
    // Arrange
    let (mut app, path) = symbol_app();
    let _ = app.update(Message::Preferences(PreferencesMsg::Open));
    // 44% so half a notch (1.44^0.5 = 1.2) cannot coincide with a full
    // notch at the old fixed 1.1.
    let _ = app.update(inner(PrefMsg::DraftZoomStepPercent(44)));

    // Act
    let ratio = wheel_in(&mut app, &path, 0.5);

    // Assert
    assert!((ratio - 1.2).abs() < 1e-4, "half a notch, got {ratio}");
}

/// Discarding the dialog must put the zoom step back, or an abandoned
/// edit would keep driving every canvas.
#[test]
fn discarding_the_dialog_restores_the_saved_zoom_step() {
    // Arrange
    let (mut app, path) = symbol_app();
    let _ = app.update(Message::Preferences(PreferencesMsg::Open));
    let _ = app.update(inner(PrefMsg::DraftZoomStepPercent(40)));
    assert!(app.ui_state.preferences_dirty, "a moved slider is unsaved");

    // Act
    let _ = app.update(inner(PrefMsg::DiscardAndClose));
    let ratio = wheel_in(&mut app, &path, 1.0);

    // Assert
    assert!((ratio - 1.1).abs() < 1e-4, "back to x1.1, got {ratio}");
}

/// Out-of-range input from a draft message is clamped, never stored raw.
#[test]
fn an_out_of_range_draft_is_clamped() {
    let (mut app, _t) = Signex::new();
    let _ = app.update(Message::Preferences(PreferencesMsg::Open));

    let _ = app.update(inner(PrefMsg::DraftZoomStepPercent(200)));
    assert_eq!(app.ui_state.preferences_draft_zoom_step_percent, 50);
    let _ = app.update(inner(PrefMsg::DraftZoomStepPercent(0)));
    assert_eq!(app.ui_state.preferences_draft_zoom_step_percent, 5);
}

/// Save promotes the draft to the committed value, clears the unsaved
/// state and persists it. Runs under the shared prefs-path lock because
/// Save writes the real (redirected) prefs file.
#[test]
fn saving_commits_the_draft_and_clears_the_unsaved_state() {
    // Arrange
    let _serial = crate::preferences_prefs_recovery::serial();
    let _guard = crate::preferences_prefs_recovery::PrefsPathGuard::capture();
    let mut app = app_on_default_speed();
    let _ = app.update(Message::Preferences(PreferencesMsg::Open));
    let _ = app.update(inner(PrefMsg::DraftZoomStepPercent(33)));
    assert!(app.ui_state.preferences_dirty, "fixture: draft is unsaved");
    assert_eq!(app.ui_state.zoom_step_percent, 10, "fixture: not committed");

    // Act
    {
        let task = app.update(inner(PrefMsg::Save));
        crate::drive::drive(&mut app, task);
    }

    // Assert
    assert_eq!(app.ui_state.zoom_step_percent, 33);
    assert!(!app.ui_state.preferences_draft_differs());
    assert!(!app.ui_state.preferences_has_unsaved_changes());
    assert_eq!(signex_app::fonts::read_zoom_step_percent_pref(), 33);
}

/// Boot reads the saved speed once into both the committed value and the
/// draft, so the first canvas frame and the Preferences slider agree.
#[test]
fn startup_loads_the_saved_zoom_step_into_live_and_draft() {
    // Arrange
    let _serial = crate::preferences_prefs_recovery::serial();
    let _guard = crate::preferences_prefs_recovery::PrefsPathGuard::capture();
    signex_app::fonts::write_zoom_step_percent_pref(31);

    // Act
    let (app, _t) = Signex::new();

    // Assert
    assert_eq!(app.ui_state.zoom_step_percent, 31);
    assert_eq!(app.ui_state.preferences_draft_zoom_step_percent, 31);
}
