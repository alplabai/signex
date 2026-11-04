//! #1867 — a placed part follows its library row: the MPN and manufacturer
//! are stamped at placement, and a row whose footprint (or MPN) moved on
//! shows as drift that "Update from library" applies as one undo step.

use signex_app::app::{EditMsg, Message, Signex};
use signex_app::library::messages::{LibraryMessage, PrimitivePickerMsg};
use signex_app::library::state::{EditorAddress, PrimitivePickerTarget};
use signex_erc::RuleKind;
use signex_library::PrimitiveKind;
use signex_library::adapter::LibraryAdapter;
use signex_library::adapters::local_git::LocalGitAdapter;
use signex_library::primitive::{Footprint, PrimitiveRef};

use crate::library_place_symbol::{Fixture, app_with_library, doc, fixture, place};
use crate::library_symbol_drift::{check_drift, click_update, is_drifted, select_first};

/// Bind a saved footprint named `name` on the fixture's row, through the
/// picker the Library Browser binds with (the app's own row edit path).
fn bind_footprint(f: &Fixture, name: &str) {
    let adapter = LocalGitAdapter::open(&f.snxlib).expect("open");
    let footprint = Footprint::empty(name);
    let uuid = footprint.uuid;
    adapter
        .save_footprint(footprint, "seed footprint")
        .expect("save footprint");
    drop(adapter);

    let mut editor = app_with_library(f);
    let address = EditorAddress::new(
        f.snxlib.clone(),
        "ICs".into(),
        signex_library::RowId(f.row_id),
    );
    let _ = editor.update(Message::Library(LibraryMessage::OpenPrimitivePicker {
        kind: PrimitiveKind::Footprint,
        target: PrimitivePickerTarget::BrowserRow(address),
    }));
    let task = editor.update(Message::Library(LibraryMessage::PrimitivePicker(
        PrimitivePickerMsg::Pick(PrimitiveRef::new(f.library_id, uuid)),
    )));
    crate::drive::drive(&mut editor, task);
    let row = LocalGitAdapter::open(&f.snxlib)
        .expect("reopen")
        .read_row("ICs", signex_library::RowId(f.row_id))
        .expect("row");
    assert_eq!(
        row.footprint_ref,
        Some(PrimitiveRef::new(f.library_id, uuid)),
        "the picker bound the footprint"
    );
}

fn reports(app: &Signex, rule: RuleKind) -> usize {
    signex_erc::run(doc(app))
        .iter()
        .filter(|v| v.rule == rule)
        .count()
}

#[test]
fn placement_stamps_the_row_mpn_and_manufacturer() {
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);

    let fields = &doc(&app).symbols[0].fields;
    assert_eq!(fields.get("MPN").map(String::as_str), Some("TWO-1"));
    assert_eq!(fields.get("Manufacturer").map(String::as_str), Some("Acme"));
    assert_eq!(reports(&app, RuleKind::PartWithoutMpn), 0);
}

#[test]
fn a_part_placed_before_the_footprint_was_bound_updates_in_one_undo_step() {
    let _ = signex_app::diagnostics::init_logging();
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    assert!(doc(&app).symbols[0].footprint.is_empty());
    assert_eq!(reports(&app, RuleKind::PartWithoutFootprint), 1);

    bind_footprint(&f, "SOIC-8");
    let stamped = doc(&app).symbols[0].library_version.clone();
    let id = select_first(&mut app);
    check_drift(&mut app);
    assert!(is_drifted(&app), "the bound footprint is drift");
    assert!(
        doc(&app).symbols[0].footprint.is_empty(),
        "nothing is rewritten before the user asks"
    );

    click_update(&mut app, id);
    assert_eq!(doc(&app).symbols[0].footprint, "SOIC-8");
    let updated = doc(&app).symbols[0].library_version.clone();
    assert_ne!(updated, stamped, "the update restamps the version");
    assert_eq!(reports(&app, RuleKind::PartWithoutFootprint), 0);
    assert!(!is_drifted(&app));
    let logged = signex_app::diagnostics::recent_entries()
        .iter()
        .any(|e| e.message.contains("updated from library") && e.message.contains("SOIC-8"));
    assert!(logged, "the change is shown in Messages");

    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert!(
        doc(&app).symbols[0].footprint.is_empty(),
        "one undo restores the part"
    );
    assert_eq!(
        doc(&app).symbols[0].library_version,
        stamped,
        "the same undo reverts the version stamp"
    );
    assert_eq!(doc(&app).symbols.len(), 1, "and keeps it placed");
}

#[test]
fn a_row_footprint_change_is_reported_as_drift() {
    let f = fixture();
    bind_footprint(&f, "SOIC-8");
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    select_first(&mut app);
    check_drift(&mut app);
    assert!(!is_drifted(&app), "an unchanged row is not drift");

    bind_footprint(&f, "TSSOP-8");
    check_drift(&mut app);
    assert!(is_drifted(&app));

    let id = doc(&app).symbols[0].uuid;
    click_update(&mut app, id);
    assert_eq!(doc(&app).symbols[0].footprint, "TSSOP-8");
}

#[test]
fn a_per_instance_footprint_override_is_not_drift_while_the_row_is_unchanged() {
    let f = fixture();
    bind_footprint(&f, "SOIC-8");
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    assert_eq!(doc(&app).symbols[0].footprint, "SOIC-8");
    let path = app.document_state.active_path.clone().unwrap();
    let uuid = doc(&app).symbols[0].uuid;
    app.document_state
        .engines
        .get_mut(&path)
        .unwrap()
        .execute(signex_engine::Command::UpdateSymbolFootprint {
            symbol_id: uuid,
            footprint: "QFN-8".into(),
        })
        .unwrap();
    select_first(&mut app);
    check_drift(&mut app);
    assert!(
        !is_drifted(&app),
        "the row is unchanged; QFN-8 is the user's choice, and Update / Apply would rewrite it to SOIC-8"
    );
}

#[test]
fn an_in_app_footprint_edit_on_the_row_is_flagged_as_drift() {
    let f = fixture();
    bind_footprint(&f, "SOIC-8");
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    let placed = LocalGitAdapter::open(&f.snxlib)
        .unwrap()
        .read_row("ICs", signex_library::RowId(f.row_id))
        .unwrap()
        .version;
    select_first(&mut app);
    check_drift(&mut app);
    assert!(!is_drifted(&app));

    bind_footprint(&f, "TSSOP-8");
    let edited = LocalGitAdapter::open(&f.snxlib)
        .unwrap()
        .read_row("ICs", signex_library::RowId(f.row_id))
        .unwrap()
        .version;
    assert_ne!(edited, placed, "the edit moved the row's version");
    check_drift(&mut app);
    assert!(is_drifted(&app), "the part follows the edited row");
}

#[test]
fn an_open_time_scan_after_a_row_footprint_edit_keeps_the_drift() {
    let f = fixture();
    bind_footprint(&f, "SOIC-8");
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    let placed = doc(&app).symbols[0].library_version.clone();

    bind_footprint(&f, "TSSOP-8");
    // Reopen the sheet: a fresh engine over the same document, no history.
    let path = app.document_state.active_path.clone().unwrap();
    let document = doc(&app).clone();
    app.document_state.engines.insert(
        path.clone(),
        signex_engine::Engine::new_with_path(document, Some(path.clone())).unwrap(),
    );
    app.scan_library_updates_for_open_schematic(path);
    select_first(&mut app);
    check_drift(&mut app);
    let s = &doc(&app).symbols[0];
    assert!(
        is_drifted(&app) || s.footprint == "TSSOP-8",
        "the scan restamped the part ({placed} -> {}) but left {}; the row's TSSOP-8 is no longer offered",
        s.library_version,
        s.footprint
    );
}

#[test]
fn a_part_from_before_mpn_stamping_is_offered_the_row_mpn() {
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    let path = app.document_state.active_path.clone().unwrap();
    let uuid = doc(&app).symbols[0].uuid;
    // A part placed before this change: no Manufacturer / MPN, same stamp.
    for key in ["MPN", "Manufacturer"] {
        app.document_state
            .engines
            .get_mut(&path)
            .unwrap()
            .execute(signex_engine::Command::RemoveSymbolField {
                symbol_id: uuid,
                key: key.into(),
            })
            .unwrap();
    }
    select_first(&mut app);
    check_drift(&mut app);
    assert!(
        is_drifted(&app),
        "an empty MPN / Manufacturer is read as the user's override, so the row's TWO-1 / Acme is never offered"
    );
}

#[test]
fn a_mount_rescan_after_a_row_footprint_edit_keeps_the_drift() {
    let f = fixture();
    bind_footprint(&f, "SOIC-8");
    let mut app = app_with_library(&f);
    place(&mut app, &f);

    bind_footprint(&f, "TSSOP-8");
    // The sheet reopens while its library is not mounted, then mounts.
    app.library.close_library(&f.snxlib);
    let path = app.document_state.active_path.clone().unwrap();
    let document = doc(&app).clone();
    app.document_state.engines.insert(
        path.clone(),
        signex_engine::Engine::new_with_path(document, Some(path.clone())).unwrap(),
    );
    app.scan_library_updates_for_open_schematic(path.clone());
    let task = app.update(Message::Library(LibraryMessage::OpenLibraryBrowser(
        f.snxlib.clone(),
    )));
    crate::drive::drive(&mut app, task);
    assert!(app.library.library_at(&f.snxlib).is_some(), "mounted again");
    // Opening the browser made its tab active; the sheet is the subject.
    app.document_state.active_path = Some(path);

    select_first(&mut app);
    check_drift(&mut app);
    let s = &doc(&app).symbols[0];
    assert!(
        is_drifted(&app) || s.footprint == "TSSOP-8",
        "the mount rescan restamped the part but left {}; the row's TSSOP-8 is no longer offered",
        s.footprint
    );
}

#[test]
fn update_from_library_keeps_a_locked_part_drifted_and_updates_the_rest() {
    let _ = signex_app::diagnostics::init_logging();
    let f = fixture();
    bind_footprint(&f, "SOIC-8");
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    crate::library_place_symbol::complete(
        &mut app,
        &f,
        "place-364.snxsch",
        crate::library_place_symbol::resolve(&f, f.row_id),
    );
    crate::library_place_symbol::click(&mut app, (50.8, 38.1));
    assert_eq!(doc(&app).symbols.len(), 2, "two parts of the same row");
    let (locked, free) = (doc(&app).symbols[0].uuid, doc(&app).symbols[1].uuid);
    let stamped = doc(&app).symbols[0].library_version.clone();
    let path = app.document_state.active_path.clone().unwrap();
    app.document_state
        .engines
        .get_mut(&path)
        .unwrap()
        .execute(signex_engine::Command::SetSymbolLocked {
            symbol_id: locked,
            locked: true,
        })
        .unwrap();

    bind_footprint(&f, "TSSOP-8");
    select_first(&mut app);
    check_drift(&mut app);
    assert!(is_drifted(&app));
    click_update(&mut app, locked);

    let part = |app: &Signex, id| {
        doc(app)
            .symbols
            .iter()
            .find(|s| s.uuid == id)
            .unwrap()
            .clone()
    };
    assert_eq!(
        part(&app, free).footprint,
        "TSSOP-8",
        "the unlocked part updates"
    );
    assert_ne!(part(&app, free).library_version, stamped);
    assert_eq!(part(&app, locked).footprint, "SOIC-8", "the lock holds");
    assert_eq!(
        part(&app, locked).library_version,
        stamped,
        "the locked part keeps its old stamp"
    );
    let locked_reference = part(&app, locked).reference;
    check_drift(&mut app);
    assert!(is_drifted(&app), "the locked part is still flagged");
    let entries = signex_app::diagnostics::recent_entries();
    assert!(
        entries
            .iter()
            .any(|e| e.message.contains("is locked") && e.message.contains(&locked_reference)),
        "a warning names the locked part"
    );
}
