//! #75 / #76 (Graphite Annotate): the sheet ticks decide which sheets a run
//! numbers, and every run the dialog starts (Accept, Reset All, Reset &
//! Renumber) is one undo step that restores every sheet it touched (AV-05).
//!
//! The fixture is a three-sheet project, every sheet open: the active root
//! places `a.snxsch` and `b.snxsch`, each holding one symbol.

use signex_app::annotate_undo::AnnotateUndo;
use signex_app::app::{AnnotateMsg, EditMsg, Message, Signex};
use signex_engine::AnnotateMode;
use signex_types::schematic::{ChildSheet, FillType, Point, SchematicSheet, Symbol};
use std::path::PathBuf;

fn symbol(lib_id: &str, reference: &str) -> Symbol {
    Symbol {
        uuid: uuid::Uuid::now_v7(),
        lib_id: lib_id.to_string(),
        reference: reference.to_string(),
        position: Point::new(0.0, 0.0),
        ..Default::default()
    }
}

fn child(filename: &str, x: f64) -> ChildSheet {
    ChildSheet {
        uuid: uuid::Uuid::now_v7(),
        name: String::new(),
        filename: filename.to_string(),
        position: Point::new(x, 0.0),
        size: (0.0, 0.0),
        stroke_width: 0.0,
        fill: FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: Vec::new(),
        instances: Vec::new(),
    }
}

fn sheet(symbols: Vec<Symbol>, children: Vec<ChildSheet>) -> SchematicSheet {
    SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        paper_size: "A4".to_string(),
        symbols,
        child_sheets: children,
        ..Default::default()
    }
}

struct Project {
    app: Signex,
    root: PathBuf,
    a: PathBuf,
    b: PathBuf,
    _dir: tempfile::TempDir,
}

/// The three sheets with the given designators, the Annotate dialog open and
/// one undo step per run.
fn project(root_ref: &str, a_ref: &str, b_ref: &str) -> Project {
    let dir = tempfile::Builder::new()
        .prefix("annotate-graphite")
        .tempdir()
        .expect("tempdir");
    let root = dir.path().join("root.snxsch");
    let a = dir.path().join("a.snxsch");
    let b = dir.path().join("b.snxsch");
    let sheets = [
        (
            &root,
            sheet(
                vec![symbol("Device:R", root_ref)],
                vec![child("a.snxsch", 0.0), child("b.snxsch", 20.0)],
            ),
        ),
        (&a, sheet(vec![symbol("Device:C", a_ref)], Vec::new())),
        (&b, sheet(vec![symbol("Device:L", b_ref)], Vec::new())),
    ];
    let (mut app, _task) = Signex::new();
    app.ui_state.annotate_undo = AnnotateUndo::Project;
    for (path, document) in sheets {
        app.document_state.engines.insert(
            path.clone(),
            signex_engine::Engine::new(document).expect("engine"),
        );
        app.document_state.tabs.push(signex_app::app::TabInfo {
            title: path.file_stem().unwrap().to_string_lossy().into_owned(),
            path: path.clone(),
            project_id: None,
            kind: signex_app::app::TabKind::Schematic,
        });
    }
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(root.clone());
    // The dialog is open: its ticks apply to the run.
    app.ui_state.annotate_dialog_open = true;
    Project {
        app,
        root,
        a,
        b,
        _dir: dir,
    }
}

fn reference(app: &Signex, path: &PathBuf) -> String {
    app.document_state.engines[path].document().symbols[0]
        .reference
        .clone()
}

fn refs(p: &Project) -> [String; 3] {
    [
        reference(&p.app, &p.root),
        reference(&p.app, &p.a),
        reference(&p.app, &p.b),
    ]
}

fn send(app: &mut Signex, message: AnnotateMsg) {
    crate::settle_support::settle(app, Message::Annotate(message));
}

fn undo(app: &mut Signex) {
    let _ = app.update(Message::Edit(EditMsg::Undo));
}

#[test]
fn two_ticked_sheets_are_numbered_and_one_undo_restores_both() {
    let mut p = project("R?", "C?", "L?");
    let b = p.b.clone();
    send(&mut p.app, AnnotateMsg::SheetTickToggled(b));

    send(&mut p.app, AnnotateMsg::Run(AnnotateMode::Incremental));

    let [root, a, b] = refs(&p);
    assert_eq!((root.as_str(), a.as_str()), ("R1", "C1"), "ticked sheets");
    assert_eq!(b, "L?", "the unticked sheet is left alone");

    undo(&mut p.app);

    assert_eq!(
        refs(&p),
        ["R?", "C?", "L?"],
        "one undo restores both sheets"
    );
}

#[test]
fn an_unticked_sheet_keeps_the_numbers_it_holds_out_of_the_counter() {
    let mut p = project("R?", "R?", "R1");
    let b = p.b.clone();
    send(&mut p.app, AnnotateMsg::SheetTickToggled(b));

    send(&mut p.app, AnnotateMsg::Run(AnnotateMode::ResetAndRenumber));

    let [root, a, b] = refs(&p);
    assert_eq!(b, "R1", "the unticked sheet keeps its designator");
    let mut ticked = [root, a];
    ticked.sort();
    assert_eq!(
        ticked,
        ["R2", "R3"],
        "the ticked sheets number around the R1 the unticked one holds"
    );
}

/// AV-05: "One undo step: Ctrl+Z restores every sheet" holds for both
/// resets, started from the dialog footer through the confirm.
fn reset_then_one_undo_restores_every_sheet(mode: AnnotateMode, expected: [&str; 3]) {
    let mut p = project("R5", "C7", "L9");
    send(&mut p.app, AnnotateMsg::ResetPressed(mode));
    assert!(p.app.ui_state.annotate_reset_confirm, "the confirm opens");
    assert_eq!(p.app.ui_state.annotate_session.reset_mode, mode);

    // The confirm's danger button runs the reset it was opened for.
    let confirmed = p.app.ui_state.annotate_session.reset_mode;
    send(&mut p.app, AnnotateMsg::Run(confirmed));

    assert_eq!(refs(&p), expected, "every sheet reset");
    assert!(!p.app.ui_state.annotate_reset_confirm, "the confirm closes");

    undo(&mut p.app);

    assert_eq!(
        refs(&p),
        ["R5", "C7", "L9"],
        "a single undo restores every sheet"
    );
}

#[test]
fn reset_all_is_one_undo_step_on_every_sheet() {
    reset_then_one_undo_restores_every_sheet(AnnotateMode::ResetOnly, ["R?", "C?", "L?"]);
}

#[test]
fn reset_and_renumber_is_one_undo_step_on_every_sheet() {
    reset_then_one_undo_restores_every_sheet(AnnotateMode::ResetAndRenumber, ["R1", "C1", "L1"]);
}

#[test]
fn the_menu_reset_opens_the_confirm_for_reset_and_renumber() {
    let mut p = project("R5", "C7", "L9");
    p.app.ui_state.annotate_session.reset_mode = AnnotateMode::ResetOnly;
    send(&mut p.app, AnnotateMsg::OpenResetConfirm);
    assert!(p.app.ui_state.annotate_reset_confirm);
    assert_eq!(
        p.app.ui_state.annotate_session.reset_mode,
        AnnotateMode::ResetAndRenumber
    );
}
