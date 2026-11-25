//! #2470 — `back_annotate_schematics document=..` writes the rows the
//! dialog ticks by default, as its Execute does, and answers with each row.

use std::path::{Path, PathBuf};

use signex_types::format::SnxSchematic;
use signex_types::pcb::{Footprint, PcbBoard, SymbolLink};
use signex_types::schematic::{InstancePath, SchematicSheet, Symbol};
use uuid::Uuid;

use super::BackAnnotateMsg;
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::path_ops::settle_task;
use crate::app::{Message, Signex, TabInfo, TabKind};
use crate::keymap::AppCommandId;

const R1: Uuid = Uuid::from_u128(0x101);
const FP_R1: Uuid = Uuid::from_u128(0xf1);

struct Rig {
    app: Signex,
    project: PathBuf,
    root: PathBuf,
    board: PathBuf,
    _tmp: tempfile::TempDir,
}

fn root_sheet() -> SchematicSheet {
    SchematicSheet {
        uuid: Uuid::from_u128(0x5),
        paper_size: "A4".to_string(),
        symbols: vec![Symbol {
            uuid: R1,
            lib_id: "R".to_string(),
            reference: "R1".to_string(),
            value: "10k".to_string(),
            in_bom: true,
            on_board: true,
            ..Default::default()
        }],
        ..Default::default()
    }
}

/// The board after edits on it: R1 renamed R101 and given 22k.
fn board() -> PcbBoard {
    let mut board: PcbBoard =
        serde_json::from_value(serde_json::json!({ "uuid": Uuid::nil() })).expect("board");
    let mut fp: Footprint = serde_json::from_value(serde_json::json!({
        "uuid": FP_R1, "reference": "R101", "value": "22k",
        "position": {"x": 0.0, "y": 0.0}, "layer": "TopCopper"
    }))
    .expect("footprint");
    fp.schematic_link = Some(SymbolLink::new(InstancePath::root(), [R1]));
    board.footprints = vec![fp];
    board
}

/// A flat project with its root sheet and its board open.
fn rig() -> Rig {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path().to_path_buf();
    let root = dir.join("root.snxsch");
    let text = SnxSchematic::new(root_sheet())
        .write_string()
        .expect("sheet");
    std::fs::write(&root, text).expect("write sheet");
    let data = signex_types::project::ProjectData {
        document_id: None,
        description: None,
        name: "Proj".to_string(),
        dir: dir.to_string_lossy().into_owned(),
        schematic_root: Some("root.snxsch".to_string()),
        pcb_file: Some("Proj.snxpcb".to_string()),
        sheets: vec![signex_types::project::SheetEntry {
            name: "root.snxsch".to_string(),
            filename: "root.snxsch".to_string(),
            symbols_count: 0,
            wires_count: 0,
            labels_count: 0,
            unknown: Default::default(),
        }],
        variant_definitions: Vec::new(),
        active_variant: None,
        variant_settings: Default::default(),
        libraries: Vec::new(),
        enable_git: false,
        net_classes: Vec::new(),
        rules: None,
        annotation: None,
        schematic_defaults: Default::default(),
        project_defaults: Default::default(),
        agent_journal: Default::default(),
        schematic_design: Default::default(),
        fab_outputs: None,
        router_tool_diameter: None,
        requirements: Default::default(),
        parameters: Default::default(),
        unknown: Default::default(),
    };
    let project = dir.join("Proj.snxprj");
    signex_types::project::write_project(&project, &data).expect("write project");

    let (mut app, _task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.main_window_id = Some(iced::window::Id::unique());
    let id = app.document_state.mint_project_id();
    app.document_state.projects.push(crate::app::LoadedProject {
        id,
        path: project.clone(),
        data,
        pending_libraries: Default::default(),
        case_fold: Default::default(),
        case_probed: false,
        case_decided: false,
    });
    app.document_state.active_project = Some(id);
    let board_path = dir.join("Proj.snxpcb");
    app.open_pcb_tab(board_path.clone(), "Proj".into(), board());
    let engine =
        signex_engine::Engine::new_with_path(root_sheet(), Some(root.clone())).expect("engine");
    app.document_state.engines.insert(root.clone(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "root".to_string(),
        path: root.clone(),
        cached_document: None,
        project_id: Some(id),
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.document_state.active_path = Some(root.clone());
    Rig {
        app,
        project,
        root,
        board: board_path,
        _tmp: tmp,
    }
}

fn r1(app: &Signex, root: &Path) -> (String, String) {
    let symbol = (app.document_state.engines[root].document().symbols.iter())
        .find(|s| s.uuid == R1)
        .expect("R1");
    (symbol.reference.clone(), symbol.value.clone())
}

fn run(app: &mut Signex, operands: &[&str]) -> CommandOutcome {
    let command = AppCommandId::new("back_annotate_schematics").expect("an id");
    let args = CommandArgs::parse_operands(&command, operands).expect("operands parse");
    let (outcome, task) = app.run_headless_command_in_test(&command, args);
    settle_task(app, task);
    outcome
}

/// The answer's rows that apply, as (kind, old, new).
fn applying_rows(outcome: &CommandOutcome) -> Vec<(String, String, String)> {
    let CommandOutcome::Answered(answer) = outcome else {
        panic!("Back Annotate answers with its rows: {outcome:?}");
    };
    let text = |value: &serde_json::Value| value.as_str().expect("text").to_string();
    (answer["rows"].as_array().expect("rows").iter())
        .filter(|row| row["applies"] == true)
        .map(|row| (text(&row["kind"]), text(&row["old"]), text(&row["new"])))
        .collect()
}

fn pair(a: &str, b: &str) -> (String, String) {
    (a.to_string(), b.to_string())
}

#[test]
fn the_operand_form_writes_what_the_dialogs_execute_writes() {
    let mut gui = rig();
    let task = gui.app.update(Message::from(BackAnnotateMsg::Requested));
    settle_task(&mut gui.app, task);
    let task = gui
        .app
        .update(Message::from(BackAnnotateMsg::ExecuteRequested));
    settle_task(&mut gui.app, task);

    let mut cli = rig();
    let document = format!("document={}", cli.project.display());
    let outcome = run(&mut cli.app, &[&document]);

    assert_eq!(
        r1(&gui.app, &gui.root),
        pair("R101", "22k"),
        "the dialog's run"
    );
    assert_eq!(r1(&cli.app, &cli.root), r1(&gui.app, &gui.root));
    let mut rows = applying_rows(&outcome);
    rows.sort();
    let row =
        |kind: &str, old: &str, new: &str| (kind.to_string(), old.to_string(), new.to_string());
    assert_eq!(
        rows,
        [row("designator", "R1", "R101"), row("value", "10k", "22k")]
    );
    let CommandOutcome::Answered(answer) = &outcome else {
        unreachable!()
    };
    assert_eq!(answer["applied"], true);
    assert_eq!(answer["sheets_changed"], 1);
    assert!(cli.app.document_state.is_document_dirty(&cli.root));
    assert!(cli.app.ui_state.back_annotate.is_none(), "no dialog");
    // One step: a single undo takes both rows back.
    let engine = cli
        .app
        .document_state
        .engines
        .get_mut(&cli.root)
        .expect("root");
    assert!(engine.undo().expect("undo").is_some());
    assert_eq!(r1(&cli.app, &cli.root), pair("R1", "10k"));
}

#[test]
fn a_dry_run_lists_the_rows_and_changes_nothing() {
    let mut f = rig();
    let document = format!("document={}", f.board.display());
    let outcome = run(&mut f.app, &[&document, "dry_run=true"]);
    assert_eq!(applying_rows(&outcome).len(), 2, "{outcome:?}");
    let CommandOutcome::Answered(answer) = &outcome else {
        unreachable!()
    };
    assert_eq!(answer["applied"], false);
    assert_eq!(r1(&f.app, &f.root), pair("R1", "10k"));
    assert!(!f.app.document_state.is_document_dirty(&f.root));
}

#[test]
fn a_project_whose_board_is_closed_is_refused() {
    let mut f = rig();
    f.app.document_state.projects[0].data.pcb_file = Some("Other.snxpcb".into());
    let document = format!("document={}", f.root.display());
    let outcome = run(&mut f.app, &[&document]);
    assert!(
        matches!(&outcome, CommandOutcome::Disabled { reason } if signex_i18n::RenderTranslatable::render_en_us(reason).contains("open it first")),
        "{outcome:?}"
    );
    assert_eq!(r1(&f.app, &f.root), pair("R1", "10k"));
}
