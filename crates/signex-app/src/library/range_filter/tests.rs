//! The range filter in the library browser and the Place Component picker
//! (#1572, phase 4): base-unit comparison, the prefix and number
//! pickers, exclusion of rows that are not measurements.

use std::collections::HashMap;
use std::path::PathBuf;

use signex_library::{
    ComponentClass, ComponentRow, ComponentSummary, DatasheetRef, InternalPn, LifecycleState,
    ManufacturerPart, ParamMap, ParamValue, PlmReserved, PrimitiveRef, RangeOp,
};
use signex_types::quantity::{SiPrefix, Unit};
use uuid::Uuid;

use super::RangeFilters;
use crate::library::messages::{RangeBound, RangeFilterMsg};
use crate::library::state::{
    LibraryBrowserState, LibraryDisplaySettings, LibraryState, OpenLibrary, PickerState,
};

fn measured(value: f64, unit: &str) -> ParamValue {
    ParamValue::Measurement {
        value,
        unit: unit.into(),
    }
}

fn row(pn: &str, value: Option<ParamValue>) -> ComponentRow {
    let now = chrono::Utc::now();
    let mut parameters = ParamMap::new();
    if let Some(v) = value {
        parameters.insert("value".into(), v);
    }
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

/// A library with one table, a browser tab on it, and the Place
/// Component picker open.
fn fixture(table: &str, rows: Vec<ComponentRow>) -> (LibraryState, PathBuf) {
    let root = PathBuf::from("/tmp/range-filter/Lib.snxlib");
    let summaries: Vec<ComponentSummary> = rows
        .iter()
        .map(|r| ComponentSummary {
            row_id: r.row_id,
            internal_pn: r.internal_pn.clone(),
            mpn: r.primary_mpn.mpn.clone(),
            state: r.state,
            description: String::new(),
        })
        .collect();
    let mut state = LibraryState::default();
    state.open_libraries.push(OpenLibrary {
        root: root.clone(),
        display_name: "Lib".into(),
        library_id: Uuid::now_v7(),
        tables: HashMap::from([(table.to_string(), rows)]),
        cached_components: summaries,
        cached_symbols: Vec::new(),
        cached_footprints: Vec::new(),
        cached_sims: Vec::new(),
        cached_classes: Vec::new(),
        observed_stamp: None,
        dangling: Default::default(),
        pricing: Default::default(),
        own_edit_stamp: None,
        display: LibraryDisplaySettings::default(),
    });
    state.touch_data();
    let mut browser = LibraryBrowserState::new(root.clone());
    browser.active_table = Some(table.to_string());
    state.library_browsers.insert(root.clone(), browser);
    state.picker = Some(PickerState::default());
    state.sync_view_caches();
    (state, root)
}

fn resistors() -> Vec<ComponentRow> {
    vec![
        row("R-1K", Some(measured(1000.0, "ohm"))),
        row("R-4K7", Some(measured(4700.0, "ohm"))),
        row("R-4K7-LEGACY", Some(measured(4.7, "kΩ"))),
        row("R-100", Some(measured(100.0, "ohm"))),
        row("R-TEXT", Some(ParamValue::Text("10k".into()))),
        row("R-NONE", None),
    ]
}

/// Messages exactly as the widgets send them: pick the column, pick the
/// operator, type the number, pick the prefix.
fn set_filter(filters: &mut RangeFilters, unit: Unit, op: RangeOp, bounds: &[(&str, SiPrefix)]) {
    let key = "value".to_string();
    filters.apply(RangeFilterMsg::ColumnPicked {
        key: key.clone(),
        unit,
    });
    filters.apply(RangeFilterMsg::OperatorPicked {
        key: key.clone(),
        op,
    });
    for (&(text, prefix), bound) in bounds.iter().zip([RangeBound::Low, RangeBound::High]) {
        filters.apply(RangeFilterMsg::NumberEdited {
            key: key.clone(),
            bound,
            text: text.into(),
        });
        filters.apply(RangeFilterMsg::PrefixPicked {
            key: key.clone(),
            bound,
            prefix,
        });
    }
}

fn browser_filters<'a>(state: &'a mut LibraryState, root: &PathBuf) -> &'a mut RangeFilters {
    &mut state
        .library_browsers
        .get_mut(root)
        .expect("browser")
        .range_filters
}

fn browser_pns(state: &LibraryState, root: &PathBuf, table: &str) -> Vec<String> {
    let lib = state.library_at(root).expect("library");
    let cache = state.library_browsers[root]
        .view_cache
        .as_ref()
        .expect("synced");
    cache
        .visible_for_test()
        .iter()
        .map(|&i| lib.tables[table][i].internal_pn.as_str().to_string())
        .collect()
}

fn picker_pns(state: &LibraryState) -> Vec<String> {
    state
        .picker
        .as_ref()
        .expect("picker")
        .rows
        .visible()
        .map(|e| e.summary.internal_pn.as_str().to_string())
        .collect()
}

#[test]
fn at_least_one_kilohm_keeps_canonical_and_legacy_and_drops_100_ohm() {
    let (mut state, root) = fixture("Resistors", resistors());
    set_filter(
        browser_filters(&mut state, &root),
        Unit::Ohm,
        RangeOp::AtLeast,
        &[("1", SiPrefix::Kilo)],
    );
    state.sync_view_caches();

    let pns = browser_pns(&state, &root, "Resistors");
    assert_eq!(pns, ["R-1K", "R-4K7", "R-4K7-LEGACY"]);
}

#[test]
fn between_ten_nanofarad_and_one_microfarad_on_capacitors() {
    let caps = vec![
        row("C-1P", Some(measured(1.0, "pF"))),
        row("C-10N", Some(measured(10e-9, "F"))),
        row("C-100N", Some(measured(100.0, "nF"))),
        row("C-1U", Some(measured(1.0, "µF"))),
        row("C-10U", Some(measured(10e-6, "F"))),
    ];
    let (mut state, root) = fixture("Capacitors", caps);
    set_filter(
        browser_filters(&mut state, &root),
        Unit::Farad,
        RangeOp::Between,
        &[("10", SiPrefix::Nano), ("1", SiPrefix::Micro)],
    );
    state.sync_view_caches();

    let pns = browser_pns(&state, &root, "Capacitors");
    assert_eq!(pns, ["C-10N", "C-100N", "C-1U"]);
}

#[test]
fn equals_four_point_seven_kilohm_matches_both_stored_forms() {
    let (mut state, root) = fixture("Resistors", resistors());
    set_filter(
        browser_filters(&mut state, &root),
        Unit::Ohm,
        RangeOp::Equals,
        &[("4.7", SiPrefix::Kilo)],
    );
    state.sync_view_caches();

    let pns = browser_pns(&state, &root, "Resistors");
    assert_eq!(pns, ["R-4K7", "R-4K7-LEGACY"]);
}

#[test]
fn letters_in_the_number_field_are_refused_with_a_reason() {
    let mut filters = RangeFilters::default();
    set_filter(
        &mut filters,
        Unit::Ohm,
        RangeOp::AtLeast,
        &[("4", SiPrefix::One)],
    );
    for typed in ["4k7", "10k", "4.7uF", "1e3", "4,7"] {
        filters.apply(RangeFilterMsg::NumberEdited {
            key: "value".into(),
            bound: RangeBound::Low,
            text: typed.into(),
        });
        let low = &filters.get("value").expect("draft").low;
        assert_eq!(low.text, "4", "{typed:?} must not enter the field");
        assert!(low.refusal.is_some(), "{typed:?} must say why");
    }
    // The reason is the editors' own for letters.
    filters.apply(RangeFilterMsg::NumberEdited {
        key: "value".into(),
        bound: RangeBound::Low,
        text: "4k7".into(),
    });
    let why = filters.get("value").unwrap().low.refusal.clone().unwrap();
    assert!(why.contains("letters"), "{why}");
    // A valid edit clears the refusal and still filters.
    filters.apply(RangeFilterMsg::NumberEdited {
        key: "value".into(),
        bound: RangeBound::Low,
        text: "-4.5".into(),
    });
    let low = &filters.get("value").unwrap().low;
    assert_eq!((low.text.as_str(), low.refusal.as_ref()), ("-4.5", None));
    assert_eq!(filters.active().len(), 1);
}

#[test]
fn a_filter_without_a_finished_number_hides_nothing() {
    let (mut state, root) = fixture("Resistors", resistors());
    let filters = browser_filters(&mut state, &root);
    filters.apply(RangeFilterMsg::ColumnPicked {
        key: "value".into(),
        unit: Unit::Ohm,
    });
    filters.apply(RangeFilterMsg::NumberEdited {
        key: "value".into(),
        bound: RangeBound::Low,
        text: "-".into(),
    });
    state.sync_view_caches();
    assert_eq!(browser_pns(&state, &root, "Resistors").len(), 6);
}

#[test]
fn rows_that_are_not_measurements_are_excluded_and_counted() {
    let (mut state, root) = fixture("Resistors", resistors());
    set_filter(
        browser_filters(&mut state, &root),
        Unit::Ohm,
        RangeOp::AtLeast,
        &[("1", SiPrefix::Kilo)],
    );
    state.sync_view_caches();

    let cache = state.library_browsers[&root].view_cache.as_ref().unwrap();
    // The text cell "10k" and the row with no value; the 100 Ω row is a
    // measurement that is simply below the range, not an exclusion.
    assert_eq!(cache.range_excluded_for_test().get("value"), Some(&2));
}

#[test]
fn the_picker_applies_the_same_filter() {
    let (mut state, _) = fixture("Resistors", resistors());
    set_filter(
        &mut state.picker.as_mut().unwrap().range_filters,
        Unit::Ohm,
        RangeOp::AtLeast,
        &[("1", SiPrefix::Kilo)],
    );
    state.sync_view_caches();

    assert_eq!(picker_pns(&state), ["R-1K", "R-4K7", "R-4K7-LEGACY"]);
    let picker = state.picker.as_ref().unwrap();
    assert_eq!(picker.rows.range_excluded().get("value"), Some(&2));
    let columns: Vec<(&str, &Unit)> = picker
        .rows
        .filter_columns()
        .iter()
        .map(|c| (c.key.as_str(), &c.unit))
        .collect();
    assert_eq!(columns, [("value", &Unit::Ohm)]);
}

#[test]
fn clearing_the_filter_restores_every_row() {
    let (mut state, root) = fixture("Resistors", resistors());
    set_filter(
        browser_filters(&mut state, &root),
        Unit::Ohm,
        RangeOp::AtLeast,
        &[("1", SiPrefix::Kilo)],
    );
    set_filter(
        &mut state.picker.as_mut().unwrap().range_filters,
        Unit::Ohm,
        RangeOp::AtLeast,
        &[("1", SiPrefix::Kilo)],
    );
    state.sync_view_caches();
    assert_eq!(browser_pns(&state, &root, "Resistors").len(), 3);
    assert_eq!(picker_pns(&state).len(), 3);

    let clear = RangeFilterMsg::Cleared {
        key: "value".into(),
    };
    browser_filters(&mut state, &root).apply(clear.clone());
    state.picker.as_mut().unwrap().range_filters.apply(clear);
    state.sync_view_caches();

    assert_eq!(browser_pns(&state, &root, "Resistors").len(), 6);
    assert_eq!(picker_pns(&state).len(), 6);
    let cache = state.library_browsers[&root].view_cache.as_ref().unwrap();
    assert!(cache.range_excluded_for_test().is_empty());
}

#[test]
fn a_prefix_the_unit_lacks_falls_back_to_none() {
    // A stale pick (kilo) after the column's unit has no kilo must not
    // scale the bound: farad takes no kilo prefix.
    let mut filters = RangeFilters::default();
    set_filter(
        &mut filters,
        Unit::Farad,
        RangeOp::AtLeast,
        &[("5", SiPrefix::Kilo)],
    );
    let f = filters.active();
    assert_eq!(f.len(), 1);
    assert_eq!(
        f[0].1.verdict(Some(&measured(5.0, "F"))),
        signex_library::RangeVerdict::Matches
    );
}

#[test]
fn the_bar_draws_in_the_browser_and_the_picker_with_a_between_filter() {
    use signex_types::theme::{ThemeId, theme_tokens};
    let (mut state, root) = fixture("Resistors", resistors());
    let bounds = [("1", SiPrefix::Kilo), ("5", SiPrefix::Kilo)];
    set_filter(
        browser_filters(&mut state, &root),
        Unit::Ohm,
        RangeOp::Between,
        &bounds,
    );
    set_filter(
        &mut state.picker.as_mut().unwrap().range_filters,
        Unit::Ohm,
        RangeOp::Between,
        &bounds,
    );
    state.sync_view_caches();
    let tokens = theme_tokens(ThemeId::Signex);

    drop(crate::library::browser::view(
        &root,
        &state,
        &state.library_browsers[&root],
        &tokens,
    ));
    drop(crate::library::picker::view(
        state.picker.as_ref().unwrap(),
        &tokens,
        false,
    ));
}
