//! #1234 — a component row offers its default footprint and alternates: the
//! Components panel details view picks one (the preview follows, Place builds
//! the part with it), a placed part switches footprint through the engine
//! (one undo step), and library drift measures the part against the footprint
//! it was built with.
//!
//! The real `Stock.snxlib`: its chip passives carry 0805 as the default and
//! six alternates.

use std::path::PathBuf;

use signex_app::app::{EditMsg, Message, Signex};
use signex_app::dock::DockMessage;
use signex_app::library::LibraryMessage;
use signex_app::library::state::{ComponentsPanelRowKey, ComponentsRowAction};
use signex_app::panels::PanelMsg;
use signex_library::RowId;
use signex_library::primitive::PrimitiveRef;
use signex_types::schematic::SchematicSheet;

use crate::library_place_symbol::{PLACE_AT, app_with_empty_sheet, click, doc};
use crate::library_symbol_drift::{check_drift, is_drifted, select_first};
use crate::settle_support::settle;

fn stock_file() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/stock-library/Stock.snxlib")
}

pub(crate) fn app_with_stock() -> Signex {
    let mut app = app_with_empty_sheet();
    let stock = stock_file();
    app.library.stock_library_root = Some(stock.clone());
    app.library
        .open_library_read_only(stock)
        .expect("mount stock");
    app
}

pub(crate) fn key_of(app: &Signex, pn: &str) -> ComponentsPanelRowKey {
    let (_, row) = app
        .library
        .all_components()
        .into_iter()
        .find(|(_, c)| c.internal_pn.as_str() == pn)
        .unwrap_or_else(|| panic!("Stock lists {pn}"));
    ComponentsPanelRowKey {
        library_path: stock_file(),
        table: app
            .library
            .table_of_row(&stock_file(), row.row_id)
            .expect("table"),
        row_id: RowId::from_uuid(row.row_id),
    }
}

pub(crate) fn select(app: &mut Signex, key: &ComponentsPanelRowKey) {
    settle(
        app,
        Message::Library(LibraryMessage::ComponentsPanelRowPressed(key.clone())),
    );
}

/// The alternate footprints of the loaded details, by name.
pub(crate) fn alternates(app: &Signex) -> Vec<(PrimitiveRef, String)> {
    let loaded = app
        .library
        .components_panel
        .details
        .loaded
        .as_deref()
        .expect("details loaded");
    loaded
        .alternate_footprints
        .iter()
        .map(|a| (a.reference, a.name.clone()))
        .collect()
}

fn pick(app: &mut Signex, reference: PrimitiveRef) {
    settle(
        app,
        Message::Library(LibraryMessage::ComponentsPanelFootprintPicked(reference)),
    );
}

pub(crate) fn place_selected(app: &mut Signex, key: &ComponentsPanelRowKey) {
    settle(
        app,
        Message::Library(LibraryMessage::ComponentsPanelRowRightClicked(key.clone())),
    );
    settle(
        app,
        Message::Library(LibraryMessage::ComponentsPanelRowMenuPicked(
            ComponentsRowAction::Place,
        )),
    );
    click(app, PLACE_AT);
}

pub(crate) fn binding_key(reference: &PrimitiveRef) -> String {
    format!("{}:{}", reference.library_id, reference.uuid)
}

#[test]
fn the_stock_resistor_offers_its_default_and_six_alternates() {
    let mut app = app_with_stock();
    let key = key_of(&app, "STOCK-R");
    select(&mut app, &key);
    let loaded = app
        .library
        .components_panel
        .details
        .loaded
        .as_deref()
        .expect("details loaded");
    let choices = loaded.footprint_choices();
    assert_eq!(choices.len(), 7);
    assert!(choices[0].label.ends_with("(default)"), "{choices:?}");
}

#[test]
fn the_details_preview_follows_the_picker_and_a_new_row_resets_it() {
    let mut app = app_with_stock();
    let key = key_of(&app, "STOCK-R");
    select(&mut app, &key);
    let (alt, alt_name) = alternates(&app).remove(0);

    let loaded = app
        .library
        .components_panel
        .details
        .loaded
        .as_deref()
        .unwrap();
    let default_name = loaded
        .footprint_preview(None)
        .expect("default preview")
        .name
        .clone();
    assert_ne!(default_name, alt_name);

    pick(&mut app, alt);
    let panel = &app.library.components_panel;
    assert_eq!(panel.footprint_pick.as_ref().map(|(_, r)| *r), Some(alt));
    let loaded = panel.details.loaded.as_deref().unwrap();
    let pick_ref = panel.footprint_pick.as_ref().map(|(_, r)| r);
    assert_eq!(
        loaded
            .footprint_preview(pick_ref)
            .expect("alt preview")
            .name,
        alt_name,
        "the preview shows the picked footprint"
    );
    assert_eq!(loaded.shown_footprint(pick_ref).unwrap().reference, alt);

    // Another row starts from its own default.
    let other = key_of(&app, "STOCK-IC_Generic");
    select(&mut app, &other);
    assert!(app.library.components_panel.footprint_pick.is_none());
}

#[test]
fn picking_the_default_again_clears_the_pick() {
    let mut app = app_with_stock();
    let key = key_of(&app, "STOCK-R");
    select(&mut app, &key);
    let (alt, _) = alternates(&app).remove(0);
    pick(&mut app, alt);
    let default = app
        .library
        .components_panel
        .details
        .loaded
        .as_deref()
        .unwrap()
        .default_footprint
        .expect("default");
    pick(&mut app, default);
    assert!(app.library.components_panel.footprint_pick.is_none());
}

#[test]
fn placing_with_an_alternate_builds_the_part_with_it_and_records_the_choice() {
    let mut app = app_with_stock();
    let key = key_of(&app, "STOCK-R");
    select(&mut app, &key);
    let (alt, alt_name) = alternates(&app).remove(0);
    pick(&mut app, alt);

    place_selected(&mut app, &key);
    let sheet: &SchematicSheet = doc(&app);
    assert_eq!(sheet.symbols.len(), 1);
    assert_eq!(sheet.symbols[0].footprint, alt_name);
    let stamp = sheet.symbols[0].library_stamp.as_ref().expect("stamp");
    assert_eq!(stamp.footprint.as_deref(), Some(alt_name.as_str()));
    assert_eq!(
        stamp.footprint_ref.as_deref(),
        Some(binding_key(&alt).as_str())
    );
}

#[test]
fn placing_without_a_pick_uses_the_default_and_records_no_choice() {
    let mut app = app_with_stock();
    let key = key_of(&app, "STOCK-R");
    select(&mut app, &key);
    place_selected(&mut app, &key);
    let default_name = app
        .library
        .components_panel
        .details
        .loaded
        .as_deref()
        .unwrap()
        .footprint
        .as_ref()
        .unwrap()
        .name
        .clone();
    let part = &doc(&app).symbols[0];
    assert_eq!(part.footprint, default_name);
    assert_eq!(part.library_stamp.as_ref().unwrap().footprint_ref, None);
}

#[test]
fn a_part_placed_with_an_alternate_is_not_drifted() {
    let mut app = app_with_stock();
    let key = key_of(&app, "STOCK-R");
    select(&mut app, &key);
    let (alt, _) = alternates(&app).remove(0);
    pick(&mut app, alt);
    place_selected(&mut app, &key);

    check_drift(&mut app);
    assert!(!is_drifted(&app), "the chosen footprint is the row's offer");
    select_first(&mut app);
    check_drift(&mut app);
    assert_ne!(
        crate::library_symbol_drift::info(&app, "Library Drift").as_deref(),
        Some("Yes"),
        "the Properties panel does not report it as out of date"
    );
}

fn switch(app: &mut Signex, symbol: uuid::Uuid, reference: &PrimitiveRef) {
    let _ = app.update(Message::Dock(DockMessage::Panel(
        PanelMsg::SwitchFootprintChoice(symbol, binding_key(reference)),
    )));
}

#[test]
fn a_placed_part_switches_footprint_as_one_undo_step() {
    let mut app = app_with_stock();
    let key = key_of(&app, "STOCK-R");
    select(&mut app, &key);
    let (alt, alt_name) = alternates(&app).remove(0);
    place_selected(&mut app, &key);
    let id = select_first(&mut app);
    let before = doc(&app).symbols[0].clone();

    switch(&mut app, id, &alt);
    let part = &doc(&app).symbols[0];
    assert_eq!(part.footprint, alt_name);
    let stamp = part.valid_stamp().expect("stamp stays valid");
    assert_eq!(
        stamp.footprint_ref.as_deref(),
        Some(binding_key(&alt).as_str())
    );
    check_drift(&mut app);
    assert!(!is_drifted(&app), "drift follows the chosen footprint");

    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(doc(&app).symbols[0], before);
    let _ = app.update(Message::Edit(EditMsg::Redo));
    assert_eq!(doc(&app).symbols[0].footprint, alt_name);
}

#[test]
fn the_properties_picker_lists_the_row_footprints_with_the_current_one_selected() {
    let mut app = app_with_stock();
    let key = key_of(&app, "STOCK-R");
    select(&mut app, &key);
    let (alt, alt_name) = alternates(&app).remove(0);
    place_selected(&mut app, &key);
    let id = select_first(&mut app);
    check_drift(&mut app);

    let items = &app.document_state.panel_ctx.footprint_choices;
    assert_eq!(items.len(), 7);
    assert_eq!(items.iter().filter(|i| i.selected).count(), 1);
    assert!(items[0].selected, "the default is selected");

    switch(&mut app, id, &alt);
    check_drift(&mut app);
    let selected: Vec<&str> = app
        .document_state
        .panel_ctx
        .footprint_choices
        .iter()
        .filter(|i| i.selected)
        .map(|i| i.label.as_str())
        .collect();
    assert_eq!(selected, [alt_name.as_str()]);
}

#[test]
fn a_footprint_the_row_does_not_offer_is_refused() {
    let _ = signex_app::diagnostics::init_logging();
    let mut app = app_with_stock();
    let key = key_of(&app, "STOCK-R");
    select(&mut app, &key);
    place_selected(&mut app, &key);
    let id = select_first(&mut app);
    let before = doc(&app).symbols[0].clone();

    let stranger = PrimitiveRef::new(before.library_id.unwrap(), uuid::Uuid::now_v7());
    switch(&mut app, id, &stranger);
    assert_eq!(doc(&app).symbols[0], before);
}

#[test]
fn placing_from_the_library_picker_uses_the_default_even_after_a_panel_pick() {
    use signex_app::library::messages::PickerMsg;
    use signex_app::library::state::PickerState;

    let mut app = app_with_stock();
    let key = key_of(&app, "STOCK-R");
    select(&mut app, &key);
    let (alt, _) = alternates(&app).remove(0);
    pick(&mut app, alt);
    assert!(app.library.components_panel.footprint_pick.is_some());

    app.library.picker = Some(PickerState::default());
    settle(
        &mut app,
        Message::Library(LibraryMessage::Picker(PickerMsg::SelectComponent(
            key.row_id.0,
        ))),
    );
    settle(
        &mut app,
        Message::Library(LibraryMessage::Picker(PickerMsg::PlaceSelected)),
    );
    click(&mut app, PLACE_AT);

    let default_name = app
        .library
        .components_panel
        .details
        .loaded
        .as_deref()
        .unwrap()
        .footprint
        .as_ref()
        .unwrap()
        .name
        .clone();
    let part = &doc(&app).symbols[0];
    assert_eq!(part.footprint, default_name);
    assert_eq!(part.library_stamp.as_ref().unwrap().footprint_ref, None);
}

// ---- A row that drops the alternate a part was built with ----

mod dropped_alternate {
    use signex_app::app::Message;
    use signex_app::dock::DockMessage;
    use signex_app::library::place::resolve_placement_with_footprint;
    use signex_app::panels::PanelMsg;
    use signex_library::adapter::LibraryAdapter;
    use signex_library::adapters::local_git::LocalGitAdapter;
    use signex_library::primitive::{Footprint, PrimitiveRef};
    use signex_library::{FootprintBinding, RowId};

    use crate::library_place_symbol::{
        Fixture, PLACE_AT, app_with_library, click, complete, doc, fixture,
    };
    use crate::library_symbol_drift::{check_drift, is_drifted, run_task, select_first};

    /// The fixture's row bound to footprint `FP-A` with `FP-B` as an alternate.
    fn with_two_footprints(f: &Fixture) -> (PrimitiveRef, PrimitiveRef) {
        let adapter = LocalGitAdapter::open(&f.snxlib).expect("open");
        let (a, b) = (Footprint::empty("FP-A"), Footprint::empty("FP-B"));
        let refs = (
            PrimitiveRef::new(f.library_id, a.uuid),
            PrimitiveRef::new(f.library_id, b.uuid),
        );
        adapter.save_footprint(a, "seed").expect("save a");
        adapter.save_footprint(b, "seed").expect("save b");
        row_update(f, "1.2.4", |row| {
            row.footprint_ref = Some(refs.0);
            row.alternate_footprints = vec![FootprintBinding::new(refs.1)];
        });
        refs
    }

    fn row_update(
        f: &Fixture,
        version: &str,
        edit: impl FnOnce(&mut signex_library::ComponentRow),
    ) {
        let adapter = LocalGitAdapter::open(&f.snxlib).expect("open");
        let mut row = adapter.read_row("ICs", RowId(f.row_id)).expect("row");
        edit(&mut row);
        row.version = version.into();
        adapter.update_row("ICs", row, "edit").expect("update row");
    }

    /// The part placed with `FP-B`, ready to be updated.
    fn placed_with_b() -> (Fixture, signex_app::app::Signex, uuid::Uuid) {
        let f = fixture();
        let (_, b) = with_two_footprints(&f);
        let mut app = app_with_library(&f);
        let open = [(f.library_id, f.snxlib.clone())];
        let resolved =
            resolve_placement_with_footprint(&f.snxlib, "ICs", RowId(f.row_id), Some(b), &open)
                .map(Box::new);
        complete(&mut app, &f, "place-364.snxsch", resolved);
        click(&mut app, PLACE_AT);
        assert_eq!(doc(&app).symbols[0].footprint, "FP-B");
        let id = select_first(&mut app);
        // The row drops the alternate and moves on.
        row_update(&f, "1.2.5", |row| row.alternate_footprints.clear());
        (f, app, id)
    }

    fn start_update(app: &mut signex_app::app::Signex, id: uuid::Uuid) -> Vec<Message> {
        check_drift(app);
        assert!(is_drifted(app), "the dropped alternate is drift");
        run_task(app.update(Message::Dock(DockMessage::Panel(
            PanelMsg::UpdateSymbolFromLibrary(id),
        ))))
    }

    #[test]
    fn update_from_library_moves_a_part_off_a_dropped_alternate_to_the_default() {
        let (_f, mut app, id) = placed_with_b();
        for message in start_update(&mut app, id) {
            let _ = app.update(message);
        }
        let part = &doc(&app).symbols[0];
        assert_eq!(part.footprint, "FP-A");
        let stamp = part.valid_stamp().expect("stamp");
        assert_eq!(stamp.footprint.as_deref(), Some("FP-A"));
        assert_eq!(
            stamp.footprint_ref, None,
            "the dropped binding is forgotten"
        );
    }

    #[test]
    fn an_edit_made_while_the_update_ran_keeps_the_part_and_its_binding() {
        let (_f, mut app, id) = placed_with_b();
        let messages = start_update(&mut app, id);
        // The user retypes the footprint after the read, before it lands.
        app.document_state
            .active_engine_mut()
            .unwrap()
            .execute(signex_engine::Command::UpdateSymbolFootprint {
                symbol_id: id,
                footprint: "MINE".into(),
            })
            .unwrap();
        for message in messages {
            let _ = app.update(message);
        }
        let part = &doc(&app).symbols[0];
        assert_eq!(part.footprint, "MINE", "the edit is never overwritten");
        let stamp = part.valid_stamp().expect("stamp");
        assert!(
            stamp.footprint_ref.is_some(),
            "the footprint did not change, so the binding stays"
        );
    }
}
