//! #2004 item 7 — a placed part remembers the row values it last agreed with
//! (`library_stamp`), so a deliberate override of footprint, Manufacturer or
//! MPN survives the row's next version, and a field both sides moved is the
//! user's to settle.

use signex_app::app::{EditMsg, Message, Signex};
use signex_app::library::LibraryMessage;
use signex_app::library::part_sync::Choice;
use signex_library::adapter::LibraryAdapter;
use signex_library::adapters::local_git::LocalGitAdapter;
use signex_library::component::ComponentRow;
use signex_library::primitive::PrimitiveRef;
use signex_types::schematic::{RowStamp, StampField};
use uuid::Uuid;

use crate::library_place_symbol::{Fixture, app_with_library, doc, fixture, place};
use crate::library_symbol_drift::{check_drift, click_update, is_drifted, run_task, select_first};

/// Edit the fixture's row as another editor would and move its version.
fn row_update(f: &Fixture, version: &str, edit: impl FnOnce(&mut ComponentRow)) {
    let adapter = LocalGitAdapter::open(&f.snxlib).expect("open");
    let mut row = adapter
        .read_row("ICs", signex_library::RowId(f.row_id))
        .expect("row");
    edit(&mut row);
    row.version = version.into();
    adapter.update_row("ICs", row, "edit").expect("update row");
}

fn stamp(app: &Signex) -> RowStamp {
    doc(app).symbols[0]
        .library_stamp
        .clone()
        .expect("a stamp is written")
}

fn mpn(app: &Signex) -> Option<String> {
    doc(app).symbols[0].fields.get("MPN").cloned()
}

fn run(app: &mut Signex, command: signex_engine::Command) {
    app.document_state
        .active_engine_mut()
        .unwrap()
        .execute(command)
        .unwrap();
}

fn set_mpn(app: &mut Signex, value: &str) {
    let symbol_id = doc(app).symbols[0].uuid;
    run(
        app,
        signex_engine::Command::SetSymbolField {
            symbol_id,
            key: "MPN".into(),
            value: value.into(),
        },
    );
}

/// What a part placed before the stamp existed looks like.
fn drop_stamp(app: &mut Signex) {
    let engine = app.document_state.active_engine_mut().unwrap();
    let mut sheet = engine.document().clone();
    sheet.symbols[0].library_stamp = None;
    engine.set_document(sheet);
}

fn update_first(app: &mut Signex) {
    let id = doc(app).symbols[0].uuid;
    click_update(app, id);
}

/// The sheet as right after it is opened: a fresh engine, no undo history,
/// so the open-time scan may restamp silently.
fn reopen(app: &mut Signex) {
    let path = app.document_state.active_path.clone().unwrap();
    let document = doc(app).clone();
    app.document_state.engines.insert(
        path.clone(),
        signex_engine::Engine::new_with_path(document, Some(path)).unwrap(),
    );
}

fn scan(app: &mut Signex) {
    let sheet = app.document_state.active_path.clone().unwrap();
    app.scan_library_updates_for_open_schematic(sheet);
}

fn placed() -> (Fixture, Signex) {
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    select_first(&mut app);
    (f, app)
}

fn logged(part: &str) -> bool {
    signex_app::diagnostics::recent_entries()
        .iter()
        .any(|e| e.message.contains(part))
}

/// Answer the open modal's conflicts with `choice`, select everything and
/// click Apply.
fn apply_modal(app: &mut Signex, choice: Option<Choice>) {
    let state = app.library.library_updates.as_mut().expect("a modal");
    let answers: Vec<_> = state.entries.iter().map(|e| e.symbol_uuid).collect();
    for e in &mut state.entries {
        e.selected = true;
    }
    if let Some(choice) = choice {
        for symbol_uuid in answers {
            let _ = app.update(Message::Library(
                LibraryMessage::LibraryUpdatesConflictAnswered {
                    symbol_uuid,
                    field: StampField::Mpn,
                    choice,
                },
            ));
        }
    }
    let task = app.update(Message::Library(LibraryMessage::LibraryUpdatesApply));
    for message in run_task(task) {
        let _ = app.update(message);
    }
}

#[test]
fn placement_stamps_the_row_values_with_the_version() {
    let (f, app) = placed();
    let s = stamp(&app);
    assert_eq!(s.row_id, f.row_id);
    assert_eq!(s.mpn.as_deref(), Some("TWO-1"));
    assert_eq!(s.manufacturer.as_deref(), Some("Acme"));
    assert_eq!(s.footprint.as_deref(), Some(""), "the row binds none");
    assert_eq!(doc(&app).symbols[0].library_version, "1.2.3");
}

#[test]
fn an_override_is_kept_across_a_version_bump() {
    let (f, mut app) = placed();
    set_mpn(&mut app, "MY-MPN");
    row_update(&f, "1.2.4", |row| row.description = "reworded".into());
    reopen(&mut app);
    scan(&mut app);
    check_drift(&mut app);
    assert!(!is_drifted(&app), "the row's MPN did not move");
    assert_eq!(mpn(&app).as_deref(), Some("MY-MPN"));
    assert_eq!(doc(&app).symbols[0].library_version, "1.2.4", "restamped");
    assert_eq!(stamp(&app).mpn.as_deref(), Some("TWO-1"), "the base stays");
}

#[test]
fn a_field_the_user_did_not_touch_follows_the_row_and_the_base_advances() {
    let (f, mut app) = placed();
    row_update(&f, "1.2.4", |row| row.primary_mpn.mpn = "NEW-1".into());
    check_drift(&mut app);
    assert!(is_drifted(&app));
    update_first(&mut app);
    assert_eq!(mpn(&app).as_deref(), Some("NEW-1"));
    assert_eq!(
        stamp(&app).mpn.as_deref(),
        Some("NEW-1"),
        "the base advanced"
    );
    row_update(&f, "1.2.5", |row| row.primary_mpn.mpn = "NEW-2".into());
    check_drift(&mut app);
    assert!(is_drifted(&app));
    update_first(&mut app);
    assert_eq!(
        mpn(&app).as_deref(),
        Some("NEW-2"),
        "no conflict with the old base"
    );
}

#[test]
fn an_override_and_a_row_move_is_flagged_and_an_update_keeps_the_part() {
    let _ = signex_app::diagnostics::init_logging();
    let (f, mut app) = placed();
    set_mpn(&mut app, "MINE");
    row_update(&f, "1.2.4", |row| row.primary_mpn.mpn = "LIB".into());
    check_drift(&mut app);
    assert!(is_drifted(&app), "both moved");
    update_first(&mut app);
    assert_eq!(mpn(&app).as_deref(), Some("MINE"), "never overwritten");
    assert!(logged("kept your MPN \"MINE\"; library now \"LIB\""));
    assert_eq!(
        stamp(&app).mpn.as_deref(),
        Some("TWO-1"),
        "update settles nothing"
    );
}

#[test]
fn use_library_value_applies_the_conflict_in_one_undo_step() {
    let (f, mut app) = placed();
    set_mpn(&mut app, "MINE");
    row_update(&f, "1.2.4", |row| row.primary_mpn.mpn = "LIB".into());
    scan(&mut app);
    let entry = &app
        .library
        .library_updates
        .as_ref()
        .expect("the modal")
        .entries[0];
    assert_eq!(entry.conflicts.len(), 1);
    assert_eq!(entry.conflicts[0].choice, Choice::KeepMine, "the default");
    apply_modal(&mut app, Some(Choice::UseLibrary));
    assert_eq!(mpn(&app).as_deref(), Some("LIB"));
    assert_eq!(stamp(&app).mpn.as_deref(), Some("LIB"));
    assert_eq!(doc(&app).symbols[0].library_version, "1.2.4");
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(mpn(&app).as_deref(), Some("MINE"), "one undo step");
    assert_eq!(stamp(&app).mpn.as_deref(), Some("TWO-1"));
    assert_eq!(doc(&app).symbols[0].library_version, "1.2.3");
}

#[test]
fn keep_mine_silences_the_flag_until_the_row_moves_again() {
    let (f, mut app) = placed();
    set_mpn(&mut app, "MINE");
    row_update(&f, "1.2.4", |row| row.primary_mpn.mpn = "LIB".into());
    scan(&mut app);
    apply_modal(&mut app, None);
    assert_eq!(mpn(&app).as_deref(), Some("MINE"));
    assert_eq!(stamp(&app).mpn.as_deref(), Some("LIB"), "acknowledged");
    check_drift(&mut app);
    assert!(!is_drifted(&app));
    row_update(&f, "1.2.5", |row| row.primary_mpn.mpn = "LIB-2".into());
    check_drift(&mut app);
    assert!(is_drifted(&app), "the row moved again");
}

#[test]
fn retyping_the_rows_new_value_raises_no_flag() {
    let (f, mut app) = placed();
    row_update(&f, "1.2.4", |row| row.primary_mpn.mpn = "NEW".into());
    set_mpn(&mut app, "NEW");
    check_drift(&mut app);
    assert!(!is_drifted(&app));
}

#[test]
fn a_cleared_field_stays_empty() {
    let (f, mut app) = placed();
    let symbol_id = doc(&app).symbols[0].uuid;
    run(
        &mut app,
        signex_engine::Command::RemoveSymbolField {
            symbol_id,
            key: "MPN".into(),
        },
    );
    row_update(&f, "1.2.4", |row| row.description = "reworded".into());
    reopen(&mut app);
    scan(&mut app);
    check_drift(&mut app);
    assert!(!is_drifted(&app), "the row's MPN did not move");
    assert_eq!(mpn(&app), None, "never refilled");
}

#[test]
fn an_unreadable_footprint_library_raises_no_flag_and_writes_no_empty_base() {
    let f = fixture();
    // The row binds a footprint of a library that is not open.
    row_update(&f, "1.2.3", |row| {
        row.footprint_ref = Some(PrimitiveRef::new(Uuid::now_v7(), Uuid::now_v7()));
    });
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    select_first(&mut app);
    assert_eq!(stamp(&app).footprint, None, "unknown, not empty");
    check_drift(&mut app);
    assert!(!is_drifted(&app));
    // A later restamp, with the library still unreadable, must not claim
    // that the row offered no footprint.
    row_update(&f, "1.2.4", |row| row.description = "reworded".into());
    reopen(&mut app);
    scan(&mut app);
    assert_eq!(doc(&app).symbols[0].library_version, "1.2.4", "restamped");
    assert_eq!(stamp(&app).footprint, None, "still unknown");
}

#[test]
fn the_personal_restamp_keeps_the_base_of_a_pending_follow() {
    let (f, mut app) = placed();
    row_update(&f, "1.2.4", |row| row.primary_mpn.mpn = "NEW".into());
    reopen(&mut app);
    scan(&mut app);
    assert_eq!(
        doc(&app).symbols[0].library_version,
        "1.2.3",
        "not restamped"
    );
    assert_eq!(stamp(&app).mpn.as_deref(), Some("TWO-1"), "base unchanged");
    check_drift(&mut app);
    assert!(is_drifted(&app), "the follow is still pending");
}

#[test]
fn an_old_part_with_the_version_moved_and_a_differing_mpn_is_a_conflict() {
    let (f, mut app) = placed();
    drop_stamp(&mut app);
    set_mpn(&mut app, "MINE");
    row_update(&f, "1.2.4", |row| row.description = "reworded".into());
    check_drift(&mut app);
    assert!(is_drifted(&app), "unknown base, differing value");
    update_first(&mut app);
    assert_eq!(mpn(&app).as_deref(), Some("MINE"), "never auto-applied");
}

#[test]
fn an_old_part_with_the_version_unmoved_behaves_as_before() {
    let (_f, mut app) = placed();
    drop_stamp(&mut app);
    set_mpn(&mut app, "MINE");
    check_drift(&mut app);
    assert!(!is_drifted(&app), "an override of an unmoved row");
}

#[test]
fn a_stamp_of_another_row_falls_back_to_the_legacy_rules() {
    let (_f, mut app) = placed();
    set_mpn(&mut app, "MINE");
    let engine = app.document_state.active_engine_mut().unwrap();
    let mut sheet = engine.document().clone();
    sheet.symbols[0].library_stamp = Some(RowStamp {
        mpn: Some("MINE".into()),
        ..RowStamp::new(Uuid::now_v7())
    });
    engine.set_document(sheet);
    check_drift(&mut app);
    assert!(
        !is_drifted(&app),
        "trusted, that stamp would make TWO-1 a follow and overwrite MINE"
    );
}

#[test]
fn update_from_library_leaves_a_conflict_for_the_modal() {
    let (f, mut app) = placed();
    set_mpn(&mut app, "MINE");
    row_update(&f, "1.2.4", |row| row.primary_mpn.mpn = "LIB".into());
    check_drift(&mut app);
    update_first(&mut app);
    assert_eq!(mpn(&app).as_deref(), Some("MINE"));
    assert_eq!(
        doc(&app).symbols[0].library_version,
        "1.2.3",
        "an open conflict keeps the part on its version"
    );
    reopen(&mut app);
    scan(&mut app);
    let modal = app.library.library_updates.as_ref().expect("asked again");
    assert_eq!(modal.entries[0].conflicts.len(), 1);
    check_drift(&mut app);
    assert!(is_drifted(&app), "the flag stays up");
}

#[test]
fn a_legacy_conflict_is_not_cleared_by_update_from_library() {
    let f = fixture();
    // The row binds a footprint of a library that is not open: unknown.
    row_update(&f, "1.2.3", |row| {
        row.footprint_ref = Some(PrimitiveRef::new(Uuid::now_v7(), Uuid::now_v7()));
    });
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    select_first(&mut app);
    drop_stamp(&mut app);
    set_mpn(&mut app, "MINE");
    row_update(&f, "1.2.4", |row| {
        row.primary_mpn.manufacturer = "Other".into();
        row.primary_mpn.mpn = "LIB".into();
    });
    check_drift(&mut app);
    assert!(is_drifted(&app));
    update_first(&mut app);
    assert_eq!(mpn(&app).as_deref(), Some("MINE"));
    check_drift(&mut app);
    assert!(is_drifted(&app), "the conflict was not answered");
}

#[test]
fn a_team_mode_override_and_a_row_move_is_a_conflict_the_user_settles() {
    let f = fixture();
    let text = std::fs::read_to_string(&f.snxlib).unwrap();
    assert!(text.contains("mode = \"personal\""));
    std::fs::write(
        &f.snxlib,
        text.replace("mode = \"personal\"", "mode = \"team\""),
    )
    .unwrap();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    select_first(&mut app);
    set_mpn(&mut app, "MINE");
    row_update(&f, "1.2.4", |row| row.primary_mpn.mpn = "LIB".into());
    scan(&mut app);
    let entry = &app.library.library_updates.as_ref().expect("modal").entries[0];
    assert_eq!(entry.conflicts.len(), 1);
    apply_modal(&mut app, Some(Choice::UseLibrary));
    assert_eq!(mpn(&app).as_deref(), Some("LIB"));
    assert_eq!(stamp(&app).mpn.as_deref(), Some("LIB"));
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(mpn(&app).as_deref(), Some("MINE"), "one undo step");
    assert_eq!(doc(&app).symbols[0].library_version, "1.2.3");
}
