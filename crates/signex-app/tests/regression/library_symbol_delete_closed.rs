//! #1903 — the symbol delete warning also covers the closed sheets of the
//! open projects, read from disk by a Task while the warning stays usable.

use crate::library_place_symbol::{Fixture, app_with_library, fixture};
use crate::library_symbol_missing::{open_container_editor, panel, symbol_count};
use iced::Task;
use signex_app::app::{LoadedProject, Message, Signex};
use signex_app::dock::DockMessage;
use signex_app::panels::{PanelMsg, SymbolDeleteWarning};
use signex_library::PrimitiveRef;
use signex_types::format::SnxSchematic;
use signex_types::project::{ProjectData, SheetEntry};
use signex_types::schematic::{SchematicSheet, Symbol};
use std::path::{Path, PathBuf};

fn closed_sheet(dir: &Path, name: &str, placing: Option<(&Fixture, &str)>) {
    let mut sheet = SchematicSheet::default();
    if let Some((f, reference)) = placing {
        let id =
            signex_app::library::place::lib_id_of(&PrimitiveRef::new(f.library_id, f.symbol_id));
        sheet.symbols.push(Symbol {
            lib_id: id.to_uppercase(),
            reference: reference.into(),
            ..Default::default()
        });
    }
    let text = SnxSchematic::new(sheet).write_string().unwrap();
    std::fs::write(dir.join(name), text).unwrap();
}

/// A project in `dir` that declares `sheets`, none of them open as a tab.
fn open_project(app: &mut Signex, dir: &Path, sheets: &[&str]) {
    let id = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id,
        path: dir.join("P.snxprj"),
        data: ProjectData {
            description: None,
            name: "P".into(),
            dir: dir.to_string_lossy().into_owned(),
            schematic_root: None,
            pcb_file: None,
            sheets: sheets
                .iter()
                .map(|f| SheetEntry {
                    name: (*f).into(),
                    filename: (*f).into(),
                    symbols_count: 0,
                    wires_count: 0,
                    labels_count: 0,
                    unknown: Default::default(),
                })
                .collect(),
            variant_definitions: Vec::new(),
            active_variant: None,
            libraries: Vec::new(),
            enable_git: false,
            net_classes: Vec::new(),
            rules: None,
            annotation: None,
            schematic_defaults: Default::default(),
            parameters: Default::default(),
            unknown: Default::default(),
        },
        pending_libraries: Default::default(),
        case_fold: Default::default(),
        case_probed: false,
        case_decided: false,
    });
}

fn delete(app: &mut Signex, idx: usize) -> Task<Message> {
    app.update(Message::Dock(DockMessage::Panel(
        PanelMsg::SchLibraryDeleteSymbol(idx),
    )))
}

fn warning(app: &Signex) -> Option<SymbolDeleteWarning> {
    app.document_state
        .panel_ctx
        .symbol_editor
        .as_ref()
        .unwrap()
        .delete_warning
        .clone()
}

/// Give the unused sibling a name no other test uses, so a log line asserted
/// below cannot come from a test running in parallel (the buffer is shared).
fn rename_sibling(app: &mut Signex, path: &Path, name: &str) {
    app.document_state
        .symbol_editors
        .get_mut(path)
        .unwrap()
        .file
        .symbols[1]
        .name = name.into();
}

fn logged(needle: &str) -> bool {
    let _ = signex_app::diagnostics::init_logging();
    signex_app::diagnostics::recent_entries()
        .iter()
        .any(|e| e.message.contains(needle))
}

fn run(app: &mut Signex, task: Task<Message>) {
    crate::settle_support::settle_task(app, task);
}

struct Setup {
    app: Signex,
    path: PathBuf,
    library_id: uuid::Uuid,
    _dir: tempfile::TempDir,
}

fn setup(sheets: &[(&str, Option<&str>)], unreadable: &[&str]) -> Setup {
    // Installed before anything logs; a line emitted earlier is lost.
    let _ = signex_app::diagnostics::init_logging();
    let f = fixture();
    let mut app = app_with_library(&f);
    let path = open_container_editor(&mut app, &f);
    let dir = tempfile::tempdir().unwrap();
    for (name, reference) in sheets {
        closed_sheet(dir.path(), name, reference.map(|r| (&f, r)));
    }
    for name in unreadable {
        std::fs::write(dir.path().join(name), "not a sheet").unwrap();
    }
    let declared: Vec<&str> = sheets
        .iter()
        .map(|(n, _)| *n)
        .chain(unreadable.iter().copied())
        .collect();
    open_project(&mut app, dir.path(), &declared);
    Setup {
        app,
        path,
        library_id: f.library_id,
        _dir: dir,
    }
}

#[test]
fn a_closed_sheet_that_places_the_symbol_is_named_when_the_scan_lands() {
    let Setup {
        mut app,
        path,
        _dir,
        ..
    } = setup(&[("closed.snxsch", Some("U9"))], &[]);

    let task = delete(&mut app, 0);
    let shown = warning(&app).expect("the row already warns");
    assert!(shown.checking, "the scan is still running");
    assert!(shown.lines.join("\n").contains("Checking closed sheets"));
    assert_eq!(symbol_count(&app, &path), 2);

    run(&mut app, task);
    let shown = warning(&app).expect("still warned");
    let text = shown.lines.join("\n");
    assert!(!shown.checking, "{text}");
    assert!(
        text.contains("Closed sheets with placed parts: closed.snxsch: U9"),
        "{text}"
    );
    assert!(!text.contains("Checking closed sheets"), "{text}");
    assert_eq!(symbol_count(&app, &path), 2);
}

#[test]
fn an_unreadable_closed_sheet_is_unchecked_never_unused() {
    // The sibling is bound by no row and placed on no open sheet.
    let Setup {
        mut app,
        path,
        _dir,
        ..
    } = setup(&[], &["broken.snxsch"]);

    let task = delete(&mut app, 1);
    assert!(warning(&app).expect("held for the scan").checking);
    run(&mut app, task);

    let shown = warning(&app).expect("a failed read does not let the delete through");
    let text = shown.lines.join("\n");
    assert!(
        text.contains("Closed sheets not checked: broken.snxsch"),
        "{text}"
    );
    assert!(shown.only_unknown, "nothing is confirmed in use");
    assert_eq!(symbol_count(&app, &path), 2);
}

#[test]
fn an_unused_symbol_is_deleted_once_every_closed_sheet_reads_clean() {
    let Setup {
        mut app,
        path,
        _dir,
        ..
    } = setup(&[("clean.snxsch", None)], &[]);
    rename_sibling(&mut app, &path, "CLEAN-SIB");

    let task = delete(&mut app, 1);
    assert_eq!(symbol_count(&app, &path), 2, "held until the scan answers");
    run(&mut app, task);

    assert_eq!(symbol_count(&app, &path), 1);
    assert!(warning(&app).is_none());
    assert!(logged(
        "CLEAN-SIB deleted from two-unit.snxsym. Not used by open sheets or mounted libraries; closed sheets of open projects checked, other projects not."
    ));
}

#[test]
fn a_scan_that_lands_after_the_symbol_file_changed_is_dropped() {
    let Setup {
        mut app,
        path,
        _dir,
        ..
    } = setup(&[("clean.snxsch", None)], &[]);
    rename_sibling(&mut app, &path, "STALE-SIB");

    let task = delete(&mut app, 1);
    let editor = app.document_state.symbol_editors.get_mut(&path).unwrap();
    assert!(editor.record_edit(None, |e| {
        e.file.symbols[0].name.push('x');
        true
    }));
    run(&mut app, task);

    assert_eq!(symbol_count(&app, &path), 2, "a stale scan deletes nothing");
    assert!(warning(&app).is_none(), "and leaves no warning behind");
    assert!(logged(
        "STALE-SIB changed since the warning; press Delete again."
    ));
}

#[test]
fn a_scan_for_a_replaced_warning_does_not_answer_the_new_one() {
    let Setup {
        mut app,
        path,
        _dir,
        ..
    } = setup(&[("clean.snxsch", None)], &[]);

    let first = delete(&mut app, 0);
    let second = delete(&mut app, 0);
    run(&mut app, first);
    assert!(
        warning(&app).expect("the new warning").checking,
        "the first scan must not finish the second warning"
    );
    run(&mut app, second);
    assert!(!warning(&app).expect("answered").checking);
    assert_eq!(symbol_count(&app, &path), 2);
}

#[test]
fn a_cancelled_warning_ignores_its_late_scan() {
    let Setup {
        mut app,
        path,
        _dir,
        ..
    } = setup(&[("clean.snxsch", None)], &[]);

    let task = delete(&mut app, 1);
    panel(&mut app, PanelMsg::SchLibraryDeleteCancelled);
    run(&mut app, task);

    assert_eq!(symbol_count(&app, &path), 2, "cancelled means kept");
}

#[test]
fn delete_anyway_while_the_scan_runs_says_closed_sheets_were_not_checked() {
    let _ = signex_app::diagnostics::init_logging();
    let Setup {
        mut app,
        path,
        _dir,
        ..
    } = setup(&[("closed.snxsch", Some("U9"))], &[]);

    let _task = delete(&mut app, 0);
    panel(&mut app, PanelMsg::SchLibraryDeleteConfirmed(0));

    assert_eq!(symbol_count(&app, &path), 1, "the user can still delete");
    let said = signex_app::diagnostics::recent_entries().iter().any(|e| {
        e.message.contains("TWO-UNIT deleted")
            && e.message.contains("Closed sheets were not checked")
    });
    assert!(said);
}

fn child_ref(name: &str) -> signex_types::schematic::ChildSheet {
    signex_types::schematic::ChildSheet {
        uuid: uuid::Uuid::now_v7(),
        name: name.into(),
        filename: name.into(),
        position: signex_types::schematic::Point::new(0.0, 0.0),
        size: (10.0, 10.0),
        stroke_width: 0.0,
        fill: Default::default(),
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: Vec::new(),
        instances: Vec::new(),
    }
}

#[test]
fn a_closed_child_sheet_reached_only_by_a_sheet_symbol_is_scanned() {
    let Setup {
        mut app,
        path,
        library_id,
        _dir,
    } = setup(&[], &[]);
    let dir = _dir.path();
    // The project declares only root.snxsch; child.snxsch is a hierarchical
    // child that places the unused sibling.
    let sibling = app.document_state.symbol_editors[&path].file.symbols[1].uuid;
    let id = signex_app::library::place::lib_id_of(&PrimitiveRef::new(library_id, sibling));
    let mut root = SchematicSheet::default();
    root.child_sheets.push(child_ref("child.snxsch"));
    let mut child = SchematicSheet::default();
    child.symbols.push(Symbol {
        lib_id: id,
        reference: "U7".into(),
        ..Default::default()
    });
    for (name, sheet) in [("root.snxsch", root), ("child.snxsch", child)] {
        std::fs::write(
            dir.join(name),
            SnxSchematic::new(sheet).write_string().unwrap(),
        )
        .unwrap();
    }
    open_project(&mut app, dir, &["root.snxsch"]);

    let task = delete(&mut app, 1);
    run(&mut app, task);

    let text = warning(&app).expect("the child keeps it").lines.join("\n");
    assert!(text.contains("child.snxsch: U7"), "{text}");
    assert_eq!(symbol_count(&app, &path), 2);
}

#[test]
fn a_scan_landing_after_a_tab_switch_deletes_only_in_its_own_file() {
    let Setup {
        mut app,
        path,
        _dir,
        ..
    } = setup(&[("clean.snxsch", None)], &[]);
    let task = delete(&mut app, 1);

    // Another symbol file, also with two symbols, becomes the active tab.
    let other = PathBuf::from("other.snxsym");
    let mut file = signex_library::SymbolFile::from_symbol(signex_library::Symbol::empty("O1"));
    file.symbols.push(signex_library::Symbol::empty("O2"));
    app.document_state.symbol_editors.insert(
        other.clone(),
        signex_app::app::SymbolDocument::new(other.clone(), file),
    );
    app.document_state.tabs.push(signex_app::app::TabInfo {
        title: "other".into(),
        path: other.clone(),
        cached_document: None,
        project_id: None,
        kind: signex_app::app::TabKind::SymbolEditor(other.clone()),
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    run(&mut app, task);

    assert_eq!(
        symbol_count(&app, &other),
        2,
        "the active file is untouched"
    );
    assert_eq!(
        symbol_count(&app, &path),
        1,
        "the file that asked is changed"
    );
}

#[test]
fn a_symbol_placed_on_an_open_sheet_during_the_scan_stops_the_delete() {
    let Setup {
        mut app,
        path,
        library_id,
        _dir,
    } = setup(&[("clean.snxsch", None)], &[]);
    let task = delete(&mut app, 1);

    let sibling = app.document_state.symbol_editors[&path].file.symbols[1].uuid;
    let id = signex_app::library::place::lib_id_of(&PrimitiveRef::new(library_id, sibling));
    let engine = app.document_state.active_engine_mut().expect("open sheet");
    let mut sheet = engine.document().clone();
    sheet.symbols.push(Symbol {
        lib_id: id,
        reference: "U5".into(),
        ..Default::default()
    });
    engine.set_document(sheet);
    run(&mut app, task);

    let shown = warning(&app).expect("the placement keeps it");
    assert!(shown.lines.join("\n").contains("U5"), "{:?}", shown.lines);
    assert!(!shown.checking);
    assert_eq!(symbol_count(&app, &path), 2);
}

fn sibling_lib_id(s: &Setup) -> String {
    let sibling = s.app.document_state.symbol_editors[&s.path].file.symbols[1].uuid;
    signex_app::library::place::lib_id_of(&PrimitiveRef::new(s.library_id, sibling))
}

fn write_sheet(path: &Path, lib_id: Option<&str>, children: &[&str]) {
    let mut sheet = SchematicSheet::default();
    if let Some(id) = lib_id {
        sheet.symbols.push(Symbol {
            lib_id: id.into(),
            reference: "U3".into(),
            ..Default::default()
        });
    }
    sheet.child_sheets = children.iter().map(|c| child_ref(c)).collect();
    std::fs::write(path, SnxSchematic::new(sheet).write_string().unwrap()).unwrap();
}

#[test]
fn a_project_opened_during_the_scan_is_not_assumed_clean() {
    let mut s = setup(&[("clean.snxsch", None)], &[]);
    let task = delete(&mut s.app, 1);

    let late = tempfile::tempdir().unwrap();
    write_sheet(
        &late.path().join("late.snxsch"),
        Some(&sibling_lib_id(&s)),
        &[],
    );
    open_project(&mut s.app, late.path(), &["late.snxsch"]);
    run(&mut s.app, task);

    let text = warning(&s.app).expect("held, not deleted").lines.join("\n");
    assert!(
        text.contains("late.snxsch was added while the check ran"),
        "{text}"
    );
    assert_eq!(symbol_count(&s.app, &s.path), 2);
}

#[test]
fn an_open_sheet_closed_during_the_scan_is_listed_unchecked() {
    // The open sheet is also a declared page of the project.
    let mut s = setup(&[("clean.snxsch", None), ("open.snxsch", None)], &[]);
    let key = s.app.document_state.engines.keys().next().cloned().unwrap();
    let engine = s.app.document_state.engines.remove(&key).unwrap();
    let open = s._dir.path().join("open.snxsch");
    s.app.document_state.engines.insert(open.clone(), engine);
    let task = delete(&mut s.app, 1);

    s.app.document_state.engines.remove(&open);
    run(&mut s.app, task);

    let text = warning(&s.app).expect("held, not deleted").lines.join("\n");
    let name = open.file_name().unwrap().to_string_lossy();
    assert!(
        text.contains(&format!("{name} was closed while the check ran")),
        "{text}"
    );
    assert!(
        !text.contains(&format!("{name} was added")),
        "a page that was open is not 'added': {text}"
    );
    assert_eq!(symbol_count(&s.app, &s.path), 2);
}

#[test]
fn a_scan_whose_worker_died_leaves_everything_unchecked_and_unlabelled() {
    let mut s = setup(&[("clean.snxsch", None)], &[]);
    let _task = delete(&mut s.app, 1);
    let serial = s.app.library.pending_symbol_delete.as_ref().unwrap().serial;
    let report = signex_app::library::closed_sheets::ClosedSheetsReport {
        serial,
        scan: signex_app::library::closed_sheets::ClosedSheetScan::unfinished("boom"),
        plan: Default::default(),
    };

    let task = s.app.update(Message::Library(
        signex_app::library::LibraryMessage::SymbolDeleteClosedSheetsScanned(Box::new(report)),
    ));
    run(&mut s.app, task);

    let text = warning(&s.app).expect("not deleted").lines.join("\n");
    assert!(text.contains("closed sheets (boom)"), "{text}");
    assert!(!text.contains("was added"), "{text}");
    assert_eq!(symbol_count(&s.app, &s.path), 2);
}

#[test]
fn a_child_referenced_only_by_an_open_sheet_is_scanned() {
    let mut s = setup(&[], &[]);
    let dir = s._dir.path().to_path_buf();
    // The open sheet's tab holds the child reference; its disk copy does not
    // exist, so only the live document can lead to kid.snxsch.
    write_sheet(&dir.join("kid.snxsch"), Some(&sibling_lib_id(&s)), &[]);
    let key = s.app.document_state.engines.keys().next().cloned().unwrap();
    let mut engine = s.app.document_state.engines.remove(&key).unwrap();
    let mut doc = engine.document().clone();
    doc.child_sheets.push(child_ref("kid.snxsch"));
    engine.set_document(doc);
    let parent = dir.join("parent.snxsch");
    s.app.document_state.engines.insert(parent, engine);

    let task = delete(&mut s.app, 1);
    run(&mut s.app, task);

    let text = warning(&s.app)
        .expect("the child keeps it")
        .lines
        .join("\n");
    assert!(text.contains("kid.snxsch: U3"), "{text}");
    assert_eq!(symbol_count(&s.app, &s.path), 2);
}

#[test]
fn a_closed_sheet_saved_with_the_symbol_after_the_worker_read_it_stops_the_delete() {
    use signex_app::library::closed_sheets::{
        ClosedSheetsReport, ScanPlan, SymbolMatch, scan_closed_sheets,
    };
    let mut s = setup(&[("clean.snxsch", None)], &[]);
    let _task = delete(&mut s.app, 1);
    let pending = s.app.library.pending_symbol_delete.as_ref().unwrap();
    let (serial, sibling) = (pending.serial, pending.symbol);
    let sheet = s._dir.path().join("clean.snxsch");
    let fold = s.app.document_state.projects[0].case_fold;
    let plan = ScanPlan {
        seeds: vec![(sheet.clone(), fold)],
        open: s.app.document_state.engines.keys().cloned().collect(),
        unresolved: Vec::new(),
    };
    // The worker reads clean.snxsch while it is still clean...
    let scan = scan_closed_sheets(&plan, SymbolMatch::InLibrary(s.library_id, sibling));
    // ...then it is opened, the symbol placed, saved and closed (or written externally).
    write_sheet(&sheet, Some(&sibling_lib_id(&s)), &[]);
    let task = s.app.update(Message::Library(
        signex_app::library::LibraryMessage::SymbolDeleteClosedSheetsScanned(Box::new(
            ClosedSheetsReport { serial, scan, plan },
        )),
    ));
    run(&mut s.app, task);
    assert_eq!(
        symbol_count(&s.app, &s.path),
        2,
        "a sheet that changed after it was read must not let the delete through"
    );
    assert!(warning(&s.app).is_some_and(|w| !w.checking));
}

#[test]
fn an_unreadable_closed_sheet_reaches_the_messages_panel() {
    let Setup { mut app, _dir, .. } = setup(&[], &["broken-1903-log.snxsch"]);
    let task = delete(&mut app, 1);
    run(&mut app, task);
    assert!(warning(&app).is_some());
    assert!(
        logged("broken-1903-log.snxsch"),
        "the failed read is only in the modal"
    );
}
