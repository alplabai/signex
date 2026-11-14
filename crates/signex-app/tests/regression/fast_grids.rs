//! #1672 - `fast_grid_1`, `fast_grid_2` and `cycle_fast_grid` set the snap
//! grid instead of reporting "not available yet". The two grids are a
//! Preferences setting (defaults 1.27 and 2.54 mm).
//!
//! The Save and startup tests write the per-process prefs path; they take
//! the shared lock (`serial()` + `PrefsPathGuard`, see
//! `preferences_prefs_recovery.rs`).

use signex_app::app::{Message, PreferencesMsg, Signex};
use signex_app::canvas::grid::FAST_GRIDS_MM;
use signex_app::preferences::PrefMsg;

use crate::menu_support::menu_row;

fn inner(msg: PrefMsg) -> Message {
    Message::Preferences(PreferencesMsg::Inner(msg))
}

/// A booted app on `mm` with the default fast grids, whatever a concurrent
/// Save test has left in the shared prefs file at that instant.
fn app_on_grid(mm: f32) -> Signex {
    let (mut app, _t) = Signex::new();
    app.ui_state.grid_size_mm = mm;
    app.ui_state.visible_grid_mm = mm;
    app.ui_state.fast_grids_mm = FAST_GRIDS_MM;
    app.ui_state.preferences_draft_fast_grids = ["1.27".to_string(), "2.54".to_string()];
    app
}

/// Draft `texts` into the two fast-grid fields and Save.
fn save_fast_grids(app: &mut Signex, texts: [&str; 2]) {
    let _ = app.update(Message::Preferences(PreferencesMsg::Open));
    for (index, text) in texts.into_iter().enumerate() {
        let _ = app.update(inner(PrefMsg::DraftFastGrid(index, text.to_string())));
    }
    let task = app.update(inner(PrefMsg::SaveAndClose));
    crate::drive::drive(app, task);
}

#[test]
fn changing_fast_grid_1_in_preferences_makes_fast_grid_1_snap_to_it() {
    // Arrange
    let _serial = crate::preferences_prefs_recovery::serial();
    let _guard = crate::preferences_prefs_recovery::PrefsPathGuard::capture();
    let mut app = app_on_grid(5.08);
    save_fast_grids(&mut app, ["0.635", "2.54"]);
    assert_eq!(app.ui_state.fast_grids_mm, [0.635, FAST_GRIDS_MM[1]]);
    assert!(!app.ui_state.preferences_has_unsaved_changes());

    // Act
    let _ = app.update(menu_row("fast_grid_1"));

    // Assert
    assert_eq!(app.ui_state.grid_size_mm, 0.635);
    assert_eq!(app.ui_state.visible_grid_mm, 0.635);
    let (rebooted, _t) = Signex::new();
    assert_eq!(
        rebooted.ui_state.fast_grids_mm,
        [0.635, FAST_GRIDS_MM[1]],
        "the saved fast grids survive a restart"
    );
}

#[test]
fn cycle_fast_grid_steps_between_the_grids_set_in_preferences() {
    let _serial = crate::preferences_prefs_recovery::serial();
    let _guard = crate::preferences_prefs_recovery::PrefsPathGuard::capture();
    let mut app = app_on_grid(1.27);
    save_fast_grids(&mut app, ["0.5", "5"]);

    let _ = app.update(menu_row("cycle_fast_grid"));
    assert_eq!(app.ui_state.grid_size_mm, 0.5, "1.27 is no fast grid now");
    let _ = app.update(menu_row("cycle_fast_grid"));
    assert_eq!(app.ui_state.grid_size_mm, 5.0);
    let _ = app.update(menu_row("fast_grid_2"));
    assert_eq!(app.ui_state.grid_size_mm, 5.0);
}

#[test]
fn a_fast_grid_that_is_not_a_finite_number_above_zero_is_refused_on_save() {
    let _serial = crate::preferences_prefs_recovery::serial();
    let _guard = crate::preferences_prefs_recovery::PrefsPathGuard::capture();
    for bad in ["0", "-2.54", "nan", "inf", "1e999", "2.54mm"] {
        let mut app = app_on_grid(5.08);
        save_fast_grids(&mut app, ["1.27", bad]);

        assert_eq!(app.ui_state.fast_grids_mm, FAST_GRIDS_MM, "{bad:?}");
        assert_eq!(app.ui_state.preferences_draft_fast_grids[1], "2.54");
        assert!(!app.ui_state.preferences_has_unsaved_changes(), "{bad:?}");
        let _ = app.update(menu_row("fast_grid_2"));
        assert_eq!(app.ui_state.grid_size_mm, FAST_GRIDS_MM[1], "{bad:?}");
    }
}

#[test]
fn a_fast_grid_draft_marks_preferences_dirty_and_discard_restores_it() {
    let mut app = app_on_grid(5.08);
    let _ = app.update(Message::Preferences(PreferencesMsg::Open));

    let _ = app.update(inner(PrefMsg::DraftFastGrid(0, "0.635".to_string())));
    assert!(app.ui_state.preferences_dirty);
    assert_eq!(app.ui_state.fast_grids_mm, FAST_GRIDS_MM, "not committed");

    let _ = app.update(inner(PrefMsg::DiscardAndClose));
    assert_eq!(app.ui_state.preferences_draft_fast_grids[0], "1.27");
    let _ = app.update(menu_row("fast_grid_1"));
    assert_eq!(app.ui_state.grid_size_mm, FAST_GRIDS_MM[0]);
}

#[test]
fn prefs_saved_before_the_fast_grid_setting_load_the_defaults() {
    // Arrange: a prefs file from before #1672, with no fast-grid keys.
    let _serial = crate::preferences_prefs_recovery::serial();
    let _guard = crate::preferences_prefs_recovery::PrefsPathGuard::capture();
    let path = signex_app::fonts::prefs_file_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create the shared config directory");
    }
    std::fs::write(&path, br#"{"grid_size_mm": 2.54, "zoom_step_percent": 20}"#)
        .expect("seed an old prefs file");

    // Act
    let (app, _t) = Signex::new();

    // Assert
    assert_eq!(app.ui_state.fast_grids_mm, FAST_GRIDS_MM);
    assert_eq!(
        app.ui_state.preferences_draft_fast_grids,
        ["1.27".to_string(), "2.54".to_string()]
    );
    assert_eq!(
        app.ui_state.zoom_step_percent, 20,
        "fixture: the file was read"
    );
}

#[test]
fn fast_grid_1_and_2_snap_to_their_grids() {
    let mut app = app_on_grid(5.08);

    let _ = app.update(menu_row("fast_grid_1"));
    assert_eq!(app.ui_state.grid_size_mm, FAST_GRIDS_MM[0]);
    assert_eq!(app.ui_state.visible_grid_mm, FAST_GRIDS_MM[0]);

    let _ = app.update(menu_row("fast_grid_2"));
    assert_eq!(app.ui_state.grid_size_mm, FAST_GRIDS_MM[1]);
    assert_eq!(app.ui_state.visible_grid_mm, FAST_GRIDS_MM[1]);
}

#[test]
fn cycle_fast_grid_steps_between_the_two_and_wraps() {
    let mut app = app_on_grid(FAST_GRIDS_MM[0]);

    let _ = app.update(menu_row("cycle_fast_grid"));
    assert_eq!(app.ui_state.grid_size_mm, FAST_GRIDS_MM[1]);

    let _ = app.update(menu_row("cycle_fast_grid"));
    assert_eq!(app.ui_state.grid_size_mm, FAST_GRIDS_MM[0]);
}

#[test]
fn cycle_fast_grid_from_another_grid_lands_on_fast_grid_1() {
    let mut app = app_on_grid(0.635);

    let _ = app.update(menu_row("cycle_fast_grid"));
    assert_eq!(app.ui_state.grid_size_mm, FAST_GRIDS_MM[0]);
}

#[test]
fn a_fast_grid_leaves_a_deliberately_different_visible_grid_alone() {
    let mut app = app_on_grid(0.635);
    app.ui_state.visible_grid_mm = 5.08;

    let _ = app.update(menu_row("fast_grid_2"));
    assert_eq!(app.ui_state.grid_size_mm, FAST_GRIDS_MM[1]);
    assert_eq!(app.ui_state.visible_grid_mm, 5.08);
}
