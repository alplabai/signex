//! #1784: the Parameter Manager across sheets. The header's Active sheet /
//! All sheets switch (Active sheet by default), the rows of every open sheet
//! of the project, one undo step for an edit that changes several sheets,
//! and the exit criterion that the Object List and the table show the
//! values the canvas draws (Caner 2026-11-25).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use signex_types::schematic::{InstancePath, SchematicSheet, Symbol, SymbolInstance};
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
        project_id: project,
        kind: TabKind::Schematic,
    });
}

/// Show the tab at `index` in the main window, as a tab switch does.
fn show(app: &mut Signex, index: usize) {
    app.document_state.active_tab = index;
    app.document_state.active_path = Some(app.document_state.tabs[index].path.clone());
    app.sync_canvas_from_visible_schematic(RenderInvalidation::FULL);
    // As a tab switch does, so the canvas names its parts by placement.
    app.refresh_canvas_designators();
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

/// Until the closed sheets are read, All sheets lists the open sheets only; a
/// sheet of the project that is not open is counted beside the switch
/// instead of being left out quietly.
#[test]
fn all_sheets_counts_the_project_sheets_it_cannot_list() {
    let mut app = app();
    all_sheets(&mut app);
    assert_eq!(app.closed_parameter_sheets(), 0, "a and b are both open");

    app.document_state.engines.remove(Path::new(B));
    app.document_state
        .tabs
        .retain(|tab| tab.path != Path::new(B));
    assert_eq!(app.parameter_sheets(), paths(&[A]));
    assert_eq!(app.closed_parameter_sheets(), 1, "b is closed");

    let _ = app.update(Message::ParameterManager(Pm::ScopePicked(
        ParameterScope::ActiveSheet,
    )));
    assert_eq!(
        app.closed_parameter_sheets(),
        0,
        "Active sheet lists a only"
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
    edit_across_sheets(&mut app);
    assert_lists_show_what_the_canvas_draws(&mut app);
}

/// The exit criterion on a child sheet placed twice (#1151): b's R3 is R103
/// and R203 by placement, and its tab shows the R203 placement. The canvas,
/// the Object List, the Parameter Manager and its CSV export all name it
/// R203, never the stored R3.
#[test]
fn on_a_placed_child_sheet_every_list_names_the_placement_designator() {
    let mut app = app();
    let first = InstancePath::from_chain(vec![Uuid::from_u128(100)]);
    let second = InstancePath::from_chain(vec![Uuid::from_u128(200)]);
    let mut r3 = part(3, "R3", "10%");
    r3.instances = [(&first, "R103"), (&second, "R203")]
        .into_iter()
        .map(|(placement, reference)| SymbolInstance {
            project: "P".into(),
            path: placement.to_string(),
            reference: reference.into(),
            unit: 1,
        })
        .collect();
    let child = SchematicSheet {
        uuid: Uuid::now_v7(),
        paper_size: "A4".to_string(),
        symbols: vec![r3],
        ..Default::default()
    };
    let engine = signex_engine::Engine::new(child).expect("engine");
    app.document_state.engines.insert(PathBuf::from(B), engine);
    app.document_state
        .tab_placements
        .record(&PathBuf::from(B), Some(second));
    edit_across_sheets(&mut app);
    assert_lists_show_what_the_canvas_draws(&mut app);

    show(&mut app, 1);
    assert_eq!(
        app.interaction_state
            .active_canvas()
            .designators
            .get(&Uuid::from_u128(3))
            .map(String::as_str),
        Some("R203"),
        "the canvas draws the placement's designator"
    );
    let csv = String::from_utf8(
        app.listed_parameter_csv()
            .expect("a sheet")
            .expect("export"),
    )
    .expect("utf-8");
    assert!(
        csv.contains("00000000-0000-0000-0000-000000000003,R203,b.snxsch,"),
        "the export names R3 as the table does: {csv}"
    );
}

/// With All sheets: a cell typed on a and on b, and a column added to both.
fn edit_across_sheets(app: &mut Signex) {
    all_sheets(app);
    typed(app, A, 2, "0.1%");
    typed(app, B, 3, "20%");
    let _ = app.update(Message::ParameterManager(Pm::NewKeyChanged(
        "MPN".to_string(),
    )));
    let _ = app.update(Message::ParameterManager(Pm::ColumnAdded));
}

/// On a and on b in turn: the Object List's part rows and the Parameter
/// Manager's rows name each part and give its value (and, for the table,
/// its parameters) as the canvas draws them.
fn assert_lists_show_what_the_canvas_draws(app: &mut Signex) {
    for (index, sheet) in [(0, A), (1, B)] {
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

        let path = Path::new(sheet);
        let doc = app.document_state.engines[path].document();
        let look = app.sheet_look(path, None);
        let listed: Vec<(String, String)> = sheet_rows(doc, path, &look, &|_: Uuid| None::<String>)
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
                (row.reference.to_string(), s.value.clone(), s.fields.clone())
            })
            .collect();
        assert_eq!(
            table, drawn,
            "{sheet}: Parameter Manager against the canvas"
        );
    }
}

fn add_mpn_column(app: &mut Signex) {
    let _ = app.update(Message::ParameterManager(Pm::NewKeyChanged(
        "MPN".to_string(),
    )));
    let _ = app.update(Message::ParameterManager(Pm::ColumnAdded));
}

/// The Messages lines said since `before`, without the bidi isolates.
fn said_since(before: Option<u64>) -> Vec<String> {
    crate::diagnostics::entries_since(before)
        .entries
        .into_iter()
        .map(|e| e.message.replace(['\u{2068}', '\u{2069}'], ""))
        .collect()
}

/// An edit across sheets is all or nothing: b opening read-only refuses the
/// column on a too, and Messages says why.
#[test]
fn a_read_only_sheet_refuses_the_whole_cross_sheet_edit() {
    let _ = crate::diagnostics::init_logging();
    let mut app = app();
    let mut newer = SchematicSheet {
        uuid: Uuid::now_v7(),
        symbols: vec![part(3, "R3", "10%")],
        ..Default::default()
    };
    newer.unknown.newer_minor_source = Some("snxsch/1.99".into());
    let engine = signex_engine::Engine::new(newer).expect("engine");
    app.document_state.engines.insert(PathBuf::from(B), engine);
    all_sheets(&mut app);
    let before = crate::diagnostics::newest_entry_id();
    add_mpn_column(&mut app);

    assert_eq!(field(&app, A, 1, "MPN"), None, "a is left as it was");
    assert_eq!(field(&app, B, 3, "MPN"), None);
    assert!(!app.document_state.is_document_dirty(Path::new(A)));
    assert_eq!(
        app.ui_state.parameter_table.new_key, "MPN",
        "the typed name stays for another try"
    );
    let lines = said_since(before);
    assert!(
        lines
            .iter()
            .any(|line| line.contains("b.snxsch opens read-only")),
        "{lines:?}"
    );
}

/// One Undo on any sheet of a cross-sheet edit takes it back on all of
/// them, and one Redo there puts it back.
#[test]
fn one_undo_on_the_other_sheet_takes_a_cross_sheet_column_back() {
    let mut app = app();
    all_sheets(&mut app);
    add_mpn_column(&mut app);
    show(&mut app, 1);
    undo(&mut app);
    assert_eq!(field(&app, A, 1, "MPN"), None, "a is taken back from b");
    assert_eq!(field(&app, B, 3, "MPN"), None);
    let _ = app.update(Message::Edit(EditMsg::Redo));
    assert_eq!(field(&app, A, 1, "MPN").as_deref(), Some(""));
    assert_eq!(field(&app, B, 3, "MPN").as_deref(), Some(""));
}

/// A listed sheet shown as a channel tab (#1279) is a compiled view, active
/// or not: its cells and an edit across sheets that reaches it are refused.
#[test]
fn a_sheet_shown_as_a_channel_refuses_edits_from_the_table() {
    let _ = crate::diagnostics::init_logging();
    let mut app = app();
    all_sheets(&mut app);
    let channels = &mut app.ui_state.compiled_design.channel_tabs;
    channels.insert(PathBuf::from(B), 1);
    let said = crate::diagnostics::said_on_this_thread();
    typed(&mut app, B, 3, "12%");
    assert_eq!(field(&app, B, 3, "Tolerance").as_deref(), Some("10%"));
    assert!(
        crate::diagnostics::said_on_this_thread() > said,
        "Messages says why"
    );

    add_mpn_column(&mut app);
    assert_eq!(
        field(&app, A, 1, "MPN"),
        None,
        "nothing changed on a either"
    );
    assert_eq!(field(&app, B, 3, "MPN"), None);
}

/// Sheet b closed, All sheets picked, and the closed sheets' read landing
/// with b (R3, Tolerance 10%) under the generation the pick started.
fn with_closed_b_read() -> Signex {
    let mut app = app();
    app.document_state.engines.remove(Path::new(B));
    app.document_state
        .tabs
        .retain(|tab| tab.path != Path::new(B));
    all_sheets(&mut app);
    assert!(
        app.ui_state.parameter_table.closed.reading,
        "b is being read"
    );
    let b = SchematicSheet {
        uuid: Uuid::now_v7(),
        paper_size: "A4".to_string(),
        symbols: vec![part(3, "R3", "10%")],
        ..Default::default()
    };
    let generation = app.ui_state.parameter_table.closed.generation;
    let _ = app.update(Message::ParameterManager(Pm::ClosedSheetsRead {
        generation,
        sheets: std::sync::Arc::new(vec![(PathBuf::from(B), b)]),
        unread: Vec::new(),
    }));
    app
}

/// Item 1 of the Advanced Schematic follow-up (Caner 2026-11-25): All sheets
/// lists the closed sheets too, read from disk in a task, after the open
/// ones; their rows are read-only and the scope bar says so.
#[test]
fn all_sheets_lists_a_closed_sheet_read_only_once_it_is_read() {
    let mut app = app();
    app.document_state.engines.remove(Path::new(B));
    app.document_state
        .tabs
        .retain(|tab| tab.path != Path::new(B));
    all_sheets(&mut app);
    let reading = app.parameter_scope_notes();
    assert_eq!(reading.len(), 1, "only the reading note while it reads");

    let app = with_closed_b_read();
    assert_eq!(app.parameter_sheets(), paths(&[A, B]));
    assert_eq!(app.closed_parameter_sheets(), 0, "b is listed now");
    let sheets = app.parameter_documents();
    assert_eq!(
        sheets.iter().map(|s| s.is_closed).collect::<Vec<_>>(),
        [false, true]
    );
    let TableBody::Rows(rows) = table_body(&app.ui_state.i18n, "", &sheets) else {
        panic!("no filter lists every part");
    };
    let r3 = rows.iter().find(|row| row.reference == "R3").expect("R3");
    assert!(r3.is_closed);
    assert_eq!(r3.sheet, Path::new(B));
    let notes: Vec<String> = (app.parameter_scope_notes().into_iter())
        .map(|n| n.into_string())
        .collect();
    assert_eq!(
        notes,
        ["1 closed sheet is listed read-only; click one of its cells to open it."]
    );
    // The CSV export covers it too, with its Sheet cell.
    let csv = app.listed_parameter_csv().expect("a sheet").expect("csv");
    let csv = String::from_utf8(csv).expect("utf-8");
    assert!(csv.contains("R3") && csv.contains("b.snxsch"), "{csv}");
}

/// A read that lands after a newer one started, or after Active sheet was
/// picked, is dropped.
#[test]
fn a_stale_read_of_the_closed_sheets_is_dropped() {
    let mut app = app();
    app.document_state.engines.remove(Path::new(B));
    app.document_state
        .tabs
        .retain(|tab| tab.path != Path::new(B));
    all_sheets(&mut app);
    let first = app.ui_state.parameter_table.closed.generation;
    all_sheets(&mut app);
    let _ = app.update(Message::ParameterManager(Pm::ClosedSheetsRead {
        generation: first,
        sheets: std::sync::Arc::new(vec![(PathBuf::from(B), SchematicSheet::default())]),
        unread: Vec::new(),
    }));
    assert_eq!(app.parameter_sheets(), paths(&[A]), "an older read");
}

/// Editing a closed sheet opens it and writes nothing (the conservative
/// choice): a typed cell, or a press on one, asks for the
/// sheet to be opened; the copy read for the table keeps its value.
#[test]
fn an_edit_of_a_closed_sheet_opens_it_and_writes_nothing() {
    let mut app = with_closed_b_read();
    typed(&mut app, B, 3, "12%");
    assert!(!app.document_state.engines.contains_key(Path::new(B)));
    assert!(
        app.document_state.pending_opens.contains(Path::new(B)),
        "b is being opened"
    );
    let copy = app.listed_closed_sheets()[0].1;
    assert_eq!(copy.symbols[0].fields["Tolerance"], "10%");
    assert!(app.parameter_session_changes().is_empty());
}

/// An edit across sheets that would change a closed sheet changes nothing,
/// on the open sheets either, and names the sheet to open.
#[test]
fn an_edit_across_sheets_that_reaches_a_closed_sheet_changes_nothing() {
    let _ = crate::diagnostics::init_logging();
    let mut app = with_closed_b_read();
    let before = crate::diagnostics::newest_entry_id();
    add_mpn_column(&mut app);
    assert_eq!(
        field(&app, A, 1, "MPN"),
        None,
        "a is open but nothing changed"
    );
    let said = said_since(before);
    assert!(
        said.iter().any(|line| line.contains("open b.snxsch first")),
        "{said:?}"
    );
}

/// Approved deviation from mockup row 70 (Caner 2026-11-25): the switch
/// reads "Active sheet / All sheets", not "This sheet / Whole project".
#[test]
fn the_scope_switch_reads_active_sheet_and_all_sheets() {
    use signex_i18n::{ids, t};
    let app = app();
    let i18n = &app.ui_state.i18n;
    assert_eq!(
        t!(i18n, ids::DIALOG_PARAMETER_MANAGER_SCOPE_ACTIVE).into_string(),
        "Active sheet"
    );
    assert_eq!(
        t!(i18n, ids::DIALOG_PARAMETER_MANAGER_SCOPE_ALL).into_string(),
        "All sheets"
    );
}

/// Run the per-message syncs, as any message does.
fn settle(app: &mut Signex) {
    let _ = app.update(Message::ParameterManager(Pm::FilterChanged(String::new())));
}

/// Land a read of the closed sheets under `generation`.
fn land(app: &mut Signex, generation: u64, sheets: Vec<(PathBuf, SchematicSheet)>) {
    let _ = app.update(Message::ParameterManager(Pm::ClosedSheetsRead {
        generation,
        sheets: std::sync::Arc::new(sheets),
        unread: Vec::new(),
    }));
}

fn sheet_with(symbols: Vec<Symbol>) -> SchematicSheet {
    SchematicSheet {
        uuid: Uuid::now_v7(),
        paper_size: "A4".to_string(),
        symbols,
        ..Default::default()
    }
}

fn listed_csv(app: &Signex) -> String {
    let csv = app.listed_parameter_csv().expect("a sheet").expect("csv");
    String::from_utf8(csv).expect("utf-8")
}

fn close_tab(app: &mut Signex, path: &str) {
    app.document_state.engines.remove(Path::new(path));
    app.document_state
        .tabs
        .retain(|tab| tab.path != Path::new(path));
    settle(app);
}

/// A closed sheet opened, saved at 12% and closed again is read again: the
/// table and the CSV list 12%, never the 10% copy read before it opened.
#[test]
fn a_closed_sheet_opened_saved_and_closed_is_listed_with_its_saved_value() {
    let mut app = with_closed_b_read();
    let id = app.document_state.projects[0].id;
    let before = app.ui_state.parameter_table.closed.generation;

    open(&mut app, B, vec![part(3, "R3", "12%")], Some(id));
    settle(&mut app);
    assert!(
        app.ui_state.parameter_table.closed.sheets.is_empty(),
        "b's copy goes once b opens"
    );
    let sheets = app.parameter_documents();
    assert_eq!(
        sheets.iter().map(|s| s.is_closed).collect::<Vec<_>>(),
        [false, false]
    );

    close_tab(&mut app, B);
    let closed = &app.ui_state.parameter_table.closed;
    assert!(closed.reading, "b is read again once it closes");
    assert!(closed.generation > before);
    assert!(app.listed_closed_sheets().is_empty(), "no stale copy");
    assert!(!listed_csv(&app).contains("10%"));

    land(
        &mut app,
        before,
        vec![(PathBuf::from(B), sheet_with(vec![part(3, "R3", "10%")]))],
    );
    assert!(app.listed_closed_sheets().is_empty(), "an older read");

    let generation = app.ui_state.parameter_table.closed.generation;
    land(
        &mut app,
        generation,
        vec![(PathBuf::from(B), sheet_with(vec![part(3, "R3", "12%")]))],
    );
    let listed = app.listed_closed_sheets();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].1.symbols[0].fields["Tolerance"], "12%");
    let csv = listed_csv(&app);
    assert!(csv.contains("12%") && !csv.contains("10%"), "{csv}");
}

/// Project Q in `/v`, listing sheets c and d.
const PROJECT_Q: &str = r#"{"name":"Q","dir":"/v","schematic_root":"c.snxsch","pcb_file":null,
"sheets":[
{"name":"c.snxsch","filename":"c.snxsch","symbols_count":0,"wires_count":0,"labels_count":0},
{"name":"d.snxsch","filename":"d.snxsch","symbols_count":0,"wires_count":0,"labels_count":0}]}"#;

/// Another project, or a loose sheet, in the main window: the closed sheets
/// of P read for All sheets are neither listed nor exported.
#[test]
fn another_project_in_the_main_window_lists_and_exports_none_of_ps_closed_sheets() {
    let mut app = with_closed_b_read();
    let p_read = app.ui_state.parameter_table.closed.generation;
    let prj = Path::new("/v/Q.snxprj");
    let data =
        signex_types::project::parse_project_bytes(prj, PROJECT_Q.as_bytes()).expect("project");
    let q = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id: q,
        path: prj.to_path_buf(),
        data,
        pending_libraries: HashMap::new(),
        case_fold: Default::default(),
        case_probed: false,
        case_decided: false,
    });
    open(&mut app, "/v/c.snxsch", vec![part(5, "R5", "2%")], Some(q));
    let c = app.document_state.tabs.len() - 1;
    show(&mut app, c);
    settle(&mut app);

    assert!(app.listed_closed_sheets().is_empty());
    assert_eq!(app.parameter_sheets(), paths(&["/v/c.snxsch"]));
    let csv = listed_csv(&app);
    assert!(!csv.contains("R3") && !csv.contains("b.snxsch"), "{csv}");
    let closed = &app.ui_state.parameter_table.closed;
    assert!(closed.reading, "Q's closed sheet d is read");
    assert_eq!(closed.pages, paths(&["/v/d.snxsch"]));
    land(
        &mut app,
        p_read,
        vec![(PathBuf::from(B), sheet_with(vec![part(3, "R3", "10%")]))],
    );
    assert!(app.listed_closed_sheets().is_empty(), "P's read is dropped");

    // The loose sheet: no project, no closed sheet.
    let tabs = &app.document_state.tabs;
    let loose = tabs.iter().position(|tab| tab.path == Path::new(LOOSE));
    show(&mut app, loose.expect("the loose tab"));
    settle(&mut app);
    assert!(app.listed_closed_sheets().is_empty());
    assert!(!app.ui_state.parameter_table.closed.reading);
    let csv = listed_csv(&app);
    assert!(!csv.contains("R3") && !csv.contains("R5"), "{csv}");
}

/// A closed sheet that cannot be read is left out of the table, and
/// Messages names it with why.
#[test]
fn a_closed_sheet_that_cannot_be_read_is_named_in_messages_with_why() {
    let dir = tempfile::tempdir().expect("tempdir");
    let bad = dir.path().join("bad.snxsch");
    std::fs::write(&bad, "not a sheet").expect("write");
    let missing = dir.path().join("missing.snxsch");
    let read = super::super::parameter_closed_sheets::read_sheets(&[bad.clone(), missing]);
    assert!(read.sheets.is_empty());
    assert_eq!(read.unread.len(), 1, "a missing sheet is not unreadable");
    assert_eq!(read.unread[0].0, bad);
    assert!(
        read.unread[0].1.starts_with("could not be"),
        "{}",
        read.unread[0].1
    );

    let _ = crate::diagnostics::init_logging();
    let mut app = app();
    close_tab(&mut app, B);
    all_sheets(&mut app);
    let generation = app.ui_state.parameter_table.closed.generation;
    let before = crate::diagnostics::newest_entry_id();
    let _ = app.update(Message::ParameterManager(Pm::ClosedSheetsRead {
        generation,
        sheets: std::sync::Arc::default(),
        unread: vec![(PathBuf::from(B), "could not be parsed (line 1)".to_string())],
    }));
    let said = said_since(before);
    assert!(
        said.iter().any(|line| line
            == "Parameter Manager: b.snxsch is not listed, it could not be parsed (line 1)."),
        "{said:?}"
    );
    assert_eq!(
        app.closed_parameter_sheets(),
        1,
        "b is counted as not listed"
    );
}

/// A read of the closed sheets that fails on its worker names every sheet
/// it was for, with why; nothing is listed.
#[test]
fn a_failed_read_of_the_closed_sheets_names_each_sheet_in_messages() {
    use super::super::parameter_closed_sheets::read_on_worker;
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let read = runtime.block_on(read_on_worker(paths(&[B]), |_| panic!("worker stopped")));
    assert!(read.sheets.is_empty());
    assert_eq!(read.unread.len(), 1);
    assert_eq!(read.unread[0].0, Path::new(B));
    assert!(
        read.unread[0]
            .1
            .starts_with("could not be read (the read failed:"),
        "{}",
        read.unread[0].1
    );

    let _ = crate::diagnostics::init_logging();
    let mut app = app();
    close_tab(&mut app, B);
    all_sheets(&mut app);
    let generation = app.ui_state.parameter_table.closed.generation;
    let before = crate::diagnostics::newest_entry_id();
    let _ = app.update(Message::ParameterManager(Pm::ClosedSheetsRead {
        generation,
        sheets: std::sync::Arc::new(read.sheets),
        unread: read.unread,
    }));
    let said = said_since(before);
    assert!(
        said.iter()
            .any(|line| line.contains("b.snxsch is not listed, it could not be read")),
        "{said:?}"
    );
    assert!(app.listed_closed_sheets().is_empty());
}
