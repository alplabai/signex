//! A library's own class template wins over the global one for its rows
//! (Caner, 2026-11-27); a column only it offers names that library.

use std::path::PathBuf;
use std::sync::Arc;

use signex_i18n::I18n;
use signex_library::TemplateRegistry;
use signex_library::templates::{ColumnKey, RangeSlot};
use uuid::Uuid;

use super::super::tests::library_with;
use super::test_fixture::{capacitor, measured, show};
use crate::library::state::LibraryState;

/// Library `a` reads the global capacitor template; library `b` has its
/// own, which pairs `temp_min` / `temp_max` into a Temperature range.
fn two_libraries() -> LibraryState {
    let mut state = library_with(vec![capacitor("C1", 100e-9, 16.0, "0402")]);
    let mut cb1 = capacitor("CB1", 1e-6, 25.0, "0603");
    cb1.parameters.extend([
        measured("temp_min", -40.0, "degC"),
        measured("temp_max", 125.0, "degC"),
    ]);
    let mut b = library_with(vec![cb1]).open_libraries.remove(0);
    b.root = PathBuf::from("b.snxlib");
    b.display_name = "b".into();
    b.library_id = Uuid::from_u128(0xb);
    state.open_libraries.push(b);
    let mut registry = TemplateRegistry::new_with_builtins();
    let mut own = registry
        .resolve(Uuid::nil(), "capacitor")
        .expect("bundled capacitor")
        .clone();
    own.ranges.push(RangeSlot {
        name: "temp".into(),
        min: "temp_min".into(),
        max: "temp_max".into(),
        sort_by: Default::default(),
    });
    registry.insert_for_library(Uuid::from_u128(0xb), own);
    state.template_registry = Arc::new(registry);
    state.touch_data();
    state.sync_view_caches();
    state
}

#[test]
fn a_librarys_own_template_reads_its_rows_and_names_its_columns() {
    let mut state = two_libraries();
    show(&mut state, "capacitor");
    let cache = &state.components_panel.table.cache;
    let temp = ColumnKey::param("temp");
    assert!(cache.offered.contains(&temp), "b's template adds Temp");
    assert_eq!(
        cache.column_origin.get(&temp).map(String::as_str),
        Some("b")
    );
    assert!(
        !cache.column_origin.contains_key(&ColumnKey::param("value")),
        "both templates offer Value"
    );
    assert!(cache.is_range("temp"));

    let i18n = I18n::en_us();
    let text = |pn: &str| {
        let r = cache
            .base
            .iter()
            .find(|r| r.row(&state.open_libraries).unwrap().internal_pn.as_str() == pn)
            .unwrap();
        super::cells::cell_text(
            r.row(&state.open_libraries).unwrap(),
            &r.facts,
            cache.template_of(r),
            &temp,
            &i18n,
        )
    };
    assert_eq!(text("CB1").as_deref(), Some("\u{2212}40 to 125 °C"));
    assert_eq!(text("C1"), None, "the global template has no Temp pair");
    assert_eq!(
        super::popovers::column_label(&temp, Some("b"), &i18n),
        "Temp (b template)"
    );
}

/// Details of a part read its own library's template: b's part lists its
/// Temperature pair.
#[test]
fn details_read_the_parts_own_template() {
    let mut state = two_libraries();
    let lib = &state.open_libraries[1];
    let row = &lib.tables["t"][0];
    state.components_panel.selected = Some(crate::library::state::ComponentsPanelRowKey {
        library_path: lib.root.clone(),
        table: "t".into(),
        row_id: signex_library::RowId(row.row_id),
    });
    state.sync_view_caches();
    let details = &state.components_panel.table.cache.details;
    assert!(details.parameters.iter().any(|e| e.name == "temp"));
}
