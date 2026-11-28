//! Change Component as one dialog (mockup row 155): the filter chips, the
//! pin tags, the Now / After rows, Keep now and Apply to, through the pure
//! helpers and through `Signex::update`.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use signex_engine::ComponentReplacement;
use signex_library::{
    CachedPricing, ComponentClass, ComponentRow, ComponentSummary, DistributorListing, InternalPn,
    LifecycleState, ParamValue, PrimitiveRef, TemplateRegistry,
};
use signex_types::schematic::{
    LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, Point, RowStamp, SchematicSheet, Symbol,
    Wire, pin_tips,
};
use uuid::Uuid;

use super::{compare_rows, distinct_pins, hidden_rows, placed_now};
use crate::app::{EditMsg, Message, Signex, TabInfo, TabKind};
use crate::library::PickerState;
use crate::library::messages::{
    LibraryMessage, PlacedPartMsg, PlacedPartTarget, ResolvedPlacement,
};
use crate::library::state::{
    ChangeFilter, ChangeFilters, ChangeScope, ChangeSession, CompareField, CompareKind,
    OpenLibrary, PickerMode, PinMatch, PlacedNow,
};

const LIB: Uuid = Uuid::from_u128(0x11);
const OLD_ROW: Uuid = Uuid::from_u128(0x21);
const NEW_ROW: Uuid = Uuid::from_u128(0x22);
const SHEET: &str = "/p/buck_channel.snxsch";

fn pin(number: &str, unit: u32, body_style: u32) -> LibPin {
    LibPin {
        unit,
        body_style,
        pin: Pin {
            direction: PinDirection::Passive,
            shape_style: PinShapeStyle::Plain,
            position: Point::new(0.0, number.parse::<f64>().unwrap_or(0.0) * 2.54),
            rotation: 0.0,
            length: 2.54,
            name: number.to_string(),
            number: number.to_string(),
            visible: true,
            name_visible: true,
            number_visible: true,
        },
    }
}

fn drawing(id: &str, pins: &[&str]) -> LibSymbol {
    let mut lib: LibSymbol = serde_json::from_value(serde_json::json!({ "id": id })).unwrap();
    lib.pins = pins.iter().map(|n| pin(n, 1, 1)).collect();
    lib
}

fn row(row_id: Uuid, symbol: PrimitiveRef) -> ComponentRow {
    let mut row = crate::library::component_preview::test_row();
    row.row_id = row_id;
    row.symbol_ref = symbol;
    row
}

fn resistor(row_id: Uuid, symbol: PrimitiveRef, value: f64) -> ComponentRow {
    let mut row = row(row_id, symbol);
    row.class = ComponentClass::new("resistor");
    row.parameters.insert(
        "value".into(),
        ParamValue::Measurement {
            value,
            unit: "ohm".into(),
        },
    );
    row
}

fn library(rows: Vec<ComponentRow>) -> OpenLibrary {
    OpenLibrary {
        root: PathBuf::from("/libs/passives.snxlib"),
        display_name: "passives".into(),
        library_id: LIB,
        tables: HashMap::from([("R".to_string(), rows)]),
        cached_components: Vec::new(),
        cached_symbols: Vec::new(),
        cached_footprints: Vec::new(),
        cached_sims: Vec::new(),
        cached_classes: Vec::new(),
        observed_stamp: None,
        dangling: Default::default(),
        own_edit_stamp: None,
        display: Default::default(),
        pricing: Default::default(),
        alternate_supply: Default::default(),
        provenance: Default::default(),
    }
}

fn placed_with(lib_id: &str) -> PlacedNow {
    PlacedNow {
        lib_id: lib_id.to_string(),
        pins: Some(2),
        ..PlacedNow::default()
    }
}

#[test]
fn a_pin_number_counts_once_over_units_and_body_styles() {
    let mut lib = drawing("L:R", &["1", "2"]);
    lib.pins.push(pin("1", 1, 2));
    lib.pins.push(pin("2", 2, 1));
    assert_eq!(distinct_pins(&lib), 2);
}

#[test]
fn same_symbol_hides_the_rows_drawn_with_another_symbol() {
    let own = PrimitiveRef::new(LIB, Uuid::from_u128(0x31));
    let other = PrimitiveRef::new(LIB, Uuid::from_u128(0x32));
    let libs = [library(vec![row(OLD_ROW, own), row(NEW_ROW, other)])];
    let registry = TemplateRegistry::new_with_builtins();
    let placed = placed_with(&crate::library::place::lib_id_of(&own));

    let hidden = hidden_rows(&libs, &registry, &placed, ChangeFilters::default());
    assert_eq!(hidden.into_iter().collect::<Vec<_>>(), [NEW_ROW]);

    let off = ChangeFilters {
        same_symbol: false,
        ..ChangeFilters::default()
    };
    assert!(hidden_rows(&libs, &registry, &placed, off).is_empty());
}

#[test]
fn same_value_compares_the_class_value_parameter() {
    let symbol = PrimitiveRef::new(LIB, Uuid::from_u128(0x31));
    let (a, b) = (Uuid::from_u128(0x41), Uuid::from_u128(0x42));
    let libs = [library(vec![
        resistor(a, symbol, 31_600.0),
        resistor(b, symbol, 10_000.0),
    ])];
    let registry = TemplateRegistry::new_with_builtins();
    let shown = ParamValue::Measurement {
        value: 31_600.0,
        unit: "ohm".into(),
    }
    .display();
    let placed = PlacedNow {
        value: Some(("value".into(), shown)),
        ..placed_with(&crate::library::place::lib_id_of(&symbol))
    };
    let filters = ChangeFilters {
        same_value: true,
        ..ChangeFilters::default()
    };
    let hidden = hidden_rows(&libs, &registry, &placed, filters);
    assert_eq!(hidden.into_iter().collect::<Vec<_>>(), [b]);
}

#[test]
fn in_stock_keeps_only_rows_a_distributor_last_reported_in_stock() {
    let symbol = PrimitiveRef::new(LIB, Uuid::from_u128(0x31));
    let (stocked, empty, unpriced) = (
        Uuid::from_u128(0x51),
        Uuid::from_u128(0x52),
        Uuid::from_u128(0x53),
    );
    let listed = |row_id| {
        let mut r = row(row_id, symbol);
        r.supply = vec![DistributorListing::new("DigiKey", "SKU-1")];
        r
    };
    let mut lib = library(vec![listed(stocked), listed(empty), listed(unpriced)]);
    let price = |stock| CachedPricing {
        sku: "SKU-1".into(),
        currency: "USD".into(),
        stock,
        breaks: Vec::new(),
        fetched_at: chrono::Utc::now(),
    };
    for (row_id, stock) in [(stocked, Some(1200)), (empty, Some(0))] {
        lib.pricing.insert(
            row_id,
            BTreeMap::from([(
                "DigiKey".to_string(),
                BTreeMap::from([("SKU-1".to_string(), price(stock))]),
            )]),
        );
    }
    let registry = TemplateRegistry::new_with_builtins();
    let placed = placed_with(&crate::library::place::lib_id_of(&symbol));
    let filters = ChangeFilters {
        in_stock: true,
        ..ChangeFilters::default()
    };
    let mut hidden: Vec<Uuid> = hidden_rows(&[lib], &registry, &placed, filters)
        .into_iter()
        .collect();
    hidden.sort();
    assert_eq!(hidden, [empty, unpriced]);
}

#[test]
fn the_pin_tag_says_same_pins_or_how_many_against_the_placed_part() {
    let target = PlacedPartTarget {
        sheet: PathBuf::from(SHEET),
        engine_instance: 0,
        symbol_id: Uuid::from_u128(1),
    };
    let mut session = ChangeSession::new(target, "R5".into());
    session.placed.pins = Some(2);
    session.row_pins.insert(OLD_ROW, 2);
    session.row_pins.insert(NEW_ROW, 4);
    assert_eq!(session.pin_match(OLD_ROW), Some(PinMatch::Same));
    assert_eq!(
        session.pin_match(NEW_ROW),
        Some(PinMatch::Differs { row: 4, placed: 2 })
    );
    assert_eq!(session.pin_match(Uuid::from_u128(9)), None, "not read yet");
    assert_eq!(session.sheet_name, "buck_channel.snxsch");
}

fn placed_symbol(uuid: u128, reference: &str) -> Symbol {
    Symbol {
        uuid: Uuid::from_u128(uuid),
        lib_id: "L:R".into(),
        reference: reference.into(),
        value: "31.6k".into(),
        footprint: "R_0402".into(),
        unit: 1,
        library_id: Some(LIB),
        row_id: Some(OLD_ROW),
        fields: HashMap::from([
            ("MPN".to_string(), "RC0402FR-0731K6L".to_string()),
            ("Note".to_string(), "FB divider".to_string()),
            ("Tag".to_string(), "drop me".to_string()),
        ]),
        ..Default::default()
    }
}

fn sheet() -> SchematicSheet {
    let mut sheet = SchematicSheet {
        uuid: Uuid::now_v7(),
        paper_size: "A4".into(),
        ..Default::default()
    };
    sheet
        .lib_symbols
        .insert("L:R".into(), drawing("L:R", &["1", "2"]));
    sheet.symbols.push(placed_symbol(1, "R5"));
    sheet.symbols.push(placed_symbol(2, "R6"));
    let mut other = placed_symbol(3, "R7");
    other.row_id = Some(Uuid::from_u128(0x99));
    sheet.symbols.push(other);
    sheet
}

fn replacement() -> ComponentReplacement {
    ComponentReplacement {
        lib_symbol: drawing("L:NEW", &["1", "2"]),
        library_id: Some(LIB),
        row_id: Some(NEW_ROW),
        library_version: "1".into(),
        stamp: Some(RowStamp::new(NEW_ROW)),
        value: "31.6k".into(),
        footprint: "R_0603".into(),
        datasheet: String::new(),
        fields: vec![("MPN".into(), "RC0603FR-0731K6L".into())],
        pin_map: None,
    }
}

#[test]
fn the_table_marks_same_replaced_and_user_rows() {
    let doc = sheet();
    let r = replacement();
    let user = r.user_parameters(&doc, Uuid::from_u128(1));
    assert_eq!(user, ["Note", "Tag"]);
    let rows = compare_rows(&doc.symbols[0], &r, &user);
    let shape: Vec<(CompareField, CompareKind)> =
        rows.iter().map(|r| (r.field.clone(), r.kind)).collect();
    assert_eq!(
        shape,
        [
            (CompareField::Value, CompareKind::Same),
            (CompareField::Footprint, CompareKind::Replaced),
            (CompareField::Named("MPN".into()), CompareKind::Replaced),
            (CompareField::Named("Note".into()), CompareKind::User),
            (CompareField::Named("Tag".into()), CompareKind::User),
        ]
    );
    assert_eq!(
        (rows[1].now.as_str(), rows[1].after.as_str()),
        ("R_0402", "R_0603")
    );
}

#[test]
fn the_placed_part_is_named_by_its_mpn_and_counts_its_pins() {
    let doc = sheet();
    let placed = placed_now(
        &doc,
        &doc.symbols[0],
        &[],
        &TemplateRegistry::new_with_builtins(),
    );
    assert_eq!(placed.part, "RC0402FR-0731K6L");
    assert_eq!(placed.footprint, "R_0402");
    assert_eq!(placed.lib_id, "L:R");
    assert_eq!(placed.pins, Some(2));
    assert_eq!(placed.value, None, "the part's row is not open");
}

/// An app with [`sheet`] active and Change Component open on R5, with the
/// new part's row picked.
fn app_changing_r5() -> (Signex, PlacedPartTarget) {
    let (mut app, _task) = Signex::new();
    let engine = signex_engine::Engine::new(sheet()).unwrap();
    let target = PlacedPartTarget {
        sheet: PathBuf::from(SHEET),
        engine_instance: engine.instance_id(),
        symbol_id: Uuid::from_u128(1),
    };
    app.document_state
        .engines
        .insert(PathBuf::from(SHEET), engine);
    app.document_state.active_path = Some(PathBuf::from(SHEET));
    // Undo steps the active tab's document, so the sheet has its tab.
    app.document_state.tabs.push(TabInfo {
        title: "buck_channel".into(),
        path: PathBuf::from(SHEET),
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    let summary = ComponentSummary {
        row_id: NEW_ROW,
        internal_pn: InternalPn::new("R-0603-31K6"),
        mpn: "RC0603FR-0731K6L".into(),
        state: LifecycleState::Released,
        description: "31.6k 1% 0603".into(),
    };
    app.library.picker = Some(PickerState {
        mode: PickerMode::ChangeComponent(Box::new(ChangeSession::new(
            target.clone(),
            "R5".into(),
        ))),
        selected: Some((PathBuf::from("/libs/passives.snxlib"), summary)),
        ..PickerState::default()
    });
    (app, target)
}

fn resolved() -> ResolvedPlacement {
    let r = replacement();
    ResolvedPlacement {
        lib_id: "L:NEW".into(),
        lib_symbol: r.lib_symbol,
        library_id: LIB,
        row_id: NEW_ROW,
        library_version: r.library_version,
        stamp: RowStamp::new(NEW_ROW),
        internal_pn: "R-0603-31K6".into(),
        value: r.value,
        is_power: false,
        net_tie: false,
        prefix: "R".into(),
        class: "resistor".into(),
        prefix_override: None,
        footprint: r.footprint,
        datasheet: String::new(),
        fields: r.fields,
    }
}

fn send(app: &mut Signex, msg: PlacedPartMsg) {
    let _ = app.update(Message::Library(LibraryMessage::PlacedPart(msg)));
}

fn preview_of(app: &Signex) -> &crate::library::state::ChangePreview {
    match &app
        .library
        .picker
        .as_ref()
        .expect("the dialog is open")
        .mode
    {
        PickerMode::ChangePreview(change) => change,
        other => panic!("expected the comparison, got {other:?}"),
    }
}

fn read_new_part(app: &mut Signex, target: &PlacedPartTarget) {
    send(
        app,
        PlacedPartMsg::ChangePartResolved {
            target: target.clone(),
            row_id: NEW_ROW,
            result: Ok(Box::new(resolved())),
        },
    );
}

#[test]
fn the_read_part_shows_its_comparison_in_the_same_dialog() {
    let (mut app, target) = app_changing_r5();
    read_new_part(&mut app, &target);
    let change = preview_of(&app);
    assert_eq!(change.session.reference, "R5");
    assert_eq!(change.scope, ChangeScope::ThisPart);
    assert!(
        change.dropped.is_empty(),
        "user parameters are kept by default"
    );
    assert_eq!(change.part_count(), 1);
}

#[test]
fn keep_now_toggles_a_user_parameter_and_never_library_data() {
    let (mut app, target) = app_changing_r5();
    read_new_part(&mut app, &target);
    send(&mut app, PlacedPartMsg::ChangeKeepToggled("Tag".into()));
    assert_eq!(preview_of(&app).dropped, ["Tag"]);
    send(&mut app, PlacedPartMsg::ChangeKeepToggled("MPN".into()));
    assert_eq!(preview_of(&app).dropped, ["Tag"], "MPN is the new part's");
    send(&mut app, PlacedPartMsg::ChangeKeepToggled("Tag".into()));
    assert!(preview_of(&app).dropped.is_empty());
}

#[test]
fn change_swaps_only_the_reviewed_part_in_one_undo_step() {
    let (mut app, target) = app_changing_r5();
    read_new_part(&mut app, &target);
    send(&mut app, PlacedPartMsg::ChangeKeepToggled("Tag".into()));
    assert_eq!(preview_of(&app).part_count(), 1);
    let r6_before = app.document_state.engines[&PathBuf::from(SHEET)]
        .document()
        .symbols[1]
        .clone();

    send(&mut app, PlacedPartMsg::ChangeConfirmed);

    let doc = app.document_state.engines[&PathBuf::from(SHEET)].document();
    let r5 = &doc.symbols[0];
    assert_eq!(r5.footprint, "R_0603", "R5 changed");
    assert_eq!(r5.row_id, Some(NEW_ROW));
    assert_eq!(
        r5.fields.get("Note").map(String::as_str),
        Some("FB divider")
    );
    assert_eq!(r5.fields.get("Tag"), None, "an unticked box clears it");
    // R6 is placed from the same row but was never reviewed: its pins and
    // parameters stay as they were.
    assert_eq!(doc.symbols[1].footprint, r6_before.footprint);
    assert_eq!(doc.symbols[1].row_id, r6_before.row_id);
    assert_eq!(doc.symbols[1].fields, r6_before.fields);
    assert_eq!(
        doc.symbols[2].footprint, "R_0402",
        "another row's part stays"
    );
    assert!(app.library.picker.is_none(), "the dialog closes");

    let _ = app.update(Message::Edit(EditMsg::Undo));
    let doc = app.document_state.engines[&PathBuf::from(SHEET)].document();
    assert!(
        doc.symbols.iter().all(|s| s.footprint == "R_0402"),
        "one undo restores the part"
    );
}

#[test]
fn a_chip_that_hides_the_picked_row_puts_the_comparison_away() {
    let (mut app, target) = app_changing_r5();
    let own = PrimitiveRef::new(LIB, Uuid::from_u128(0x31));
    let other = PrimitiveRef::new(LIB, Uuid::from_u128(0x32));
    app.library.open_libraries = vec![library(vec![row(NEW_ROW, other)])];
    read_new_part(&mut app, &target);
    if let Some(picker) = app.library.picker.as_mut()
        && let PickerMode::ChangePreview(change) = &mut picker.mode
    {
        change.session.placed.lib_id = crate::library::place::lib_id_of(&own);
        change.session.filters.same_symbol = false;
    }

    send(
        &mut app,
        PlacedPartMsg::ChangeFilterToggled(ChangeFilter::SameSymbol),
    );

    let picker = app.library.picker.as_ref().expect("the dialog stays open");
    assert!(picker.selected.is_none(), "the hidden row is not picked");
    let PickerMode::ChangeComponent(session) = &picker.mode else {
        panic!(
            "expected the list without a comparison, got {:?}",
            picker.mode
        );
    };
    assert!(session.filters.same_symbol);
    assert!(session.hidden.contains(&NEW_ROW));
}

#[test]
fn a_row_whose_pins_differ_is_not_picked() {
    let (mut app, _target) = app_changing_r5();
    let mut lib = library(vec![row(
        NEW_ROW,
        PrimitiveRef::new(LIB, Uuid::from_u128(0x32)),
    )]);
    let picker = app.library.picker.as_mut().unwrap();
    // The row is listed, so only its pins keep it from being picked.
    lib.cached_components
        .extend(picker.selected.take().map(|(_, summary)| summary));
    app.library.open_libraries = vec![lib];
    let picker = app.library.picker.as_mut().unwrap();
    if let PickerMode::ChangeComponent(session) = &mut picker.mode {
        session.placed.pins = Some(2);
        session.row_pins.insert(NEW_ROW, 4);
    }

    send(&mut app, PlacedPartMsg::ChangeRowClicked(NEW_ROW));

    let picker = app.library.picker.as_ref().unwrap();
    assert!(picker.selected.is_none());
    assert!(matches!(picker.mode, PickerMode::ChangeComponent(_)));
}

#[test]
fn pin_counts_of_another_change_are_dropped() {
    let (mut app, target) = app_changing_r5();
    let other = PlacedPartTarget {
        symbol_id: Uuid::from_u128(2),
        ..target.clone()
    };
    send(
        &mut app,
        PlacedPartMsg::ChangePinCountsRead {
            target: other,
            counts: Ok(vec![(NEW_ROW, 4)]),
        },
    );
    send(
        &mut app,
        PlacedPartMsg::ChangePinCountsRead {
            target,
            counts: Ok(vec![(OLD_ROW, 2)]),
        },
    );
    let PickerMode::ChangeComponent(session) = &app.library.picker.as_ref().unwrap().mode else {
        panic!("the list is open");
    };
    assert_eq!(session.row_pins.get(&NEW_ROW), None);
    assert_eq!(session.row_pins.get(&OLD_ROW), Some(&2));
}

/// A pin-count read whose thread panicked says so in Messages, as a
/// symbol read that failed, and leaves the rows without a count.
#[test]
fn a_panicked_pin_count_read_says_so_in_messages() {
    let (mut app, target) = app_changing_r5();
    let needle = "pin count read 7f3c panicked";
    let before = crate::diagnostics::logged_count(needle);
    send(
        &mut app,
        PlacedPartMsg::ChangePinCountsRead {
            target,
            counts: Err(needle.into()),
        },
    );
    assert_eq!(crate::diagnostics::logged_count(needle), before + 1);
    let PickerMode::ChangeComponent(session) = &app.library.picker.as_ref().unwrap().mode else {
        panic!("the list is open");
    };
    assert_eq!(session.row_pins.get(&NEW_ROW), None);
}

#[test]
fn apply_to_stays_on_this_part_with_another_part_of_the_same_row_on_the_sheet() {
    let (mut app, target) = app_changing_r5();
    read_new_part(&mut app, &target);
    send(
        &mut app,
        PlacedPartMsg::ChangeScopePicked(ChangeScope::ThisPart),
    );
    let change = preview_of(&app);
    assert_eq!(change.scope, ChangeScope::ThisPart);
    assert_eq!(change.part_count(), 1, "R6 is not reached");
}

// Pin mapping (#2954): `PlacedPartMsg::ChangePinRemapped` through `update`
// (`pin_mapping.rs`), and Change following the edited mapping.

/// R5 moved clear of R6 and R7 (placed on its pins at the origin), with a
/// wire 10.16 mm to the right of each pin in `wired` (ids 0x500 on); R5's
/// pin tips by number.
fn r5_wired(app: &mut Signex, wired: &[&str]) -> HashMap<String, Point> {
    let engine = app
        .document_state
        .engines
        .get_mut(&PathBuf::from(SHEET))
        .unwrap();
    let mut doc = engine.document().clone();
    doc.symbols[0].position = Point::new(50.8, 50.8);
    let tips: HashMap<String, Point> = {
        let sym = &doc.symbols[0];
        let lib = doc.resolve_lib_symbol(sym).expect("R5's drawing");
        pin_tips(&lib, sym)
            .map(|(lp, tip)| (lp.pin.number.clone(), tip))
            .collect()
    };
    for (n, pin) in wired.iter().enumerate() {
        let tip = tips[*pin];
        doc.wires.push(Wire {
            uuid: Uuid::from_u128(0x500 + n as u128),
            start: tip,
            end: Point::new(tip.x + 10.16, tip.y),
            ..Default::default()
        });
    }
    engine.set_document(doc);
    tips
}

fn mapped(change: &crate::library::state::ChangePreview) -> Vec<(&str, Option<&str>)> {
    change
        .pins
        .iter()
        .map(|r| (r.old.as_str(), r.new.as_ref().map(|(key, _)| key.as_str())))
        .collect()
}

fn remap(app: &mut Signex, old: &str, new: Option<&str>) {
    send(
        app,
        PlacedPartMsg::ChangePinRemapped {
            old: old.into(),
            new: new.map(str::to_string),
        },
    );
}

#[test]
fn a_remapped_pin_shows_in_the_pin_table_and_change_carries_its_wire() {
    let (mut app, target) = app_changing_r5();
    let tips = r5_wired(&mut app, &["2"]);
    read_new_part(&mut app, &target);
    let change = preview_of(&app);
    assert!(change.pin_map.is_none(), "mapped by number until an edit");
    assert_eq!(mapped(change), [("1", Some("1")), ("2", Some("2"))]);
    assert_eq!(change.pins[0].net, None, "nothing is on pin 1");
    assert_eq!(change.pins[1].net, Some(String::new()), "an unnamed net");
    assert_eq!(change.pin_choices.len(), 2);

    remap(&mut app, "2", Some("1"));
    remap(&mut app, "1", Some("2"));
    let change = preview_of(&app);
    assert!(change.pin_map.is_some());
    assert_eq!(mapped(change), [("1", Some("2")), ("2", Some("1"))]);
    assert_eq!(change.preview.carried, vec!["2".to_string()]);
    assert!(change.preview.refused.is_none());

    send(&mut app, PlacedPartMsg::ChangeConfirmed);
    assert!(app.library.picker.is_none(), "the dialog closes");
    let doc = app.document_state.engines[&PathBuf::from(SHEET)].document();
    assert_eq!(doc.symbols[0].row_id, Some(NEW_ROW));
    let wire = |doc: &SchematicSheet| {
        let wire = doc.wires.iter().find(|w| w.uuid == Uuid::from_u128(0x500));
        signex_net::pt_key(&wire.expect("the wire").start)
    };
    assert_eq!(
        wire(doc),
        signex_net::pt_key(&tips["1"]),
        "the wire of old pin 2 reaches new pin 1"
    );

    let _ = app.update(Message::Edit(EditMsg::Undo));
    let doc = app.document_state.engines[&PathBuf::from(SHEET)].document();
    assert_eq!(doc.symbols[0].row_id, Some(OLD_ROW), "one undo step");
    assert_eq!(wire(doc), signex_net::pt_key(&tips["2"]));
}

#[test]
fn a_mapping_the_sheet_cannot_follow_is_refused_and_the_dialog_stays() {
    let (mut app, target) = app_changing_r5();
    r5_wired(&mut app, &["1", "2"]);
    read_new_part(&mut app, &target);
    // Old pin 2 to new pin 1 leaves old pin 1 unmatched, and its wire stays
    // on the tip old pin 2's wire would be carried to.
    remap(&mut app, "2", Some("1"));
    let change = preview_of(&app);
    assert!(change.preview.refused.is_some(), "{:?}", change.preview);
    assert_eq!(change.preview.unmatched_connected, vec!["1".to_string()]);

    let needle = "Change the pin mapping of R5";
    let before = crate::diagnostics::logged_count(needle);
    send(&mut app, PlacedPartMsg::ChangeConfirmed);
    assert_eq!(crate::diagnostics::logged_count(needle), before + 1);
    assert!(matches!(
        app.library.picker.as_ref().map(|p| &p.mode),
        Some(PickerMode::ChangePreview(_))
    ));
    let doc = app.document_state.engines[&PathBuf::from(SHEET)].document();
    assert_eq!(doc.symbols[0].row_id, Some(OLD_ROW), "nothing changed");
}

#[test]
fn a_remap_of_a_pin_or_to_a_pin_the_part_lacks_changes_nothing() {
    let (mut app, target) = app_changing_r5();
    read_new_part(&mut app, &target);
    remap(&mut app, "9", Some("1"));
    remap(&mut app, "1", Some("7"));
    let change = preview_of(&app);
    assert!(change.pin_map.is_none());
    assert_eq!(mapped(change), [("1", Some("1")), ("2", Some("2"))]);
}
