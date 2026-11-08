//! #2172: where each kind keeps its id, how an unopened project file gets
//! one, and what a repeated or missing id turns into.

use std::path::Path;

use signex_types::document_id::DocumentId;
use signex_types::project::{ProjectData, SheetEntry};

use super::headers::read_header_id;
use super::{DocumentKind, DocumentTarget, HeaderRead};
use crate::app::Signex;
use crate::app::command::queries::tests::{SHEET, answer, app_with, path, uid};
use crate::app::state::LoadedProject;

fn temp_dir() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("snx-2172")
        .tempdir()
        .expect("temp dir")
}

fn sheet_entry(name: &str) -> SheetEntry {
    SheetEntry {
        name: name.into(),
        filename: format!("{name}.snxsch"),
        symbols_count: 0,
        wires_count: 0,
        labels_count: 0,
        unknown: Default::default(),
    }
}

/// A project of two sheets and a board, none loaded.
fn app_with_project(dir: &Path) -> Signex {
    let (mut app, _task) = Signex::new();
    let id = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id,
        path: dir.join("P.snxprj"),
        data: ProjectData {
            document_id: Some(DocumentId::new()),
            description: None,
            name: "P".into(),
            dir: dir.to_string_lossy().into_owned(),
            schematic_root: Some("Top.snxsch".into()),
            pcb_file: Some("Board.snxpcb".into()),
            sheets: vec![sheet_entry("Top"), sheet_entry("Sub")],
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
    app
}

fn names(app: &Signex, kind: DocumentKind) -> Vec<String> {
    app.document_entries()
        .into_iter()
        .filter(|e| e.kind == kind)
        .map(|e| e.name)
        .collect()
}

#[test]
fn files_a_project_names_are_listed_unloaded_with_the_id_read_from_their_head() {
    let dir = temp_dir();
    let mut app = app_with_project(dir.path());
    let headers = |app: &mut Signex, reads: Vec<(&str, Option<u128>)>| {
        let project = app.document_state.projects[0].id;
        let reads = reads
            .into_iter()
            .map(|(name, n)| HeaderRead {
                path: dir.path().join(name),
                id: n.and_then(|n| DocumentId::from_uuid(uid(n))),
            })
            .collect();
        let _ = app.handle_document_headers_read(project, reads);
    };
    // Nothing read yet: listed, without an id.
    let entries = app.document_entries();
    let top = entries.iter().find(|e| e.name == "Top").expect("Top");
    assert!(!top.loaded && top.id.is_none());
    assert_eq!(names(&app, DocumentKind::Schematic), ["Sub", "Top"]);
    assert_eq!(names(&app, DocumentKind::Pcb), ["Board"]);
    assert_eq!(names(&app, DocumentKind::Project), ["P"]);

    headers(
        &mut app,
        vec![
            ("Top.snxsch", Some(1)),
            ("Sub.snxsch", Some(2)),
            ("Board.snxpcb", Some(3)),
        ],
    );
    let id_of = |app: &Signex, name: &str| {
        app.document_entries()
            .into_iter()
            .find(|e| e.name == name)
            .and_then(|e| e.id)
    };
    assert_eq!(id_of(&app, "Top"), DocumentId::from_uuid(uid(1)));
    assert_eq!(id_of(&app, "Board"), DocumentId::from_uuid(uid(3)));
    let project = app
        .document_entries()
        .into_iter()
        .find(|e| e.name == "Top")
        .unwrap()
        .project
        .unwrap();
    assert_eq!(project.name, "P");
}

#[test]
fn a_head_id_another_document_already_holds_is_not_listed_twice() {
    let dir = temp_dir();
    let mut app = app_with_project(dir.path());
    let project = app.document_state.projects[0].id;
    let _ = app.handle_document_headers_read(
        project,
        vec![
            HeaderRead {
                path: dir.path().join("Sub.snxsch"),
                id: DocumentId::from_uuid(uid(5)),
            },
            HeaderRead {
                path: dir.path().join("Top.snxsch"),
                id: DocumentId::from_uuid(uid(5)),
            },
        ],
    );
    let ids: Vec<_> = app
        .document_entries()
        .into_iter()
        .filter(|e| e.kind == DocumentKind::Schematic)
        .map(|e| (e.name, e.id.is_some()))
        .collect();
    assert_eq!(ids, [("Sub".to_string(), false), ("Top".to_string(), true)]);
}

#[test]
fn a_head_read_for_a_project_that_closed_is_dropped() {
    let dir = temp_dir();
    let mut app = app_with_project(dir.path());
    let project = app.document_state.projects[0].id;
    app.document_state.projects.clear();
    let _ = app.handle_document_headers_read(
        project,
        vec![HeaderRead {
            path: dir.path().join("Top.snxsch"),
            id: DocumentId::from_uuid(uid(1)),
        }],
    );
    assert_eq!(
        app.document_state
            .document_ids
            .header(&dir.path().join("Top.snxsch")),
        None
    );
}

#[test]
fn closing_a_project_forgets_the_heads_under_its_folder() {
    let dir = temp_dir();
    let mut app = app_with_project(dir.path());
    let top = dir.path().join("Top.snxsch");
    app.document_state
        .document_ids
        .set_header(top.clone(), DocumentId::from_uuid(uid(1)));
    app.document_state.document_ids.set_header(
        std::env::temp_dir().join("elsewhere-2172.snxsch"),
        DocumentId::from_uuid(uid(2)),
    );
    app.document_state
        .document_ids
        .forget_headers_under(dir.path());
    assert_eq!(app.document_state.document_ids.header(&top), None);
    assert!(
        app.document_state
            .document_ids
            .header(&std::env::temp_dir().join("elsewhere-2172.snxsch"))
            .is_some()
    );
}

#[test]
fn only_files_not_yet_read_are_read() {
    let dir = temp_dir();
    let mut app = app_with_project(dir.path());
    let project = app.document_state.projects[0].id;
    assert!(app.read_missing_document_headers(project).units() > 0);
    for name in ["Top.snxsch", "Sub.snxsch", "Board.snxpcb"] {
        app.document_state
            .document_ids
            .set_header(dir.path().join(name), None);
    }
    assert_eq!(app.read_missing_document_headers(project).units(), 0);
}

#[test]
fn a_head_is_read_from_the_file_and_a_missing_or_idless_one_has_none() {
    let dir = temp_dir();
    let sheet = crate::app::command::queries::tests::sheet(SHEET);
    let file = dir.path().join("a.snxsch");
    let mut engine = signex_engine::Engine::new_with_path(sheet, Some(file.clone())).unwrap();
    engine.save().unwrap();
    assert_eq!(
        read_header_id(&file).map(|id| id.as_uuid()),
        Some(uid(SHEET))
    );
    assert_eq!(read_header_id(&dir.path().join("none.snxsch")), None);
    let plain = dir.path().join("plain.txt");
    std::fs::write(&plain, "schematic_id = \"x\"").unwrap();
    assert_eq!(read_header_id(&plain), None);
    let nil = dir.path().join("nil.snxsch");
    std::fs::write(&nil, "format = \"snxsch/1\"\n").unwrap();
    assert_eq!(read_header_id(&nil), None);
}

#[test]
fn only_the_head_of_a_big_file_is_read() {
    let dir = temp_dir();
    let big = dir.path().join("big.snxsch");
    let id = uid(42);
    let mut text = format!("format = \"snxsch/1\"\nschematic_id = \"{id}\"\n[extras]\n");
    text.push_str(&"filler = 1\n".repeat(20_000));
    std::fs::write(&big, text).unwrap();
    assert_eq!(read_header_id(&big).map(|i| i.as_uuid()), Some(id));
}

#[test]
fn a_document_target_is_an_id_or_an_absolute_path() {
    let id = DocumentId::new();
    assert_eq!(
        DocumentTarget::parse(&id.to_string()),
        Ok(DocumentTarget::Id(id))
    );
    let abs = path("x.snxsch").display().to_string();
    assert_eq!(
        DocumentTarget::parse(&abs),
        Ok(DocumentTarget::Path(abs.clone()))
    );
    assert!(DocumentTarget::parse("relative/x.snxsch").is_err());
    assert!(DocumentTarget::parse("").is_err());
    assert!(DocumentTarget::parse("/tmp/a\0b").is_err());
    assert_eq!(DocumentTarget::Id(id).to_string(), id.to_string());
    assert_eq!(DocumentTarget::Path(abs.clone()).to_string(), abs);
}

#[test]
fn a_path_resolves_through_the_volume_rule_not_its_spelling() {
    let mut app = app_with(&[("a.snxsch", SHEET)]);
    let dotted = path("sub")
        .join("..")
        .join("a.snxsch")
        .display()
        .to_string();
    let target = DocumentTarget::parse(&dotted).expect("a path");
    let entry = app.resolve_document_target(&target).expect("resolves");
    assert_eq!(entry.path.as_deref(), Some(path("a.snxsch").as_path()));
    let _ = answer(&mut app, "list_documents", &[]);
}

#[test]
fn a_board_and_a_project_with_no_id_are_given_one() {
    let dir = temp_dir();
    let mut app = app_with_project(dir.path());
    app.document_state.projects[0].data.document_id = None;
    let board = signex_types::pcb::PcbBoard {
        unknown: Default::default(),
        uuid: uuid::Uuid::nil(),
        version: 1,
        generator: "t".into(),
        thickness: 1.6,
        outline: vec![],
        layers: vec![],
        setup: None,
        nets: vec![],
        footprints: vec![],
        segments: vec![],
        vias: vec![],
        zones: vec![],
        graphics: vec![],
        texts: vec![],
    };
    app.open_pcb_tab(dir.path().join("Board.snxpcb"), "Board".into(), board);
    let repaired = app.reconcile_document_ids();
    assert_eq!(repaired.len(), 2, "{repaired:?}");
    assert!(app.document_state.projects[0].data.document_id.is_some());
    let ids: Vec<_> = app
        .document_entries()
        .into_iter()
        .filter_map(|e| e.id)
        .collect();
    assert_eq!(ids.len(), 2);
    assert!(
        app.reconcile_document_ids().is_empty(),
        "nothing left to repair"
    );
}

/// A project whose two sheets are files on disk and in no tab, heads read.
fn app_with_files(dir: &Path) -> Signex {
    for (name, id) in [("Top", SHEET), ("Sub", SHEET + 1)] {
        let file = dir.join(format!("{name}.snxsch"));
        signex_engine::Engine::new_with_path(
            crate::app::command::queries::tests::sheet(id),
            Some(file),
        )
        .unwrap()
        .save()
        .unwrap();
    }
    let mut app = app_with_project(dir);
    let project = app.document_state.projects[0].id;
    let reads = ["Top.snxsch", "Sub.snxsch"]
        .map(|n| HeaderRead {
            path: dir.join(n),
            id: read_header_id(&dir.join(n)),
        })
        .to_vec();
    let _ = app.handle_document_headers_read(project, reads);
    app
}

fn headless(app: &mut Signex, name: &str, operands: &[&str]) -> serde_json::Value {
    let command = crate::keymap::AppCommandId::new(name).unwrap();
    let args = crate::app::command::CommandArgs::parse_operands(&command, operands).unwrap();
    match app.run_query_headless(&command, &args).expect("a query").0 {
        crate::app::command::CommandOutcome::Answered(value) => value,
        other => panic!("{name}: {other:?}"),
    }
}

/// A sheet no tab holds is loaded for the answer and dropped: the query is
/// answered, no tab is made and nothing of the person's moves.
#[test]
fn a_query_on_an_unloaded_sheet_loads_it_for_the_answer_and_drops_it() {
    use crate::app::command::interaction_snapshot_tests::interaction_snapshot;
    let dir = temp_dir();
    let mut app = app_with_files(dir.path());
    let before = interaction_snapshot(&app);
    assert!(app.document_state.engines.is_empty() && app.document_state.tabs.is_empty());

    let read = headless(
        &mut app,
        "read_document",
        &[&format!("document={}", uid(SHEET + 1))],
    );
    assert_eq!(read["total"], 10);
    assert_eq!(read["document"]["loaded"], false);
    assert!(
        read["revision"].is_null(),
        "an unloaded sheet has no revision"
    );

    let found = headless(&mut app, "find_items", &["designator=U3", "kind=symbol"]);
    assert_eq!(found["count"], 2, "both sheets are searched");
    let element = headless(&mut app, "resolve", &[&format!("id={}", uid(7))]);
    assert_eq!(element["found"], true);

    assert!(app.document_state.engines.is_empty(), "nothing is kept");
    assert!(app.document_state.tabs.is_empty(), "no tab was made");
    assert!(
        !app.document_state
            .document_ids
            .is_transient(&dir.path().join("Sub.snxsch"))
    );
    assert_eq!(interaction_snapshot(&app), before);
    let listed = app.document_entries();
    assert!(
        listed
            .iter()
            .filter(|e| e.kind == DocumentKind::Schematic)
            .all(|e| !e.loaded)
    );
}

#[test]
fn an_unreadable_sheet_is_named_and_the_read_is_refused() {
    let _ = crate::diagnostics::init_logging();
    let dir = temp_dir();
    let mut app = app_with_files(dir.path());
    std::fs::write(dir.path().join("Sub.snxsch"), "not a sheet").unwrap();
    let command = crate::keymap::AppCommandId::new("read_document").unwrap();
    let args = crate::app::command::CommandArgs::parse_operands(
        &command,
        &[&format!("document={}", uid(SHEET + 1))],
    )
    .unwrap();
    let outcome = app.run_query_headless(&command, &args).unwrap().0;
    assert!(
        matches!(&outcome, crate::app::command::CommandOutcome::Disabled { reason } if reason.contains("could not be read")),
        "{outcome:?}"
    );
    assert!(
        crate::diagnostics::recent_entries()
            .iter()
            .any(|e| e.message.contains("Sub.snxsch") && e.message.contains("could not be read"))
    );
}

#[test]
fn an_unloaded_board_is_read_from_its_file() {
    let dir = temp_dir();
    let mut app = app_with_files(dir.path());
    let board = signex_types::pcb::PcbBoard {
        unknown: Default::default(),
        uuid: uid(77),
        version: 1,
        generator: "t".into(),
        thickness: 1.6,
        outline: vec![],
        layers: vec![],
        setup: None,
        nets: vec![],
        footprints: vec![],
        segments: vec![],
        vias: vec![],
        zones: vec![],
        graphics: vec![],
        texts: vec![],
    };
    let file = dir.path().join("Board.snxpcb");
    std::fs::write(
        &file,
        signex_types::format::SnxPcb::new(board)
            .write_string()
            .unwrap(),
    )
    .unwrap();
    let project = app.document_state.projects[0].id;
    let _ = app.handle_document_headers_read(
        project,
        vec![HeaderRead {
            path: file.clone(),
            id: read_header_id(&file),
        }],
    );
    let read = headless(
        &mut app,
        "read_document",
        &[&format!("document={}", uid(77))],
    );
    assert_eq!(read["header"]["uuid"], uid(77).to_string());
    assert!(read["revision"].is_null());
    assert!(app.document_state.tabs.is_empty());
}

/// Symbol and footprint containers repeat a `file_uuid` when a file is copied
/// outside Signex; the later editor is given a new id, in the file it would
/// write, and the first keeps its own.
#[test]
fn a_repeated_file_uuid_of_a_symbol_or_footprint_file_is_repaired() {
    let _ = crate::diagnostics::init_logging();
    let dir = temp_dir();
    let (mut app, _task) = Signex::new();
    let symbol = signex_library::SymbolFile::from_symbol(signex_library::Symbol::empty("s"));
    let footprint =
        signex_library::FootprintFile::from_footprint(signex_library::Footprint::empty("f"));
    for name in ["a.snxsym", "b.snxsym"] {
        let path = dir.path().join(name);
        app.document_state.symbol_editors.insert(
            path.clone(),
            crate::app::SymbolDocument::new(path.clone(), symbol.clone()),
        );
        app.document_state.tabs.push(crate::app::TabInfo {
            title: name.into(),
            path: path.clone(),
            cached_document: None,
            project_id: None,
            kind: crate::app::TabKind::SymbolEditor(path),
        });
    }
    for name in ["a.snxfpt", "b.snxfpt"] {
        let path = dir.path().join(name);
        app.document_state.footprint_editors.insert(
            path.clone(),
            crate::app::FootprintDocument::new(path.clone(), footprint.clone()),
        );
        app.document_state.tabs.push(crate::app::TabInfo {
            title: name.into(),
            path: path.clone(),
            cached_document: None,
            project_id: None,
            kind: crate::app::TabKind::FootprintEditor(path),
        });
    }
    let repaired = app.reconcile_document_ids();
    assert_eq!(repaired.len(), 2, "{repaired:?}");
    let docs = &app.document_state;
    let sym = |n: &str| docs.symbol_editors[&dir.path().join(n)].file.file_uuid;
    let fpt = |n: &str| docs.footprint_editors[&dir.path().join(n)].file.file_uuid;
    assert_eq!(sym("a.snxsym"), symbol.file_uuid, "the first keeps it");
    assert_ne!(sym("b.snxsym"), symbol.file_uuid);
    assert_eq!(fpt("a.snxfpt"), footprint.file_uuid);
    assert_ne!(fpt("b.snxfpt"), footprint.file_uuid);
    let b = &docs.symbol_editors[&dir.path().join("b.snxsym")];
    assert_eq!(
        b.file.file_uuid, b.baseline.file_uuid,
        "the repair is not an edit"
    );
    assert!(!b.is_dirty());
}
