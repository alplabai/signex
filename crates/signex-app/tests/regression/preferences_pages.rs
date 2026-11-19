//! #144 / #147 - the Preferences Appearance and Editing pages. A fast grid
//! field shows why Save would refuse it while the user types, and Save
//! still refuses it (DECISIONS 2026-11-14). A pitch set on the snap
//! preview fills its field through the field's own draft. The Increment
//! step stepper skips 0 and keeps to its range.
//!
//! The Save test writes the per-process prefs path; it takes the shared
//! lock (`serial()` + `PrefsPathGuard`, see `preferences_prefs_recovery.rs`).

use iced::Point;
use signex_app::app::{Message, PreferencesMsg, Signex};
use signex_app::canvas::grid::FAST_GRIDS_MM;
use signex_app::preferences::{PagesViewMsg, PrefMsg};
use signex_widgets::dimension_label::DimensionLabelMessage as Label;

fn inner(msg: PrefMsg) -> Message {
    Message::Preferences(PreferencesMsg::Inner(msg))
}

fn page(msg: PagesViewMsg) -> Message {
    inner(PrefMsg::PagesView(msg))
}

fn pitch(msg: Label) -> Message {
    page(PagesViewMsg::SnapLabel(msg))
}

/// Preferences open on the default fast grids, whatever a concurrent Save
/// test has left in the shared prefs file.
fn open_preferences() -> Signex {
    let (mut app, _t) = Signex::new();
    app.ui_state.fast_grids_mm = FAST_GRIDS_MM;
    let _ = app.update(Message::Preferences(PreferencesMsg::Open));
    assert_eq!(
        app.ui_state.preferences_draft_fast_grids,
        ["1.27".to_string(), "2.54".to_string()],
        "fixture: the fields show the default grids"
    );
    app
}

#[test]
fn a_fast_grid_shows_its_error_while_typing_and_save_still_refuses_it() {
    let _serial = crate::preferences_prefs_recovery::serial();
    let _guard = crate::preferences_prefs_recovery::PrefsPathGuard::capture();
    let mut app = open_preferences();

    let _ = app.update(inner(PrefMsg::DraftFastGrid(1, "0.0000001".to_string())));

    assert_eq!(app.ui_state.fast_grid_field_error(0), None);
    assert_eq!(
        app.ui_state.fast_grid_field_error(1).as_deref(),
        Some("0.0000001 mm is finer than the 1 nm (0.000001 mm) minimum"),
        "the row error shows while typing"
    );
    assert!(
        app.ui_state.preferences_fast_grid_status.is_empty(),
        "no refusal line before Save"
    );

    let task = app.update(inner(PrefMsg::SaveAndClose));
    crate::drive::drive(&mut app, task);

    assert!(app.ui_state.preferences_open, "the dialog stays open");
    assert_eq!(
        app.ui_state.fast_grids_mm, FAST_GRIDS_MM,
        "nothing committed"
    );
    let status = &app.ui_state.preferences_fast_grid_status;
    assert!(
        status.contains("not saved")
            && status.contains("Fast grid 2")
            && status.contains("minimum"),
        "{status:?}"
    );

    let _ = app.update(inner(PrefMsg::DraftFastGrid(1, "2.54".to_string())));
    assert_eq!(
        app.ui_state.fast_grid_field_error(1),
        None,
        "fixing the field clears the row error"
    );
}

#[test]
fn a_pitch_typed_on_the_snap_preview_fills_its_field() {
    let mut app = open_preferences();

    let _ = app.update(pitch(Label::LabelPressed(0)));
    assert_eq!(
        app.ui_state
            .preferences_pages
            .pages_view
            .snap_label_editing(),
        Some(0)
    );
    let _ = app.update(pitch(Label::DraftEdited("0.635".to_string())));
    let _ = app.update(pitch(Label::DraftSubmitted));

    assert_eq!(app.ui_state.preferences_draft_fast_grids[0], "0.635");
    assert_eq!(
        app.ui_state
            .preferences_pages
            .pages_view
            .snap_label_editing(),
        None
    );
    assert!(
        app.ui_state.preferences_dirty,
        "a draft like any field edit"
    );
    assert_eq!(
        app.ui_state.fast_grids_mm, FAST_GRIDS_MM,
        "not committed before Save"
    );
}

#[test]
fn tab_commits_a_pitch_and_opens_the_other_and_esc_cancels() {
    let mut app = open_preferences();

    let _ = app.update(pitch(Label::LabelPressed(0)));
    let _ = app.update(pitch(Label::DraftEdited("0.5".to_string())));
    let _ = app.update(pitch(Label::TabPressed { backward: false }));

    assert_eq!(app.ui_state.preferences_draft_fast_grids[0], "0.5");
    assert_eq!(
        app.ui_state
            .preferences_pages
            .pages_view
            .snap_label_editing(),
        Some(1)
    );

    let _ = app.update(pitch(Label::DraftEdited("9".to_string())));
    let _ = app.update(pitch(Label::EscapePressed));

    assert_eq!(app.ui_state.preferences_draft_fast_grids[1], "2.54");
    assert_eq!(
        app.ui_state
            .preferences_pages
            .pages_view
            .snap_label_editing(),
        None
    );
}

#[test]
fn a_refused_pitch_can_be_fixed_on_the_drawing() {
    let mut app = open_preferences();
    let _ = app.update(inner(PrefMsg::DraftFastGrid(1, "abc".to_string())));
    assert!(app.ui_state.fast_grid_field_error(1).is_some());

    let _ = app.update(pitch(Label::LabelPressed(1)));
    let _ = app.update(pitch(Label::DraftEdited("5.08".to_string())));
    let _ = app.update(pitch(Label::DraftSubmitted));

    assert_eq!(app.ui_state.preferences_draft_fast_grids[1], "5.08");
    assert_eq!(app.ui_state.fast_grid_field_error(1), None);
}

#[test]
fn dragging_a_pitch_handle_moves_its_field() {
    let mut app = open_preferences();

    let _ = app.update(pitch(Label::HandlePressed(0)));
    // The first move sets where the drag measures from; 24 px is 1 mm.
    let _ = app.update(pitch(Label::PointerMoved(Point::new(100.0, 30.0))));
    let _ = app.update(pitch(Label::PointerMoved(Point::new(124.0, 30.0))));
    let _ = app.update(pitch(Label::PointerReleased));

    assert_eq!(app.ui_state.preferences_draft_fast_grids[0], "2.27");
    assert_eq!(app.ui_state.preferences_draft_fast_grids[1], "2.54");
}

#[test]
fn the_increment_stepper_skips_zero_and_keeps_to_its_range() {
    let mut app = open_preferences();
    let step = |app: &Signex| app.ui_state.preferences_draft_increment_step;

    let _ = app.update(inner(PrefMsg::DraftIncrementStep(1)));
    let _ = app.update(page(PagesViewMsg::IncrementStepNudged(-1)));
    assert_eq!(step(&app), -1, "0 is skipped going down");
    let _ = app.update(page(PagesViewMsg::IncrementStepNudged(1)));
    assert_eq!(step(&app), 1, "and going up");

    let _ = app.update(inner(PrefMsg::DraftIncrementStep(100)));
    let _ = app.update(page(PagesViewMsg::IncrementStepNudged(1)));
    assert_eq!(step(&app), 100);
    let _ = app.update(inner(PrefMsg::DraftIncrementStep(-100)));
    let _ = app.update(page(PagesViewMsg::IncrementStepNudged(-1)));
    assert_eq!(step(&app), -100);
}
