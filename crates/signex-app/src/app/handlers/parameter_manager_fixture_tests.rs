//! Advanced Schematic exit criterion 4 (`docs/ROADMAP.md`) on a project on
//! disk: a Parameter Manager edit across several sheets of
//! `nested-harness` (`mcu`, `sensor` and `connectors`, each placed once) is
//! one undo step, and afterwards the Object List and the Parameter Manager
//! show the values the canvas draws.
//!
//! Written against batch A's Parameter Manager scope (#1784, the All sheets
//! switch and `ParameterManagerMsg::Edit { sheet, .. }`), so it is not in the
//! module tree on trunk. After batch A (#2933) merges, add beside its
//! `scope_tests` in `handlers/parameter_manager.rs`:
//!
//! ```text
//! #[cfg(test)]
//! #[path = "parameter_manager_fixture_tests.rs"]
//! mod fixture_tests;
//! ```

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::app::contracts::ParameterScope;
use crate::app::handlers::menu::export::tests::exit_fixtures::{fixture_dir, open_fixture};
use crate::app::view::parameter_manager::{TableBody, table_body};
use crate::app::{EditMsg, Message, ParameterManagerMsg as Pm, Signex, TabInfo, TabKind};
use crate::panels::object_list::model::{ObjectKind, RowName, RowValue, sheet_rows};
use crate::schematic_runtime::RenderInvalidation;

const PROJECT: &str = "nested-harness";
const SHEETS: [&str; 3] = ["mcu.snxsch", "sensor.snxsch", "connectors.snxsch"];

fn path(sheet: &str) -> PathBuf {
    fixture_dir(PROJECT).join(sheet)
}

/// The fixture with a tab per sheet, `mcu` shown, the manager open on All
/// sheets.
fn app() -> Signex {
    let mut app = open_fixture(PROJECT);
    app.ui_state.first_run_tour_open = false;
    let project = app.document_state.active_project;
    for sheet in SHEETS {
        let path = path(sheet);
        app.document_state.tabs.push(TabInfo {
            title: path.file_stem().unwrap().to_string_lossy().into_owned(),
            path,
            project_id: project,
            kind: TabKind::Schematic,
        });
    }
    show(&mut app, 0);
    let _ = app.update(Message::ParameterManager(Pm::Open));
    let _ = app.update(Message::ParameterManager(Pm::ScopePicked(
        ParameterScope::AllSheets,
    )));
    app
}

/// Show the tab at `index`, as a tab switch does.
fn show(app: &mut Signex, index: usize) {
    app.document_state.active_tab = index;
    app.document_state.active_path = Some(app.document_state.tabs[index].path.clone());
    app.sync_canvas_from_visible_schematic(RenderInvalidation::FULL);
    app.refresh_canvas_designators();
}

fn uuid_of(app: &Signex, sheet: &str, reference: &str) -> Uuid {
    app.document_state.engines[&path(sheet)]
        .document()
        .symbols
        .iter()
        .find(|s| s.reference == reference)
        .unwrap_or_else(|| panic!("{reference} on {sheet}"))
        .uuid
}

fn field(app: &Signex, sheet: &str, reference: &str, key: &str) -> Option<String> {
    let id = uuid_of(app, sheet, reference);
    app.document_state.engines[&path(sheet)]
        .document()
        .symbols
        .iter()
        .find(|s| s.uuid == id)
        .and_then(|s| s.fields.get(key).cloned())
}

const PARTS: [(&str, &str); 3] = [
    ("mcu.snxsch", "U3"),
    ("sensor.snxsch", "U1"),
    ("connectors.snxsch", "J2"),
];

fn mpn(app: &Signex) -> Vec<Option<String>> {
    PARTS
        .iter()
        .map(|(sheet, part)| field(app, sheet, part, "MPN"))
        .collect()
}

fn add_mpn_column(app: &mut Signex) {
    let _ = app.update(Message::ParameterManager(Pm::NewKeyChanged(
        "MPN".to_string(),
    )));
    let _ = app.update(Message::ParameterManager(Pm::ColumnAdded));
}

#[test]
fn a_column_added_across_the_fixtures_sheets_is_one_undo_step() {
    let mut app = app();
    add_mpn_column(&mut app);
    let empty = || Some(String::new());
    assert_eq!(mpn(&app), [empty(), empty(), empty()]);

    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(
        mpn(&app),
        [None, None, None],
        "one Undo takes every sheet back"
    );
    let _ = app.update(Message::Edit(EditMsg::Redo));
    assert_eq!(mpn(&app), [empty(), empty(), empty()]);
}

#[test]
fn cells_typed_on_the_other_sheets_from_the_root_tab_land_there() {
    let mut app = app();
    add_mpn_column(&mut app);
    for (sheet, part, value) in [
        ("sensor.snxsch", "U1", "BME280"),
        ("connectors.snxsch", "J2", "HDR-1x4"),
    ] {
        let symbol_uuid = uuid_of(&app, sheet, part);
        let _ = app.update(Message::ParameterManager(Pm::Edit {
            sheet: path(sheet),
            symbol_uuid,
            key: "MPN".to_string(),
            value: value.to_string(),
        }));
    }
    assert_eq!(
        mpn(&app),
        [
            Some(String::new()),
            Some("BME280".to_string()),
            Some("HDR-1x4".to_string())
        ]
    );
    assert_lists_show_what_the_canvas_draws(&mut app);
}

/// On each sheet in turn: the Object List's part rows and the Parameter
/// Manager's rows name each part and give its value (and, for the table,
/// its parameters) as the canvas draws them.
fn assert_lists_show_what_the_canvas_draws(app: &mut Signex) {
    for (index, sheet) in SHEETS.iter().enumerate() {
        show(app, index);
        let canvas = app.active_render_snapshot().expect("the sheet is drawn");
        let drawn: Vec<(String, String, HashMap<String, String>)> = (canvas.symbols.iter())
            .filter(|s| !s.is_power)
            .map(|s| {
                let shown = app
                    .interaction_state
                    .active_canvas()
                    .designators
                    .get(&s.uuid);
                let reference = shown.cloned().unwrap_or_else(|| s.reference.clone());
                (reference, s.value.clone(), s.fields.clone())
            })
            .collect();

        let file = path(sheet);
        let doc = app.document_state.engines[&file].document();
        let look = app.sheet_look(&file, None);
        let listed: Vec<(String, String)> =
            sheet_rows(doc, &file, &look, &|_: Uuid| None::<String>)
                .into_iter()
                .filter(|row| row.kind == ObjectKind::Part)
                .map(|row| match (row.name, row.value) {
                    (RowName::Text(name), RowValue::Text(value)) => (name, value),
                    other => panic!("a part row names its part: {other:?}"),
                })
                .collect();
        let pairs: Vec<(String, String)> = (drawn.iter())
            .map(|(reference, value, _)| (reference.clone(), value.clone()))
            .collect();
        assert_eq!(listed, pairs, "{sheet}: Object List against the canvas");

        let sheets = app.parameter_documents();
        let TableBody::Rows(rows) = table_body(&app.ui_state.i18n, "", &sheets) else {
            panic!("no filter lists every part");
        };
        let table: Vec<(String, String, HashMap<String, String>)> = (rows.iter())
            .filter(|row| row.sheet == Path::new(&file))
            .map(|row| {
                let s = row.symbol;
                (row.reference.to_string(), s.value.clone(), s.fields.clone())
            })
            .collect();
        assert_eq!(
            table, drawn,
            "{sheet}: Parameter Manager against the canvas"
        );
    }
}
