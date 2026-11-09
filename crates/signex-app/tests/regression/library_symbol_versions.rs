//! #2209 — a sheet embeds one body per version of a library symbol: a new
//! placement always draws with the library's current symbol, parts already
//! placed keep their version until "Update from library", and old and new
//! versions coexist on one sheet.

use crate::library_place_symbol::{app_with_library, doc, fixture, place, resolve};
use crate::library_symbol_drift::{
    add_pin, check_drift, click_update, edit_library_symbol, is_drifted, select_first,
};
use signex_app::app::{EditMsg, Message, Signex};
use signex_app::dock::DockMessage;
use signex_app::panels::PanelMsg;
use signex_types::schematic::SymbolTransform;
use uuid::Uuid;

/// Pins of the body the symbol at `index` draws with.
fn pins_of(app: &Signex, index: usize) -> usize {
    let sheet = doc(app);
    sheet
        .body_of(&sheet.symbols[index])
        .expect("body")
        .pins
        .len()
}

/// Place twice around a library edit: symbol 0 on the old version, symbol 1 on
/// the new one.
fn two_versions() -> (crate::library_place_symbol::Fixture, Signex) {
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    edit_library_symbol(&f, add_pin);
    place(&mut app, &f);
    (f, app)
}

#[test]
fn a_placement_after_a_library_change_uses_the_new_body_and_the_old_instance_keeps_the_old() {
    let (_f, app) = two_versions();
    let sheet = doc(&app);
    assert_eq!(sheet.symbols.len(), 2);
    assert_eq!(sheet.lib_symbols.len(), 2, "both versions are embedded");
    assert_eq!(sheet.symbols[0].lib_id, sheet.symbols[1].lib_id);
    assert_ne!(sheet.symbols[0].body_ref, sheet.symbols[1].body_ref);
    assert_eq!(pins_of(&app, 1), pins_of(&app, 0) + 1);
}

#[test]
fn update_from_library_moves_every_instance_and_one_undo_restores_both_versions() {
    let (f, mut app) = two_versions();
    let id = select_first(&mut app);
    check_drift(&mut app);
    assert!(is_drifted(&app));
    let (old_pins, new_pins) = (pins_of(&app, 0), pins_of(&app, 1));

    click_update(&mut app, id);
    assert_eq!(pins_of(&app, 0), new_pins);
    assert_eq!(pins_of(&app, 1), new_pins);
    assert_eq!(doc(&app).lib_symbols.len(), 1, "the old version is dropped");

    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(pins_of(&app, 0), old_pins, "one undo restores the old one");
    assert_eq!(pins_of(&app, 1), new_pins);
    assert_eq!(doc(&app).lib_symbols.len(), 2);
    drop(f);
}

#[test]
fn only_this_one_moves_one_part_and_leaves_the_other_version_in_place() {
    let (_f, mut app) = two_versions();
    let id = select_first(&mut app);
    check_drift(&mut app);
    let (old_pins, new_pins) = (pins_of(&app, 0), pins_of(&app, 1));

    let task = app.update(Message::Dock(DockMessage::Panel(
        PanelMsg::UpdateSymbolFromLibraryOnly(id),
    )));
    for message in crate::library_symbol_drift::run_task(task) {
        let _ = app.update(message);
    }
    assert_eq!(pins_of(&app, 0), new_pins, "the chosen part moved");
    assert_eq!(pins_of(&app, 1), new_pins);
    assert_eq!(
        doc(&app).lib_symbols.len(),
        1,
        "nothing draws with the old one"
    );
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(pins_of(&app, 0), old_pins);
}

#[test]
fn only_this_one_on_a_sheet_with_two_parts_leaves_the_other_part_on_its_version() {
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    place(&mut app, &f);
    edit_library_symbol(&f, add_pin);
    let id = select_first(&mut app);
    check_drift(&mut app);
    let old_pins = pins_of(&app, 1);

    let task = app.update(Message::Dock(DockMessage::Panel(
        PanelMsg::UpdateSymbolFromLibraryOnly(id),
    )));
    for message in crate::library_symbol_drift::run_task(task) {
        let _ = app.update(message);
    }
    assert_eq!(pins_of(&app, 0), old_pins + 1);
    assert_eq!(
        pins_of(&app, 1),
        old_pins,
        "the other part keeps its version"
    );
    assert_eq!(doc(&app).lib_symbols.len(), 2);
}

#[test]
fn properties_say_how_many_instances_use_an_older_version() {
    let (_f, mut app) = two_versions();
    select_first(&mut app);
    check_drift(&mut app);
    assert_eq!(
        crate::library_symbol_drift::info(&app, "Library Older Version").as_deref(),
        Some("1")
    );
}

#[test]
fn update_refuses_and_names_the_pin_when_a_connected_pin_would_be_lost() {
    let _ = signex_app::diagnostics::init_logging();
    let (f, mut app) = two_versions();
    let tip = {
        let sheet = doc(&app);
        let sym = &sheet.symbols[0];
        let pin1 = sheet
            .body_of(sym)
            .unwrap()
            .pins
            .iter()
            .find(|p| p.pin.number == "1")
            .unwrap()
            .pin
            .position;
        SymbolTransform::from_symbol(sym).apply(pin1)
    };
    app.document_state
        .active_engine_mut()
        .unwrap()
        .execute(signex_engine::Command::PlaceWireSegment {
            wire: signex_types::schematic::Wire {
                start: tip,
                end: signex_types::schematic::Point::new(tip.x - 10.0, tip.y),
                ..Default::default()
            },
        })
        .unwrap();
    let id = select_first(&mut app);
    edit_library_symbol(&f, |s| s.pins.retain(|p| p.number != "1"));
    check_drift(&mut app);
    let before = (
        pins_of(&app, 0),
        pins_of(&app, 1),
        doc(&app).lib_symbols.len(),
    );

    click_update(&mut app, id);
    assert_eq!(
        (
            pins_of(&app, 0),
            pins_of(&app, 1),
            doc(&app).lib_symbols.len()
        ),
        before,
        "a refused update changes nothing"
    );
    let logged = signex_app::diagnostics::recent_entries()
        .iter()
        .any(|e| e.message.contains("update refused") && e.message.contains("U1 pin 1"));
    assert!(logged, "the refusal reason must reach the Messages panel");
}

#[test]
fn a_fresh_sheet_places_the_body_the_components_panel_previews() {
    use signex_library::adapters::local_git::LocalGitAdapter;
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);

    let own = LocalGitAdapter::open(&f.snxlib).expect("open");
    let key = signex_app::library::state::ComponentsPanelRowKey {
        library_path: f.snxlib.clone(),
        table: "ICs".into(),
        row_id: signex_library::RowId(f.row_id),
    };
    let details =
        signex_app::panels::components_panel::details::load::load_details(&key, &own, &vec![])
            .expect("details");
    let previewed = details.symbol.expect("preview symbol");
    let sheet = doc(&app);
    let placed = sheet.body_of(&sheet.symbols[0]).expect("placed body");
    assert_eq!(placed, &previewed, "placement and preview read one source");
    let _ = Uuid::nil();
}

#[test]
fn a_sheet_with_two_versions_survives_save_and_reopen() {
    let (_f, mut app) = two_versions();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sheet.snxsch");
    app.document_state
        .active_engine_mut()
        .unwrap()
        .save_as(&path)
        .unwrap();
    let reopened = signex_engine::Engine::open(&path).unwrap();
    let sheet = reopened.document();
    assert_eq!(sheet.lib_symbols.len(), 2);
    let pins = |i: usize| sheet.body_of(&sheet.symbols[i]).unwrap().pins.len();
    assert_eq!(pins(1), pins(0) + 1);
}

#[test]
fn resolving_twice_gives_the_same_version() {
    let f = fixture();
    let a = resolve(&f, f.row_id).unwrap();
    let b = resolve(&f, f.row_id).unwrap();
    assert_eq!(a.lib_symbol.body_ref(), b.lib_symbol.body_ref());
}

/// R1 on v1, a library edit, R2 placed on v2, R1 deleted: the v1 body no
/// instance draws with is not judged, so nothing reads "out of date".
#[test]
fn a_version_no_instance_draws_with_any_more_is_not_drift() {
    let (_f, mut app) = two_versions();
    select_first(&mut app);
    check_drift(&mut app);
    assert!(is_drifted(&app), "R1 is behind while it exists");

    let _ = app.update(Message::Edit(EditMsg::DeleteSelected));
    assert_eq!(doc(&app).symbols.len(), 1);
    assert_eq!(
        doc(&app).lib_symbols.len(),
        2,
        "the orphan body stays embedded"
    );
    check_drift(&mut app);
    assert!(!is_drifted(&app), "an orphan body is not drift");
}

/// "Only this one" is offered for an instance that is behind, not for one
/// that already draws with the current version.
#[test]
fn only_this_one_is_not_offered_for_an_instance_that_is_current() {
    let (_f, mut app) = two_versions();
    for (index, current) in [(0, false), (1, true)] {
        let id = doc(&app).symbols[index].uuid;
        app.interaction_state.active_canvas_mut().selected =
            vec![signex_types::schematic::SelectedItem::new(
                id,
                signex_types::schematic::SelectedKind::Symbol,
            )];
        check_drift(&mut app);
        assert_eq!(
            crate::library_symbol_drift::info(&app, "Library This One Current").as_deref(),
            current.then_some("Yes"),
            "instance {index}"
        );
    }
}
