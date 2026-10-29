//! #1572 phase 4 — the range filter reached through the real dispatcher.
//!
//! The Place Component picker and a Library Browser tab each take the
//! filter as a namespaced library message. Typing a prefix or a unit into
//! the number field is refused (the field keeps its number and says why),
//! a `>= 1 k` filter compares in base units so a legacy `{4.7, "kΩ"}` row
//! stays, and clearing the filter brings every row back.

use std::collections::HashMap;
use std::path::PathBuf;

use signex_app::app::{Message, Signex};
use signex_app::library::messages::{LibraryMessage, PickerMsg, RangeBound, RangeFilterMsg};
use signex_app::library::state::{LibraryBrowserState, LibraryDisplaySettings, OpenLibrary};
use signex_library::{
    ComponentClass, ComponentRow, ComponentSummary, DatasheetRef, InternalPn, LifecycleState,
    ManufacturerPart, ParamMap, ParamValue, PlmReserved, PrimitiveRef, RangeOp,
};
use signex_types::quantity::{SiPrefix, Unit};
use uuid::Uuid;

fn row(pn: &str, value: Option<ParamValue>) -> ComponentRow {
    let now = chrono::Utc::now();
    let mut parameters = ParamMap::new();
    if let Some(v) = value {
        parameters.insert("value".into(), v);
    }
    ComponentRow {
        alternate_symbols: Vec::new(),
        row_id: Uuid::new_v4(),
        internal_pn: InternalPn::new(pn),
        class: ComponentClass::generic(),
        datasheet: DatasheetRef::default(),
        datasheet_label: String::new(),
        description: String::new(),
        more_datasheets: Vec::new(),
        state: LifecycleState::Draft,
        symbol_ref: PrimitiveRef::new(Uuid::nil(), Uuid::nil()),
        footprint_ref: None,
        alternate_footprints: Vec::new(),
        sim_ref: None,
        pin_map_overrides: Vec::new(),
        primary_mpn: ManufacturerPart::draft("Acme", "MPN"),
        alternates: Vec::new(),
        supply: Vec::new(),
        parameters,
        plm: PlmReserved::default(),
        version: "0.0.1".into(),
        released: false,
        symbol_version: String::new(),
        footprint_version: String::new(),
        sim_version: String::new(),
        created: now,
        updated: now,
        content_hash: [0u8; 32],
        server_revision: None,
    }
}

fn measured(value: f64, unit: &str) -> ParamValue {
    ParamValue::Measurement {
        value,
        unit: unit.into(),
    }
}

/// An app with one open library of resistors and a browser tab on it.
fn app_with_resistors() -> (Signex, PathBuf) {
    let rows = vec![
        row("R-1K", Some(measured(1000.0, "ohm"))),
        row("R-4K7", Some(measured(4700.0, "ohm"))),
        row("R-4K7-LEGACY", Some(measured(4.7, "kΩ"))),
        row("R-100", Some(measured(100.0, "ohm"))),
        row("R-TEXT", Some(ParamValue::Text("10k".into()))),
    ];
    let summaries = rows
        .iter()
        .map(|r| ComponentSummary {
            row_id: r.row_id,
            internal_pn: r.internal_pn.clone(),
            mpn: r.primary_mpn.mpn.clone(),
            state: r.state,
            description: String::new(),
        })
        .collect();
    let root = PathBuf::from("range-filter.snxlib");
    let (mut app, _boot) = Signex::new();
    app.library.open_libraries.clear();
    app.library.open_libraries.push(OpenLibrary {
        root: root.clone(),
        display_name: "Range".into(),
        library_id: Uuid::new_v4(),
        tables: HashMap::from([("Resistors".to_string(), rows)]),
        cached_components: summaries,
        cached_symbols: Vec::new(),
        cached_footprints: Vec::new(),
        cached_sims: Vec::new(),
        cached_classes: Vec::new(),
        observed_stamp: None,
        own_edit_stamp: None,
        display: LibraryDisplaySettings::default(),
        dangling: Default::default(),
        pricing: Default::default(),
    });
    app.library.touch_data();
    let mut browser = LibraryBrowserState::new(root.clone());
    browser.active_table = Some("Resistors".into());
    app.library.library_browsers.insert(root.clone(), browser);
    (app, root)
}

fn picker_msg(app: &mut Signex, msg: RangeFilterMsg) {
    let _ = app.update(Message::Library(LibraryMessage::Picker(
        PickerMsg::RangeFilter(msg),
    )));
}

fn picker_pns(app: &Signex) -> Vec<String> {
    app.library
        .picker
        .as_ref()
        .expect("picker open")
        .rows
        .visible()
        .map(|e| e.summary.internal_pn.as_str().to_string())
        .collect()
}

fn at_least_one_kilohm(app: &mut Signex) {
    let key = "value".to_string();
    for msg in [
        RangeFilterMsg::ColumnPicked {
            key: key.clone(),
            unit: Unit::Ohm,
        },
        RangeFilterMsg::NumberEdited {
            key: key.clone(),
            bound: RangeBound::Low,
            text: "1".into(),
        },
        RangeFilterMsg::PrefixPicked {
            key,
            bound: RangeBound::Low,
            prefix: SiPrefix::Kilo,
        },
    ] {
        picker_msg(app, msg);
    }
}

#[test]
fn the_picker_filters_in_base_units_and_clearing_restores_every_row() {
    let (mut app, _) = app_with_resistors();
    let _ = app.update(Message::Library(LibraryMessage::OpenPicker));
    assert_eq!(picker_pns(&app).len(), 5);

    at_least_one_kilohm(&mut app);

    assert_eq!(picker_pns(&app), ["R-1K", "R-4K7", "R-4K7-LEGACY"]);
    let rows = &app.library.picker.as_ref().unwrap().rows;
    assert_eq!(
        rows.range_excluded().get("value"),
        Some(&1),
        "the text cell"
    );

    picker_msg(
        &mut app,
        RangeFilterMsg::Cleared {
            key: "value".into(),
        },
    );
    assert_eq!(picker_pns(&app).len(), 5);
}

#[test]
fn letters_typed_into_the_number_field_are_refused() {
    let (mut app, _) = app_with_resistors();
    let _ = app.update(Message::Library(LibraryMessage::OpenPicker));
    at_least_one_kilohm(&mut app);

    picker_msg(
        &mut app,
        RangeFilterMsg::NumberEdited {
            key: "value".into(),
            bound: RangeBound::Low,
            text: "4k7".into(),
        },
    );

    let picker = app.library.picker.as_ref().unwrap();
    let draft = picker.range_filters.get("value").expect("filter");
    assert_eq!(draft.low.text, "1", "the field keeps its number");
    assert!(
        draft
            .low
            .refusal
            .as_deref()
            .is_some_and(|w| w.contains("letters"))
    );
    assert_eq!(picker_pns(&app), ["R-1K", "R-4K7", "R-4K7-LEGACY"]);
}

#[test]
fn a_browser_tab_takes_the_filter_and_a_table_switch_drops_it() {
    let (mut app, root) = app_with_resistors();
    for msg in [
        RangeFilterMsg::ColumnPicked {
            key: "value".into(),
            unit: Unit::Ohm,
        },
        RangeFilterMsg::OperatorPicked {
            key: "value".into(),
            op: RangeOp::Equals,
        },
    ] {
        let _ = app.update(Message::Library(LibraryMessage::BrowserRangeFilter {
            library_path: root.clone(),
            msg,
        }));
    }
    let filters = &app.library.library_browsers[&root].range_filters;
    assert_eq!(filters.get("value").map(|d| d.op), Some(RangeOp::Equals));

    let _ = app.update(Message::Library(LibraryMessage::BrowserSelectTable {
        library_path: root.clone(),
        table: "Capacitors".into(),
    }));
    assert!(app.library.library_browsers[&root].range_filters.is_empty());
}
