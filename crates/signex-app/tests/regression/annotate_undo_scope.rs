//! #829 item 2 — "Annotate undo": an annotate run is one undo step for the
//! whole project (default) or one per sheet.
//!
//! The fixture is a two-sheet project: the active root sheet holds unit 1 of a
//! dual part plus a capacitor, a parked engine holds unit 2 plus a resistor.
//! Per-sheet undo used to leave `U1A` renumbered on one sheet and `U1B`
//! still `U?` on the other.

use signex_app::annotate_undo::AnnotateUndo;
use signex_app::app::{AnnotateMsg, EditMsg, Message, Signex};
use signex_types::schematic::{ChildSheet, FillType, Point, SchematicSheet, Symbol};
use std::path::PathBuf;

fn symbol(lib_id: &str, reference: &str, unit: u32, x: f64) -> Symbol {
    Symbol {
        uuid: uuid::Uuid::new_v4(),
        lib_id: lib_id.to_string(),
        reference: reference.to_string(),
        unit,
        position: Point::new(x, 0.0),
        ..Default::default()
    }
}

fn sheet_with(symbols: Vec<Symbol>) -> SchematicSheet {
    SchematicSheet {
        uuid: uuid::Uuid::new_v4(),
        paper_size: "A4".to_string(),
        symbols,
        ..Default::default()
    }
}

struct Project {
    app: Signex,
    root: PathBuf,
    child: PathBuf,
    _dir: tempfile::TempDir,
}

fn project(mode: AnnotateUndo) -> Project {
    let dir = tempfile::Builder::new()
        .prefix("annotate-undo")
        .tempdir()
        .expect("tempdir");
    let root = dir.path().join("root.snxsch");
    let child = dir.path().join("child.snxsch");

    let mut root_sheet = sheet_with(vec![
        symbol("Amp:Dual", "U?", 1, 0.0),
        symbol("Device:C", "C?", 1, 10.0),
    ]);
    root_sheet.child_sheets.push(ChildSheet {
        uuid: uuid::Uuid::nil(),
        name: String::new(),
        filename: "child.snxsch".to_string(),
        position: Point::new(0.0, 0.0),
        size: (0.0, 0.0),
        stroke_width: 0.0,
        fill: FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: Vec::new(),
        instances: Vec::new(),
    });
    let child_sheet = sheet_with(vec![
        symbol("Amp:Dual", "U?", 2, 0.0),
        symbol("Device:R", "R?", 1, 10.0),
    ]);

    let (mut app, _task) = Signex::new();
    app.ui_state.annotate_undo = mode;
    app.document_state.engines.insert(
        root.clone(),
        signex_engine::Engine::new(root_sheet).expect("engine"),
    );
    app.document_state.engines.insert(
        child.clone(),
        signex_engine::Engine::new(child_sheet).expect("engine"),
    );
    for path in [&root, &child] {
        app.document_state.tabs.push(signex_app::app::TabInfo {
            title: path.file_stem().unwrap().to_string_lossy().into_owned(),
            path: path.clone(),
            cached_document: None,
            dirty: false,
            project_id: None,
            kind: signex_app::app::TabKind::Schematic,
        });
    }
    activate(&mut app, 0);
    Project {
        app,
        root,
        child,
        _dir: dir,
    }
}

/// Make tab `index` the active sheet, as a tab click does for engines.
fn activate(app: &mut Signex, index: usize) {
    app.document_state.active_tab = index;
    app.document_state.active_path = Some(app.document_state.tabs[index].path.clone());
}

fn refs(app: &Signex, path: &PathBuf) -> Vec<String> {
    app.document_state.engines[path]
        .document()
        .symbols
        .iter()
        .map(|s| s.reference.clone())
        .collect()
}

fn annotate(app: &mut Signex) {
    let _ = app.update(Message::Annotate(AnnotateMsg::Run(
        signex_engine::AnnotateMode::ResetAndRenumber,
    )));
}

fn undo(app: &mut Signex) {
    let _ = app.update(Message::Edit(EditMsg::Undo));
}

fn redo(app: &mut Signex) {
    let _ = app.update(Message::Edit(EditMsg::Redo));
}

fn annotated_pair(p: &Project) -> (Vec<String>, Vec<String>) {
    (refs(&p.app, &p.root), refs(&p.app, &p.child))
}

#[test]
fn one_step_mode_undo_on_the_root_sheet_reverts_both_sheets() {
    let mut p = project(AnnotateUndo::Project);
    annotate(&mut p.app);
    let (root_after, child_after) = annotated_pair(&p);
    assert!(
        root_after.iter().all(|r| !r.ends_with('?')),
        "{root_after:?}"
    );
    assert!(
        child_after.iter().all(|r| !r.ends_with('?')),
        "{child_after:?}"
    );
    assert_eq!(
        root_after[0], child_after[0],
        "both units share one designator"
    );

    undo(&mut p.app);

    assert_eq!(refs(&p.app, &p.root), ["U?", "C?"]);
    assert_eq!(refs(&p.app, &p.child), ["U?", "R?"]);
    assert!(p.app.document_state.dirty_paths.contains(&p.child));

    redo(&mut p.app);

    assert_eq!(
        (refs(&p.app, &p.root), refs(&p.app, &p.child)),
        (root_after, child_after)
    );
}

#[test]
fn one_step_mode_undo_pressed_on_the_other_sheet_reverts_both_sheets() {
    let mut p = project(AnnotateUndo::Project);
    annotate(&mut p.app);
    let after = annotated_pair(&p);

    activate(&mut p.app, 1);
    undo(&mut p.app);

    assert_eq!(refs(&p.app, &p.root), ["U?", "C?"]);
    assert_eq!(refs(&p.app, &p.child), ["U?", "R?"]);

    redo(&mut p.app);
    assert_eq!(annotated_pair(&p), after);
}

#[test]
fn per_sheet_mode_undo_reverts_only_the_sheet_it_was_pressed_on() {
    let mut p = project(AnnotateUndo::PerSheet);
    annotate(&mut p.app);
    let (_, child_after) = annotated_pair(&p);

    undo(&mut p.app);

    assert_eq!(
        refs(&p.app, &p.root),
        ["U?", "C?"],
        "the active sheet reverts"
    );
    assert_eq!(
        refs(&p.app, &p.child),
        child_after,
        "the other sheet keeps its annotation: this is the split the default avoids"
    );
}

#[test]
fn one_step_mode_refuses_to_undo_out_of_order_after_an_edit_on_the_other_sheet() {
    // Install the process logger so the warning reaches the ring buffer.
    let _ = signex_app::diagnostics::init_logging();
    let mut p = project(AnnotateUndo::Project);
    annotate(&mut p.app);
    let after = annotated_pair(&p);
    let edited = p.app.document_state.engines[&p.child].document().symbols[1].uuid;
    p.app
        .document_state
        .engines
        .get_mut(&p.child)
        .unwrap()
        .execute(signex_engine::Command::SetSymbolDnp {
            symbol_id: edited,
            dnp: true,
        })
        .expect("edit after annotate");

    undo(&mut p.app);

    assert_eq!(annotated_pair(&p), after, "nothing reverts out of order");
    assert!(
        signex_app::diagnostics::recent_entries().iter().any(|e| e
            .message
            .contains("Cannot undo the project-wide annotate")
            && e.message
                .contains("Undo the newer edit on child.snxsch first")),
        "the refusal is explained in the Messages panel"
    );

    // Undoing the newer edit first unblocks the project-wide step.
    activate(&mut p.app, 1);
    undo(&mut p.app);
    undo(&mut p.app);
    assert_eq!(refs(&p.app, &p.root), ["U?", "C?"]);
    assert_eq!(refs(&p.app, &p.child), ["U?", "R?"]);
}

fn toggle_dnp(app: &mut Signex, path: &PathBuf, symbol_id: uuid::Uuid, dnp: bool) {
    app.document_state
        .engines
        .get_mut(path)
        .unwrap()
        .execute(signex_engine::Command::SetSymbolDnp { symbol_id, dnp })
        .expect("edit");
}

/// Replace the child's engine with a fresh one holding the same document, as
/// closing and reopening the sheet does: the history, and the group, are gone.
fn reopen_child(p: &mut Project) {
    let document = p.app.document_state.engines[&p.child].document().clone();
    p.app.document_state.engines.insert(
        p.child.clone(),
        signex_engine::Engine::new(document).expect("engine"),
    );
}

#[test]
fn redo_works_after_the_other_sheet_was_closed_and_reopened() {
    let mut p = project(AnnotateUndo::Project);
    annotate(&mut p.app);
    let (root_after, child_after) = annotated_pair(&p);
    reopen_child(&mut p);

    undo(&mut p.app);
    assert_eq!(refs(&p.app, &p.root), ["U?", "C?"]);
    assert_eq!(
        refs(&p.app, &p.child),
        child_after,
        "the reopened sheet has nothing to undo"
    );

    redo(&mut p.app);
    assert_eq!(
        refs(&p.app, &p.root),
        root_after,
        "redo must not be refused forever"
    );
}

#[test]
fn undo_and_redo_work_after_the_other_sheet_history_was_trimmed() {
    let mut p = project(AnnotateUndo::Project);
    annotate(&mut p.app);
    let (root_after, child_after) = annotated_pair(&p);
    let symbol_id = p.app.document_state.engines[&p.child].document().symbols[1].uuid;
    // Push the group entry off the end of the 100-entry history cap.
    for i in 0..100 {
        toggle_dnp(&mut p.app, &p.child.clone(), symbol_id, i % 2 == 0);
    }
    assert!(
        !p.app.document_state.engines[&p.child].has_undo_group(
            p.app.document_state.engines[&p.root]
                .next_undo_group()
                .unwrap()
        )
    );

    undo(&mut p.app);
    assert_eq!(refs(&p.app, &p.root), ["U?", "C?"]);
    assert_eq!(refs(&p.app, &p.child), child_after);

    redo(&mut p.app);
    assert_eq!(refs(&p.app, &p.root), root_after);
}

#[test]
fn redo_is_refused_with_its_own_wording_after_an_edit_following_the_undo() {
    let _ = signex_app::diagnostics::init_logging();
    let mut p = project(AnnotateUndo::Project);
    annotate(&mut p.app);
    undo(&mut p.app);
    let symbol_id = p.app.document_state.engines[&p.child].document().symbols[1].uuid;
    toggle_dnp(&mut p.app, &p.child.clone(), symbol_id, true);

    redo(&mut p.app);

    assert_eq!(refs(&p.app, &p.root), ["U?", "C?"], "nothing is redone");
    assert!(
        signex_app::diagnostics::recent_entries()
            .iter()
            .any(|e| e.message.contains(
                "Cannot redo the project-wide annotate: child.snxsch was edited after the undo"
            )),
    );
}

#[test]
fn undo_says_which_sheets_stay_annotated_because_they_are_not_open() {
    let _ = signex_app::diagnostics::init_logging();
    let mut p = project(AnnotateUndo::Project);
    annotate(&mut p.app);
    p.app.document_state.engines.remove(&p.child);

    undo(&mut p.app);

    assert_eq!(refs(&p.app, &p.root), ["U?", "C?"]);
    assert!(
        signex_app::diagnostics::recent_entries()
            .iter()
            .any(|e| e.message.contains(
                "Annotate undone on open sheets; not open, so still annotated on disk: child.snxsch"
            )),
    );
}
