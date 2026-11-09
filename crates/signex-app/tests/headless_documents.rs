//! #2172: documents carry a persistent id that an agent reads and addresses,
//! through the public headless entry. The config-root override is process-wide,
//! so each scenario runs in a child process of this binary ([`probe`]).

#![allow(
    clippy::disallowed_methods,
    reason = "the test re-runs its own binary as a child process"
)]

use std::path::{Path, PathBuf};

use serde_json::Value;
use signex_app::app::Signex;
use signex_app::config_root::set_config_root_override;
use signex_app::fonts::catalog::{FontSet, install_before_boot};
use signex_app::headless::{CommandArgs, CommandOutcome, OperandError};
use signex_app::keymap::AppCommandId;
use signex_app::task_driver::TaskDriver;
use signex_types::project::{ErcRules, ProjectData, RuleProfile, SheetEntry};
use signex_types::schematic::{Point, SchematicSheet, Wire};

const PROBE_ENV: &str = "SIGNEX_HEADLESS_DOCUMENTS_PROBE";

const TOP: &str = "0193a2f0-7b1c-7cde-8123-000000000001";
const SUB: &str = "0193a2f0-7b1c-7cde-8123-000000000002";
const BOARD: &str = "0193a2f0-7b1c-7cde-8123-000000000003";

fn run_probe(scenario: &str) {
    let output = std::process::Command::new(std::env::current_exe().expect("test binary"))
        .args(["probe", "--exact", "--nocapture", "--test-threads=1"])
        .env(PROBE_ENV, scenario)
        .output()
        .expect("run the probe");
    assert!(
        output.status.success(),
        "scenario {scenario} failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn a_projects_documents_keep_their_ids_across_close_reopen_and_restart() {
    run_probe("ids_persist");
}

#[test]
fn a_headless_document_command_names_its_document_and_never_falls_back() {
    run_probe("by_document");
}

#[test]
fn save_as_makes_a_new_document_and_a_copied_file_gets_a_new_id() {
    run_probe("save_as_and_copies");
}

#[test]
fn probe() {
    let Ok(scenario) = std::env::var(PROBE_ENV) else {
        return;
    };
    match scenario.as_str() {
        "ids_persist" => ids_persist(),
        "by_document" => by_document(),
        "save_as_and_copies" => save_as_and_copies(),
        "new_project" => new_project(),
        other => panic!("unknown scenario {other}"),
    }
}

fn rules() -> RuleProfile {
    let mut erc = ErcRules::default();
    erc.severity.insert("unused_pin".into(), "off".into());
    RuleProfile::new("Headless", "1", erc)
}

fn id(name: &str) -> AppCommandId {
    AppCommandId::new(name).expect("a valid id")
}

/// A headless app and the driver that runs its tasks.
struct Session {
    app: Signex,
    driver: TaskDriver,
}

impl Session {
    fn boot() -> Self {
        let (mut app, boot) = Signex::boot_headless(Vec::new(), &rules()).expect("boot");
        let mut driver = TaskDriver::new();
        driver.settle(&mut app, boot).expect("boot settled");
        Self { app, driver }
    }

    fn run(&mut self, name: &str, operands: &[&str]) -> CommandOutcome {
        let command = id(name);
        let args = if operands.is_empty() {
            CommandArgs::none()
        } else {
            CommandArgs::parse_operands(&command, operands).expect("operands")
        };
        let (outcome, task) = self.app.dispatch_command_headless(&command, args);
        self.driver.settle(&mut self.app, task).expect("settled");
        outcome
    }

    fn answer(&mut self, name: &str, operands: &[&str]) -> Value {
        match self.run(name, operands) {
            CommandOutcome::Answered(value) => value,
            other => panic!("{name} {operands:?} was not answered: {other:?}"),
        }
    }

    /// Open a file through the Open dialog's answer.
    fn open(&mut self, path: &Path) -> Value {
        self.app
            .headless_answers()
            .expect("headless answers")
            .replace_all([("path".to_string(), path.display().to_string())]);
        match self.run("open_document", &[]) {
            CommandOutcome::Answered(value) => value,
            other => panic!("open was not answered: {other:?}"),
        }
    }

    /// The listed documents, by file name.
    fn listed(&mut self) -> Vec<Value> {
        self.answer("list_documents", &[])["documents"]
            .as_array()
            .expect("documents")
            .clone()
    }

    fn entry(&mut self, name: &str) -> Value {
        self.listed()
            .into_iter()
            .find(|d| {
                d["path"]
                    .as_str()
                    .is_some_and(|p| Path::new(p).file_name().is_some_and(|f| f == name))
            })
            .unwrap_or_else(|| panic!("{name} is not listed"))
    }
}

fn wire_sheet(id: &str) -> SchematicSheet {
    SchematicSheet {
        uuid: id.parse().expect("uuid"),
        paper_size: "A4".into(),
        wires: vec![Wire {
            color: None,
            uuid: uuid::Uuid::now_v7(),
            start: Point::new(0.0, 0.0),
            end: Point::new(10.0, 0.0),
            stroke_width: 0.0,
            unknown_columns: Default::default(),
            locked: false,
        }],
        ..Default::default()
    }
}

fn write_sheet(path: &Path, id: &str) {
    let mut engine = signex_engine::Engine::new_with_path(wire_sheet(id), Some(path.to_path_buf()))
        .expect("engine");
    engine.save().expect("sheet file");
}

fn write_board(path: &Path, id: &str) {
    let mut board = signex_types::pcb::PcbBoard {
        sketch: None,
        unknown: Default::default(),
        uuid: id.parse().expect("uuid"),
        version: 1,
        generator: "test".into(),
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
    board.uuid = id.parse().expect("uuid");
    let text = signex_types::format::SnxPcb::new(board)
        .write_string()
        .expect("board text");
    std::fs::write(path, text).expect("board file");
}

/// A project of two sheets and a board, none opened yet, with no id of its own.
fn write_project(dir: &Path) -> PathBuf {
    write_sheet(&dir.join("Top.snxsch"), TOP);
    write_sheet(&dir.join("Sub.snxsch"), SUB);
    write_board(&dir.join("Board.snxpcb"), BOARD);
    let sheet = |name: &str| SheetEntry {
        name: name.into(),
        filename: format!("{name}.snxsch"),
        symbols_count: 0,
        wires_count: 0,
        labels_count: 0,
        unknown: Default::default(),
    };
    let data = ProjectData {
        document_id: None,
        description: None,
        name: "P".into(),
        dir: String::new(),
        schematic_root: Some("Top.snxsch".into()),
        pcb_file: Some("Board.snxpcb".into()),
        sheets: vec![sheet("Top"), sheet("Sub")],
        variant_definitions: Vec::new(),
        active_variant: None,
        libraries: Vec::new(),
        enable_git: false,
        net_classes: Vec::new(),
        rules: None,
        annotation: None,
        schematic_defaults: Default::default(),
        project_defaults: Default::default(),
        parameters: Default::default(),
        unknown: Default::default(),
    };
    let path = dir.join("P.snxprj");
    signex_types::project::write_project(&path, &data).expect("project file");
    path
}

fn isolated() -> tempfile::TempDir {
    let root = tempfile::tempdir().expect("override root");
    set_config_root_override(root.path().to_path_buf()).expect("override");
    install_before_boot(FontSet::BundledOnly).expect("fonts");
    root
}

fn text(value: &Value) -> &str {
    value.as_str().expect("text")
}

/// Every document a project references is listed with its id as soon as the
/// project opens, tab or not; the ids survive a tab close and reopen and an
/// app restart; a project written by an older build gets one on open that the
/// next save writes and the next open reads back.
fn ids_persist() {
    let root = isolated();
    let dir = root.path().join("proj");
    std::fs::create_dir(&dir).expect("project dir");
    let project = write_project(&dir);
    let project_bytes = std::fs::read(&project).expect("project bytes");

    let mut session = Session::boot();
    let opened = session.open(&project);
    assert_eq!(opened["kind"], "project");
    assert_eq!(opened["path"], project.display().to_string());
    assert!(opened["id"].is_string());

    // Listed before any tab opens: ids read from the file heads.
    let top = session.entry("Top.snxsch");
    assert_eq!(top["id"], TOP);
    assert_eq!(top["loaded"], false);
    assert_eq!(top["kind"], "schematic");
    assert_eq!(session.entry("Sub.snxsch")["id"], SUB);
    let board = session.entry("Board.snxpcb");
    assert_eq!(board["id"], BOARD);
    assert_eq!(board["kind"], "pcb");
    let listed_project = session.entry("P.snxprj");
    assert_eq!(listed_project["kind"], "project");
    assert_eq!(listed_project["loaded"], true);
    assert_eq!(top["project"]["path"], listed_project["path"]);
    let project_id = text(&listed_project["id"]).to_string();
    assert_eq!(top["project"]["id"], project_id.as_str());
    assert_eq!(
        std::fs::read(&project).expect("project bytes"),
        project_bytes,
        "minting an id on open writes nothing and marks nothing dirty"
    );
    assert!(listed_project["dirty"] == false);

    // An unloaded sheet is read without a tab (#2172).
    let read = session.answer("read_document", &[&format!("document={SUB}")]);
    assert_eq!(read["document"]["loaded"], false);
    assert!(read["total"].is_number() && read["revision"].is_null());
    assert_eq!(
        session.entry("Sub.snxsch")["loaded"],
        false,
        "nothing is kept"
    );
    assert!(
        session.app.document_state.tabs.is_empty(),
        "no tab was made"
    );

    // A tab: loaded now, same id; closed, still listed with it; reopened, same.
    let opened = session.open(&dir.join("Top.snxsch"));
    assert_eq!(opened["id"], TOP);
    assert_eq!(opened["kind"], "schematic");
    let open = session.entry("Top.snxsch");
    assert_eq!(
        (open["id"].as_str(), open["loaded"].as_bool()),
        (Some(TOP), Some(true))
    );
    let by_id = format!("document={TOP}");
    assert_eq!(
        session.run("close_active_document", &[&by_id]),
        CommandOutcome::Ran
    );
    let closed = session.entry("Top.snxsch");
    assert_eq!(
        (closed["id"].as_str(), closed["loaded"].as_bool()),
        (Some(TOP), Some(false))
    );
    session.open(&dir.join("Top.snxsch"));
    assert_eq!(session.entry("Top.snxsch")["id"], TOP);

    // The next save of an edited project writes the id it minted on open (a
    // clean project has nothing to write).
    session.app.document_state.mark_document_dirty(&project);
    let by_project = format!("document={project_id}");
    assert_eq!(
        session.run("save_document", &[&by_project]),
        CommandOutcome::Ran
    );
    let saved = signex_types::project::parse_project(&project).expect("saved project");
    assert_eq!(
        saved.document_id.map(|id| id.to_string()),
        Some(project_id.clone())
    );

    // A restart reads every id back.
    let mut restarted = Session::boot();
    restarted.open(&project);
    assert_eq!(restarted.entry("P.snxprj")["id"], project_id.as_str());
    assert_eq!(restarted.entry("Top.snxsch")["id"], TOP);
    assert_eq!(restarted.entry("Board.snxpcb")["id"], BOARD);
}

/// Every document command names its document and never falls back to the
/// active one; named by id or by path it acts on that document and leaves the
/// tab, selection and the other document as they were.
fn by_document() {
    let root = isolated();
    let a = root.path().join("a.snxsch");
    let b = root.path().join("b.snxsch");
    write_sheet(&a, TOP);
    write_sheet(&b, SUB);
    let mut session = Session::boot();
    session.open(&a);
    session.open(&b); // b is the active tab

    // No document: refused with the reason, never run on the active one.
    for name in [
        "undo",
        "redo",
        "save_document",
        "close_active_document",
        "clear_all_net_colors",
        "color_net",
    ] {
        let (outcome, _task) = session
            .app
            .dispatch_command_headless(&id(name), CommandArgs::none());
        assert!(
            matches!(&outcome, CommandOutcome::Disabled { reason } if reason.contains("needs the operand")),
            "{name}: {outcome:?}"
        );
    }
    // With the other operands but no document, the operands do not parse.
    for (name, operands) in [
        ("undo", vec![]),
        ("clear_all_net_colors", vec![]),
        ("color_net", vec!["at=5,0", "color=#336699"]),
    ] {
        let error = CommandArgs::parse_operands(&id(name), &operands).err();
        assert!(
            matches!(&error, Some(OperandError::Missing { operand, .. }) if operand == "document"),
            "{name}: {error:?}"
        );
    }

    // By id and by path on the background sheet a, while b stays the active tab.
    let active_before = session.app.document_state.active_path.clone();
    assert_eq!(active_before.as_deref(), Some(b.as_path()));
    let by_id = format!("document={TOP}");
    let by_path = format!("document={}", a.display());
    let painted = |s: &Session, path: &Path| {
        s.app.document_state.engines[path]
            .document()
            .net_colors
            .len()
    };
    for target in [&by_id, &by_path] {
        assert_eq!(
            session.run("color_net", &["at=5,0", "color=#336699", target]),
            CommandOutcome::Ran
        );
        assert_eq!(painted(&session, &a), 1, "a was coloured");
        assert_eq!(painted(&session, &b), 0, "b was not");
        assert!(session.app.document_state.is_document_dirty(&a));
        assert_eq!(session.run("undo", &[target]), CommandOutcome::Ran);
        assert_eq!(painted(&session, &a), 0);
        assert_eq!(session.run("redo", &[target]), CommandOutcome::Ran);
        assert_eq!(painted(&session, &a), 1);
        assert_eq!(session.run("undo", &[target]), CommandOutcome::Ran);
        assert_eq!(
            session.app.document_state.active_path, active_before,
            "the active tab never moved"
        );
    }
    assert_eq!(
        session.entry("a.snxsch")["dirty"],
        false,
        "undone back to the save"
    );
    assert_eq!(
        session.entry("b.snxsch")["dirty"],
        false,
        "b was never touched"
    );

    // A named document that is not open is refused with a reason.
    let missing = format!("document={}", root.path().join("nope.snxsch").display());
    let outcome = session.run("save_document", &[&missing]);
    assert!(
        matches!(&outcome, CommandOutcome::Disabled { reason } if reason.contains("not an open")),
        "{outcome:?}"
    );

    // Save by id writes that document's file, and only that one.
    session.run("color_net", &["at=5,0", "color=#336699", &by_id]);
    let b_before = std::fs::read(&b).expect("b");
    assert_eq!(session.run("save_document", &[&by_id]), CommandOutcome::Ran);
    let saved = signex_types::format::SnxSchematic::parse(&std::fs::read_to_string(&a).expect("a"))
        .expect("a reparses")
        .sheet;
    assert_eq!(saved.net_colors.len(), 1, "the file holds the colour");
    assert_eq!(std::fs::read(&b).expect("b"), b_before, "b was not written");
    assert_eq!(session.entry("a.snxsch")["dirty"], false, "saved");
    assert_eq!(session.app.document_state.active_path, active_before);

    // Close by id closes that tab and leaves the active one.
    assert_eq!(
        session.run("close_active_document", &[&by_id]),
        CommandOutcome::Ran
    );
    let still_listed = session
        .listed()
        .iter()
        .any(|d| d["path"].as_str().is_some_and(|p| p.ends_with("a.snxsch")));
    assert!(
        !still_listed,
        "a loose clean sheet is gone once its tab closed"
    );
    assert_eq!(session.app.document_state.active_path, active_before);
}

/// Save As gives the copy a new id and leaves the original's; a file copied
/// outside Signex repeats an id, and the later one opened is given a new one.
fn save_as_and_copies() {
    let root = isolated();
    let a = root.path().join("a.snxsch");
    write_sheet(&a, TOP);
    let mut session = Session::boot();
    session.open(&a);

    let copy = root.path().join("copy.snxsch");
    let operands = [
        format!("document={TOP}"),
        format!("path={}", copy.display()),
    ];
    let refs: Vec<&str> = operands.iter().map(String::as_str).collect();
    assert_eq!(session.run("save_document_as", &refs), CommandOutcome::Ran);
    let original_on_disk =
        signex_types::format::document_id_in_header("snxsch", &std::fs::read(&a).expect("a"));
    assert_eq!(
        original_on_disk.map(|id| id.to_string()).as_deref(),
        Some(TOP)
    );
    let copy_id =
        signex_types::format::document_id_in_header("snxsch", &std::fs::read(&copy).expect("copy"))
            .expect("the copy carries an id")
            .to_string();
    assert_ne!(copy_id, TOP, "Save As is a new document");
    // The open document is now the copy, and says so.
    let listed = session.entry("copy.snxsch");
    assert_eq!(listed["id"], copy_id.as_str());

    // A file copied outside Signex repeats the id of the original.
    let outside = root.path().join("outside.snxsch");
    std::fs::copy(&a, &outside).expect("copy on disk");
    session.open(&a);
    session.open(&outside);
    let first = session.entry("a.snxsch");
    let second = session.entry("outside.snxsch");
    assert_eq!(first["id"], TOP);
    assert_ne!(second["id"], TOP, "the later copy gets a new id");
    assert!(second["id"].is_string());
}

#[test]
fn new_document_answers_with_the_project_it_made() {
    run_probe("new_project");
}

/// New Project runs to its end inside the dispatch and answers with the id,
/// path and kind of the project it created.
fn new_project() {
    let root = isolated();
    let mut session = Session::boot();
    let chosen = root.path().join("Made.snxprj");
    session
        .app
        .headless_answers()
        .expect("headless answers")
        .replace_all([("path".to_string(), chosen.display().to_string())]);
    let answer = match session.run("new_document", &[]) {
        CommandOutcome::Answered(value) => value,
        other => panic!("new_document was not answered: {other:?}"),
    };
    assert_eq!(answer["kind"], "project");
    let path = PathBuf::from(text(&answer["path"]));
    assert!(
        path.is_file(),
        "the project file exists: {}",
        path.display()
    );
    let written = signex_types::project::parse_project(&path).expect("project");
    assert_eq!(
        written.document_id.map(|id| id.to_string()).as_deref(),
        answer["id"].as_str(),
        "the id is the one stored in the new project"
    );
    assert_eq!(session.entry("Made.snxprj")["id"], answer["id"]);
}
