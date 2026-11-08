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
                modified: None,
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
                modified: None,
                path: dir.path().join("Sub.snxsch"),
                id: DocumentId::from_uuid(uid(5)),
            },
            HeaderRead {
                modified: None,
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
            modified: None,
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
fn closing_a_project_forgets_only_its_own_files_heads() {
    let dir = temp_dir();
    let mut app = app_with_project(dir.path());
    let top = dir.path().join("Top.snxsch");
    // A file in the same folder that another open project lists.
    let other = dir.path().join("Other.snxsch");
    let ids = &mut app.document_state.document_ids;
    ids.set_header(top.clone(), DocumentId::from_uuid(uid(1)), None);
    ids.set_header(other.clone(), DocumentId::from_uuid(uid(2)), None);
    let own: Vec<_> = crate::app::document_ids::referenced_files(&app.document_state.projects[0])
        .into_iter()
        .map(|(path, _)| path)
        .collect();
    app.document_state
        .document_ids
        .forget_headers_of(own.iter().map(|p| p.as_path()));
    assert_eq!(app.document_state.document_ids.header(&top), None);
    assert!(app.document_state.document_ids.header(&other).is_some());
}

#[test]
fn a_head_goes_stale_when_the_file_changes_or_is_written_or_its_engine_goes() {
    let dir = temp_dir();
    let mut app = app_with_project(dir.path());
    let file = dir.path().join("Top.snxsch");
    let t0 = std::time::UNIX_EPOCH + std::time::Duration::from_secs(10);
    let t1 = std::time::UNIX_EPOCH + std::time::Duration::from_secs(20);
    let ids = &mut app.document_state.document_ids;
    ids.set_header(file.clone(), DocumentId::from_uuid(uid(1)), Some(t0));
    ids.forget_changed_headers(|_| Some(t0));
    assert!(
        ids.header(&file).is_some(),
        "an unchanged file keeps its head"
    );
    ids.forget_changed_headers(|_| None);
    assert!(
        ids.header(&file).is_some(),
        "an unknown time is not a change"
    );
    ids.forget_changed_headers(|_| Some(t1));
    assert!(ids.header(&file).is_none(), "a changed file is read again");
    // A head that read no id is wanted again; one that read an id is not.
    ids.set_header(file.clone(), None, Some(t0));
    assert!(ids.header_wanted(&file));
    ids.set_header(file.clone(), DocumentId::from_uuid(uid(1)), Some(t0));
    assert!(!ids.header_wanted(&file));
    // Dropping the engine of a file drops the head read before it was open.
    app.document_state.engines.insert(
        file.clone(),
        signex_engine::Engine::new(crate::app::command::queries::tests::sheet(SHEET)).unwrap(),
    );
    app.document_state.remove_engine(&file);
    assert!(app.document_state.document_ids.header(&file).is_none());
}

#[test]
fn only_files_not_yet_read_are_read() {
    let dir = temp_dir();
    let mut app = app_with_project(dir.path());
    let project = app.document_state.projects[0].id;
    assert!(app.read_missing_document_headers(project).units() > 0);
    for name in ["Top.snxsch", "Sub.snxsch", "Board.snxpcb"] {
        app.document_state.document_ids.set_header(
            dir.path().join(name),
            DocumentId::from_uuid(uid(9)),
            None,
        );
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
    // The board was repaired as it opened; the project is left for now.
    assert_eq!(repaired.len(), 0, "{repaired:?}");
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
            modified: None,
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
            modified: None,
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
    // The board was repaired as it opened; the project is left for now.
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

/// A sheet loaded only for an answer is not repaired: it is dropped after, so
/// a new id would name a document that never keeps it.
#[test]
fn a_sheet_loaded_only_for_a_query_keeps_the_id_its_file_holds() {
    let dir = temp_dir();
    let mut app = app_with_files(dir.path());
    // An open sheet elsewhere repeats the id of the unloaded Sub.
    crate::app::handlers::editing_commands::net_color_driver::add_sheet_with(
        &mut app,
        &path("open.snxsch"),
        crate::app::command::queries::tests::sheet(SHEET + 1),
    );
    let found = headless(&mut app, "find_items", &["kind=symbol", "designator=U3"]);
    let in_sub: Vec<_> = found["elements"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| {
            e["document"]["path"]
                .as_str()
                .is_some_and(|p| p.ends_with("Sub.snxsch"))
        })
        .collect();
    assert_eq!(in_sub.len(), 1);
    assert_eq!(in_sub[0]["document"]["id"], uid(SHEET + 1).to_string());
}

/// A repaired duplicate: the project's waivers filed under the shared id hold
/// for the repaired sheet too, and the project is marked as holding a change.
#[test]
fn repairing_a_duplicate_sheet_keeps_its_net_waivers() {
    use crate::app::handlers::editing_commands::net_color_driver::add_sheet_with;
    use signex_types::project::{ErcWaiver, ProjectRules};
    let dir = temp_dir();
    let mut app = app_with_project(dir.path());
    let shared = uid(SHEET);
    app.document_state.projects[0].data.rules = Some(ProjectRules {
        erc_waivers: vec![ErcWaiver {
            rule_id: "builtin::single_pin_net".into(),
            objects: vec![format!("net:SENSE@{shared}")],
            fingerprint: "f".into(),
            reason: "r".into(),
            author: "a".into(),
            ..Default::default()
        }],
        ..Default::default()
    });
    for name in ["Top", "Sub"] {
        let sheet = crate::app::command::queries::tests::sheet(SHEET);
        add_sheet_with(&mut app, &dir.path().join(format!("{name}.snxsch")), sheet);
        app.document_state
            .mark_document_clean(&dir.path().join(format!("{name}.snxsch")));
    }
    let repaired = app.reconcile_document_ids();
    assert_eq!(repaired.len(), 1);
    let new = app.document_state.engines[&dir.path().join("Sub.snxsch")]
        .document()
        .uuid;
    let waivers = &app.document_state.projects[0]
        .data
        .rules
        .as_ref()
        .unwrap()
        .erc_waivers;
    let mut held: Vec<_> = waivers.iter().map(|w| w.objects[0].clone()).collect();
    held.sort();
    let mut want = vec![format!("net:SENSE@{shared}"), format!("net:SENSE@{new}")];
    want.sort();
    assert_eq!(held, want);
    assert!(
        app.document_state
            .is_document_dirty(&dir.path().join("P.snxprj"))
    );
}

fn write_sheet_file(path: &Path, id: uuid::Uuid) {
    let mut sheet = crate::app::command::queries::tests::sheet(SHEET);
    sheet.uuid = id;
    signex_engine::Engine::new_with_path(sheet, Some(path.to_path_buf()))
        .unwrap()
        .save()
        .unwrap();
}

/// Open the sheet file at `path` in a tab, as a File > Open does.
fn open_tab(app: &mut Signex, path: &Path) {
    let text = std::fs::read_to_string(path).unwrap();
    let sheet = signex_types::format::SnxSchematic::parse(&text)
        .unwrap()
        .sheet;
    app.open_schematic_tab(path.to_path_buf(), "tab".into(), sheet);
    app.document_state.mark_document_clean(path);
}

fn close_tab_of(app: &mut Signex, path: &Path) {
    let index = app
        .document_state
        .tabs
        .iter()
        .position(|t| t.path == path)
        .unwrap();
    let _ = app.close_tab_now(index);
}

fn run_by_document(app: &mut Signex, name: &str, operands: &[&str]) {
    let command = crate::keymap::AppCommandId::new(name).unwrap();
    let args = crate::app::command::CommandArgs::parse_operands(&command, operands).unwrap();
    let (outcome, task) = app.run_operands(&command, &args);
    assert_eq!(
        outcome,
        crate::app::command::CommandOutcome::Ran,
        "{name}: {outcome:?}"
    );
    crate::task_driver::TaskDriver::new()
        .settle(app, task)
        .unwrap();
}

fn listed_ids(app: &mut Signex, dir: &Path) -> Vec<(String, Option<String>)> {
    let value = headless(app, "list_documents", &[]);
    let mut out: Vec<_> = value["documents"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["kind"] == "schematic")
        .map(|d| {
            let name = Path::new(d["path"].as_str().unwrap())
                .strip_prefix(dir)
                .unwrap()
                .display()
                .to_string();
            (name, d["id"].as_str().map(str::to_string))
        })
        .collect();
    out.sort();
    out
}

/// A sheet with no id is given one, saved by it and its tab closed: it is
/// still listed with the id its file now holds, and that id still resolves.
#[test]
fn a_saved_then_closed_sheet_keeps_its_listed_id() {
    let dir = temp_dir();
    write_sheet_file(&dir.path().join("Top.snxsch"), uid(SHEET));
    let sub = dir.path().join("Sub.snxsch");
    write_sheet_file(&sub, uuid::Uuid::nil());
    let mut app = app_with_project(dir.path());
    app.read_wanted_document_headers_now();
    open_tab(&mut app, &sub);
    let _ = headless(&mut app, "list_documents", &[]);
    let minted = app.document_state.engines[&sub].document().uuid;
    assert!(!minted.is_nil());
    let doc = format!("document={minted}");
    run_by_document(&mut app, "color_net", &["at=5,0", "color=#336699", &doc]);
    run_by_document(&mut app, "save_document", &[&doc]);
    close_tab_of(&mut app, &sub);
    assert!(!app.document_state.engines.contains_key(&sub));

    let listed = listed_ids(&mut app, dir.path());
    assert_eq!(
        listed[0],
        ("Sub.snxsch".to_string(), Some(minted.to_string()))
    );
    let found = headless(&mut app, "document_info", &[&doc]);
    assert_eq!(found["path"], sub.display().to_string());
}

/// Two files with one id: the later is repaired and saved; once both tabs
/// are closed each id resolves to its own file.
#[test]
fn after_a_repaired_duplicate_is_saved_and_closed_each_id_names_its_own_file() {
    let dir = temp_dir();
    let (top, sub) = (dir.path().join("Top.snxsch"), dir.path().join("Sub.snxsch"));
    write_sheet_file(&top, uid(SHEET));
    write_sheet_file(&sub, uid(SHEET));
    let mut app = app_with_project(dir.path());
    open_tab(&mut app, &top);
    open_tab(&mut app, &sub);
    let _ = headless(&mut app, "list_documents", &[]);
    let repaired = app.document_state.engines[&sub].document().uuid;
    assert_ne!(repaired, uid(SHEET), "the later one was repaired");
    run_by_document(
        &mut app,
        "color_net",
        &["at=5,0", "color=#336699", &format!("document={repaired}")],
    );
    run_by_document(
        &mut app,
        "save_document",
        &[&format!("document={repaired}")],
    );
    close_tab_of(&mut app, &top);
    close_tab_of(&mut app, &sub);

    let listed = listed_ids(&mut app, dir.path());
    assert_eq!(
        listed,
        [
            ("Sub.snxsch".to_string(), Some(repaired.to_string())),
            ("Top.snxsch".to_string(), Some(uid(SHEET).to_string())),
        ]
    );
    for (id, file) in [(repaired, &sub), (uid(SHEET), &top)] {
        let info = headless(&mut app, "document_info", &[&format!("document={id}")]);
        assert_eq!(info["path"], file.display().to_string());
    }
}

/// A read-only query must not move `documents_epoch`: it cancels a person's
/// running Annotate, Replace All and project read.
#[test]
fn a_headless_query_does_not_move_the_documents_epoch() {
    let dir = temp_dir();
    let mut app = app_with_files(dir.path());
    let epoch = app.document_state.documents_epoch;
    let _ = headless(&mut app, "resolve", &[&format!("id={}", uid(7))]);
    let _ = headless(
        &mut app,
        "read_document",
        &[&format!("document={}", uid(SHEET))],
    );
    let _ = headless(&mut app, "find_items", &["designator=U3"]);
    assert_eq!(app.document_state.documents_epoch, epoch);
}

/// A person opening a copied file is told and the copy is repaired as it
/// opens, with no agent asking.
#[test]
fn opening_a_copied_file_repairs_and_warns_at_once() {
    let _ = crate::diagnostics::init_logging();
    let dir = temp_dir();
    let (a, b) = (dir.path().join("a.snxsch"), dir.path().join("b.snxsch"));
    write_sheet_file(&a, uid(SHEET));
    write_sheet_file(&b, uid(SHEET));
    let (mut app, _task) = Signex::new();
    let mut driver = crate::task_driver::TaskDriver::new();
    for file in [&a, &b] {
        driver
            .send(
                &mut app,
                crate::app::Message::File(crate::app::FileMsg::Opened(Some(file.clone()))),
            )
            .unwrap();
    }
    let id = |p: &std::path::PathBuf| app.document_state.engines[p].document().uuid;
    assert_eq!(id(&a), uid(SHEET), "the first opened keeps its id");
    assert_ne!(id(&b), uid(SHEET), "the copy is repaired on open");
    assert!(
        crate::diagnostics::recent_entries()
            .iter()
            .any(|e| e.message.contains("b.snxsch") && e.message.contains("same document id"))
    );
    assert!(!app.document_state.is_document_dirty(&b));
}

/// The query commands answer an agent: a palette, key or menu call is
/// refused with the reason and the palette does not list them.
#[test]
fn a_query_is_refused_in_the_gui_and_not_a_palette_row() {
    let (mut app, _task) = Signex::new();
    for name in [
        "list_documents",
        "document_info",
        "resolve",
        "find_items",
        "read_document",
    ] {
        let command = crate::keymap::AppCommandId::new(name).unwrap();
        let (outcome, task) = app.dispatch_command_reporting(
            &command,
            crate::app::command::CommandArgs::none(),
            None,
        );
        assert!(
            matches!(&outcome, crate::app::command::CommandOutcome::Disabled { reason } if reason.contains("agents and the CLI")),
            "{name}: {outcome:?}"
        );
        assert_eq!(task.units(), 0);
        assert!(
            !crate::app::command::bridge::is_dispatchable(&command),
            "{name}"
        );
    }
}
