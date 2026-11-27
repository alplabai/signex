//! Parts for the table tests: capacitors (the bundled template's view
//! groups them by package then value and sorts by voltage), op-amps (a
//! min-to-max supply) and one resistor, in one mounted library.

use signex_library::{ComponentClass, ComponentRow, ParamValue};

use super::super::tests::{fixture_row, library_with};
use super::state::CategoryKey;
use crate::library::state::{ComponentsPanelRowKey, LibraryState};

pub(super) fn measured(name: &str, value: f64, unit: &str) -> (String, ParamValue) {
    (
        name.to_owned(),
        ParamValue::Measurement {
            value,
            unit: unit.to_owned(),
        },
    )
}

pub(super) fn part(pn: &str, class: &str, params: Vec<(String, ParamValue)>) -> ComponentRow {
    let mut row = fixture_row(pn, "", "Mfr");
    row.class = ComponentClass::new(class);
    for (name, value) in params {
        row.parameters.insert(name, value);
    }
    row
}

pub(super) fn capacitor(pn: &str, farad: f64, volt: f64, package: &str) -> ComponentRow {
    part(
        pn,
        "capacitor",
        vec![
            measured("value", farad, "F"),
            measured("voltage_max", volt, "V"),
            ("package".into(), ParamValue::Text(package.into())),
        ],
    )
}

pub(super) fn opamp(pn: &str, low: Option<f64>, high: Option<f64>) -> ComponentRow {
    let mut params = Vec::new();
    if let Some(low) = low {
        params.push(measured("supply_min", low, "V"));
    }
    if let Some(high) = high {
        params.push(measured("supply_max", high, "V"));
    }
    part(pn, "opamp", params)
}

/// Five capacitors, three op-amps (one with only its low supply end) and a
/// resistor, caches current.
pub(super) fn parts() -> LibraryState {
    library_with(vec![
        capacitor("C1", 100e-9, 16.0, "0402"),
        capacitor("C2", 100e-9, 50.0, "0402"),
        capacitor("C3", 1e-6, 25.0, "0603"),
        capacitor("C4", 100e-12, 50.0, "0603"),
        part("C5", "capacitor", vec![measured("value", 10e-6, "F")]),
        opamp("U1", Some(3.0), Some(3.6)),
        opamp("U2", Some(4.5), Some(5.5)),
        opamp("U3", Some(2.7), None),
        part("R1", "resistor", vec![measured("value", 4700.0, "ohm")]),
    ])
}

/// Show the category `category` names (`*`, `@recent` or a class), as the
/// picker does, and rebuild the caches.
pub(super) fn show(state: &mut LibraryState, category: &str) {
    let key = CategoryKey::from_pref_key(category);
    let _ = state
        .components_panel
        .table
        .apply(crate::library::messages::ComponentsTableMsg::CategoryPicked(key));
    state.sync_view_caches();
}

/// The part numbers of the drawn rows, top to bottom.
pub(super) fn drawn_parts(state: &LibraryState) -> Vec<String> {
    let table = &state.components_panel.table;
    table
        .cache
        .lines
        .iter()
        .filter_map(|line| match line {
            super::TableLine::Row { row, .. } => {
                let r = &table.cache.base[*row];
                Some(
                    r.row(&state.open_libraries)?
                        .internal_pn
                        .as_str()
                        .to_owned(),
                )
            }
            super::TableLine::Group { .. } => None,
        })
        .collect()
}

/// The key of the part numbered `pn`.
pub(super) fn key_of(state: &LibraryState, pn: &str) -> ComponentsPanelRowKey {
    let lib = &state.open_libraries[0];
    let (table, rows) = lib.tables.iter().next().expect("one table");
    let row = rows
        .iter()
        .find(|r| r.internal_pn.as_str() == pn)
        .expect("the part");
    ComponentsPanelRowKey {
        library_path: lib.root.clone(),
        table: table.clone(),
        row_id: signex_library::RowId(row.row_id),
    }
}
