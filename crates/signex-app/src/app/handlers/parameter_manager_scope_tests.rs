//! #1784: the Parameter Manager across sheets. The header's Active sheet /
//! All sheets switch (Active sheet by default), the rows of every open sheet
//! of the project, one undo step for an edit that changes several sheets,
//! and the exit criterion that the Object List and the table show the
//! values the canvas draws (Caner 2026-11-25).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use signex_types::schematic::{SchematicSheet, Symbol};
use uuid::Uuid;

use crate::app::contracts::ParameterScope;
use crate::app::view::parameter_manager::{TableBody, table_body};
use crate::app::{
    EditMsg, LoadedProject, Message, ParameterManagerMsg as Pm, ProjectId, Signex, TabInfo, TabKind,
};
use crate::panels::object_list::model::{ObjectKind, RowName, RowValue, sheet_rows};
use crate::schematic_runtime::RenderInvalidation;

const A: &str = "/w/a.snxsch";
const B: &str = "/w/b.snxsch";
const LOOSE: &str = "/x/loose.snxsch";

/// Project P in `/w`, listing sheets a and b.
const PROJECT: &str = r#"{"name":"P","dir":"/w","schematic_root":"a.snxsch","pcb_file":null,
"sheets":[
{"name":"a.snxsch","filename":"a.snxsch","symbols_count":0,"wires_count":0,"labels_count":0},
{"name":"b.snxsch","filename":"b.snxsch","symbols_count":0,"wires_count":0,"labels_count":0}]}"#;

fn part(n: u128, reference: &str, tolerance: &str) -> Symbol {
    let mut sym = Symbol {
        uuid: Uuid::from_u128(n),
        lib_id: "Device:R".to_string(),
        reference: reference.to_string(),
        value: "10k".to_string(),
        footprint: "R_0603".to_string(),
        ..Default::default()
    };
    sym.fields
        .insert("Tolerance".to_string(), tolerance.to_string());
    sym
}

fn open(app: &mut Signex, path: &str, symbols: Vec<Symbol>, project: Option<ProjectId>) {
    let sheet = SchematicSheet {
        uuid: Uuid::now_v7(),
        paper_size: "A4".to_string(),
        symbols,
        ..Default::default()
    };
    let path = PathBuf::from(path);
    let engine = signex_engine::Engine::new(sheet).expect("engine");
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.tabs.push(TabInfo {
        title: path.file_stem().unwrap().to_string_lossy().into_owned(),
        path,
        cached_document: None,
        project_id: project,
        kind: TabKind::Schematic,
    });
}

/// Show the tab at `index` in the main window, as a tab switch does.
fn show(app: &mut Signex, index: usize) {
    app.document_state.active_tab = index;
    app.document_state.active_path = Some(app.document_state.tabs[index].path.clone());
    app.sync_canvas_from_visible_schematic(RenderInvalidation::FULL);
}

/// Sheets a (R1, R2) and b (R3) of project P and a loose sheet (R9) open,
/// a shown, the manager open.
fn app() -> Signex {
    let (mut app, _task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    let prj = Path::new("/w/P.snxprj");
    let data =
        signex_types::project::parse_project_bytes(prj, PROJECT.as_bytes()).expect("project");
    let id = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id,
        path: prj.to_path_buf(),
        data,
        pending_libraries: HashMap::new(),
        case_fold: Default::default(),
        case_probed: false,
        case_decided: false,
    });
    app.document_state.active_project = Some(id);
    open(
        &mut app,
        A,
        vec![part(1, "R1", "1%"), part(2, "R2", "5%")],
        Some(id),
    );
    open(&mut app, B, vec![part(3, "R3", "10%")], Some(id));
    open(&mut app, LOOSE, vec![part(4, "R9", "")], None);
    show(&mut app, 0);
    let _ = app.update(Message::ParameterManager(Pm::Open));
    app
}

fn all_sheets(app: &mut Signex) {
    let _ = app.update(Message::ParameterManager(Pm::ScopePicked(
        ParameterScope::AllSheets,
    )));
}

fn field(app: &Signex, sheet: &str, n: u128, key: &str) -> Option<String> {
    app.document_state.engines[Path::new(sheet)]
        .document()
        .symbols
        .iter()
        .find(|s| s.uuid == Uuid::from_u128(n))
        .and_then(|s| s.fields.get(key).cloned())
}

fn typed(app: &mut Signex, sheet: &str, n: u128, value: &str) {
    let _ = app.update(Message::ParameterManager(Pm::Edit {
        sheet: sheet.into(),
        symbol_uuid: Uuid::from_u128(n),
        key: "Tolerance".to_string(),
        value: value.to_string(),
    }));
}

fn undo(app: &mut Signex) {
    let _ = app.update(Message::Edit(EditMsg::Undo));
}

fn paths(sheets: &[&str]) -> Vec<PathBuf> {
    sheets.iter().map(PathBuf::from).collect()
}

#[test]
fn active_sheet_is_the_default_and_all_sheets_adds_the_projects_other_open_sheets() {
    let mut app = app();
    assert_eq!(
        app.ui_state.parameter_table.scope,
        ParameterScope::ActiveSheet
    );
    assert_eq!(app.parameter_sheets(), paths(&[A]));

    all_sheets(&mut app);
    assert_eq!(
        app.parameter_sheets(),
        paths(&[A, B]),
        "the loose sheet is not part of P"
    );

    let _ = app.update(Message::ParameterManager(Pm::Close));
    let _ = app.update(Message::ParameterManager(Pm::Open));
    assert_eq!(
        app.ui_state.parameter_table.scope,
        ParameterScope::ActiveSheet,
        "every opening starts on Active sheet"
    );
}

#[test]
fn the_subtitle_names_the_sheet_or_counts_the_sheets() {
    let mut app = app();
    assert_eq!(
        app.parameter_manager_subtitle().as_deref(),
        Some("a.snxsch · 2 parts · 1 parameter")
    );
    all_sheets(&mut app);
    assert_eq!(
        app.parameter_manager_subtitle().as_deref(),
        Some("2 sheets · 3 parts · 1 parameter")
    );
}

#[test]
fn all_sheets_on_a_loose_sheet_lists_no_project_sheet() {
    let mut app = app();
    show(&mut app, 2);
    all_sheets(&mut app);
    assert_eq!(app.parameter_sheets(), paths(&[LOOSE]));
}

#[test]
fn all_sheets_lists_the_parts_of_every_listed_sheet_with_their_sheet() {
    let mut app = app();
    all_sheets(&mut app);
    let sheets = app.parameter_documents();
    let TableBody::Rows(rows) = table_body(&app.ui_state.i18n, "", &sheets) else {
        panic!("no filter lists every part");
    };
    let listed: Vec<(&Path, &str)> = rows
        .iter()
        .map(|row| (row.sheet, row.symbol.reference.as_str()))
        .collect();
    let (a, b) = (Path::new(A), Path::new(B));
    assert_eq!(listed, [(a, "R1"), (a, "R2"), (b, "R3")]);
}

#[test]
fn a_cell_of_another_sheet_is_written_there_as_one_undo_step_of_that_sheet() {
    let mut app = app();
    all_sheets(&mut app);
    typed(&mut app, B, 3, "1");
    typed(&mut app, B, 3, "12%");
    assert_eq!(field(&app, B, 3, "Tolerance").as_deref(), Some("12%"));
    assert_eq!(field(&app, A, 1, "Tolerance").as_deref(), Some("1%"));
    assert!(app.document_state.is_document_dirty(Path::new(B)));
    assert!(!app.document_state.is_document_dirty(Path::new(A)));
    assert_eq!(app.parameter_session_changes().len(), 1);

    show(&mut app, 1);
    undo(&mut app);
    assert_eq!(
        field(&app, B, 3, "Tolerance").as_deref(),
        Some("10%"),
        "the keystrokes in one cell are one undo step on its sheet"
    );
}

#[test]
fn a_column_added_across_sheets_is_one_undo_step() {
    let mut app = app();
    all_sheets(&mut app);
    let _ = app.update(Message::ParameterManager(Pm::NewKeyChanged(
        "MPN".to_string(),
    )));
    let _ = app.update(Message::ParameterManager(Pm::ColumnAdded));
    let mpn = |app: &Signex| [(A, 1), (A, 2), (B, 3)].map(|(sheet, n)| field(app, sheet, n, "MPN"));
    let empty = || Some(String::new());
    assert_eq!(mpn(&app), [empty(), empty(), empty()]);
    assert_eq!(
        field(&app, LOOSE, 4, "MPN"),
        None,
        "the loose sheet is not listed"
    );
    assert!(app.ui_state.parameter_table.new_key.is_empty());

    undo(&mut app);
    assert_eq!(
        mpn(&app),
        [None, None, None],
        "one Undo takes both sheets back"
    );
    let _ = app.update(Message::Edit(EditMsg::Redo));
    assert_eq!(mpn(&app), [empty(), empty(), empty()]);
}

#[test]
fn a_column_removed_across_sheets_is_one_undo_step() {
    let mut app = app();
    all_sheets(&mut app);
    let _ = app.update(Message::ParameterManager(Pm::ParameterRemoved {
        key: "Tolerance".to_string(),
    }));
    assert_eq!(field(&app, A, 1, "Tolerance"), None);
    assert_eq!(field(&app, B, 3, "Tolerance"), None);
    assert_eq!(field(&app, LOOSE, 4, "Tolerance").as_deref(), Some(""));

    undo(&mut app);
    assert_eq!(field(&app, A, 1, "Tolerance").as_deref(), Some("1%"));
    assert_eq!(field(&app, B, 3, "Tolerance").as_deref(), Some("10%"));
}

#[test]
fn revert_across_sheets_is_one_undo_step() {
    let mut app = app();
    all_sheets(&mut app);
    typed(&mut app, A, 1, "2%");
    typed(&mut app, B, 3, "20%");
    assert_eq!(app.parameter_session_changes().len(), 2);

    let _ = app.update(Message::ParameterManager(Pm::SessionReverted));
    assert_eq!(field(&app, A, 1, "Tolerance").as_deref(), Some("1%"));
    assert_eq!(field(&app, B, 3, "Tolerance").as_deref(), Some("10%"));
    assert!(app.parameter_session_changes().is_empty());

    undo(&mut app);
    assert_eq!(field(&app, A, 1, "Tolerance").as_deref(), Some("2%"));
    assert_eq!(
        field(&app, B, 3, "Tolerance").as_deref(),
        Some("20%"),
        "one Undo takes the whole revert back on both sheets"
    );
}

#[test]
fn with_active_sheet_a_column_stays_on_the_active_sheet() {
    let mut app = app();
    let _ = app.update(Message::ParameterManager(Pm::NewKeyChanged(
        "MPN".to_string(),
    )));
    let _ = app.update(Message::ParameterManager(Pm::ColumnAdded));
    assert_eq!(field(&app, A, 1, "MPN").as_deref(), Some(""));
    assert_eq!(field(&app, B, 3, "MPN"), None);
}

#[test]
fn the_changed_count_follows_the_scope() {
    let mut app = app();
    all_sheets(&mut app);
    typed(&mut app, A, 1, "2%");
    typed(&mut app, B, 3, "20%");
    let _ = app.update(Message::ParameterManager(Pm::ScopePicked(
        ParameterScope::ActiveSheet,
    )));
    assert_eq!(
        app.parameter_session_changes().len(),
        1,
        "Active sheet counts the active sheet's cells"
    );
}

/// Exit criterion (Advanced Schematic): the Object List and the Parameter
/// Manager show the values the canvas draws, after edits across sheets.
#[test]
fn the_object_list_and_the_parameter_manager_show_what_the_canvas_draws() {
    let mut app = app();
    all_sheets(&mut app);
    typed(&mut app, A, 2, "0.1%");
    typed(&mut app, B, 3, "20%");
    let _ = app.update(Message::ParameterManager(Pm::NewKeyChanged(
        "MPN".to_string(),
    )));
    let _ = app.update(Message::ParameterManager(Pm::ColumnAdded));

    for (index, sheet) in [(0, A), (1, B)] {
        if index != 0 {
            show(&mut app, index);
        }
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

        let path = Path::new(sheet);
        let doc = app.document_state.engines[path].document();
        let listed: Vec<(String, String)> = sheet_rows(doc, path, &|_: Uuid| None::<String>)
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
            .filter(|row| row.sheet == path)
            .map(|row| {
                let s = row.symbol;
                (s.reference.clone(), s.value.clone(), s.fields.clone())
            })
            .collect();
        assert_eq!(
            table, drawn,
            "{sheet}: Parameter Manager against the canvas"
        );
    }
}
