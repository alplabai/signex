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

fn run(app: &mut Signex, task: Task<Message>) {
    crate::settle_support::settle_task(app, task);
}

struct Setup {
    app: Signex,
    path: PathBuf,
    _dir: tempfile::TempDir,
}

fn setup(sheets: &[(&str, Option<&str>)], unreadable: &[&str]) -> Setup {
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
        _dir: dir,
    }
}

#[test]
fn a_closed_sheet_that_places_the_symbol_is_named_when_the_scan_lands() {
    let Setup {
        mut app,
        path,
        _dir,
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
    } = setup(&[("clean.snxsch", None)], &[]);

    let task = delete(&mut app, 1);
    assert_eq!(symbol_count(&app, &path), 2, "held until the scan answers");
    run(&mut app, task);

    assert_eq!(symbol_count(&app, &path), 1);
    assert!(warning(&app).is_none());
}

#[test]
fn a_scan_that_lands_after_the_symbol_file_changed_is_dropped() {
    let Setup {
        mut app,
        path,
        _dir,
    } = setup(&[("clean.snxsch", None)], &[]);

    let task = delete(&mut app, 1);
    let editor = app.document_state.symbol_editors.get_mut(&path).unwrap();
    assert!(editor.record_edit(None, |e| {
        e.file.symbols[0].name.push('x');
        true
    }));
    run(&mut app, task);

    assert_eq!(symbol_count(&app, &path), 2, "a stale scan deletes nothing");
    assert!(warning(&app).is_none(), "and leaves no warning behind");
}

#[test]
fn a_scan_for_a_replaced_warning_does_not_answer_the_new_one() {
    let Setup {
        mut app,
        path,
        _dir,
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
