//! #1701 — a Library Browser range-filter edit is per-keystroke traffic:
//! it resyncs the grid's view cache itself and skips `finish_update`.

use std::collections::HashMap;
use std::path::PathBuf;

use signex_library::{
    ComponentClass, ComponentRow, DatasheetRef, InternalPn, LifecycleState, ManufacturerPart,
    ParamMap, ParamValue, PlmReserved, PrimitiveRef,
};
use signex_types::quantity::{SiPrefix, Unit};
use uuid::Uuid;

use crate::app::{Message, Signex};
use crate::library::messages::{LibraryMessage, RangeBound, RangeFilterMsg};
use crate::library::state::{LibraryBrowserState, LibraryDisplaySettings, OpenLibrary};

fn row(pn: &str, ohms: f64) -> ComponentRow {
    let now = chrono::Utc::now();
    let mut parameters = ParamMap::new();
    parameters.insert(
        "value".into(),
        ParamValue::Measurement {
            value: ohms,
            unit: "ohm".into(),
        },
    );
    ComponentRow {
        alternate_symbols: Vec::new(),
        row_id: Uuid::now_v7(),
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

fn app_with_browser() -> (Signex, PathBuf) {
    let (mut app, _task) = Signex::new();
    let root = PathBuf::from("/tmp/range-filter-dispatch/Lib.snxlib");
    let rows = vec![
        row("R-100", 100.0),
        row("R-1K", 1000.0),
        row("R-4K7", 4700.0),
    ];
    app.library.open_libraries.clear();
    app.library.open_libraries.push(OpenLibrary {
        root: root.clone(),
        display_name: "Lib".into(),
        library_id: Uuid::now_v7(),
        tables: HashMap::from([("Resistors".to_string(), rows)]),
        cached_components: Vec::new(),
        cached_symbols: Vec::new(),
        cached_footprints: Vec::new(),
        cached_sims: Vec::new(),
        cached_classes: Vec::new(),
        observed_stamp: None,
        own_edit_stamp: None,
        display: LibraryDisplaySettings::default(),
        dangling: Default::default(),
        pricing: Default::default(),
        alternate_supply: Default::default(),
        provenance: Default::default(),
    });
    app.library.touch_data();
    app.library
        .library_browsers
        .insert(root.clone(), LibraryBrowserState::new(root.clone()));
    (app, root)
}

#[test]
fn a_range_filter_edit_resyncs_the_grid_without_finish_update() {
    let (mut app, root) = app_with_browser();
    // `finish_update` drains this flag; a per-keystroke message must not.
    app.document_state.instance_note_due = true;

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
        let _ = app.update(Message::Library(LibraryMessage::BrowserRangeFilter {
            library_path: root.clone(),
            msg,
        }));
    }

    assert!(
        app.document_state.instance_note_due,
        "a range-filter keystroke ran the full finish_update"
    );
    let cache = app.library.library_browsers[&root]
        .view_cache
        .as_ref()
        .expect("the handler builds the grid's cache");
    assert_eq!(
        cache.visible_for_test().len(),
        2,
        ">= 1 kΩ keeps 1k and 4k7"
    );
}
