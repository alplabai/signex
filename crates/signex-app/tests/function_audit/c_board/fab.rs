//! Mockup row 115, Fabrication Outputs (decisions 2026-11-15, 2026-11-18 and
//! 2026-11-19): the ticks, the ODB++ folder box, the whole-panel box, the
//! destination, Generate and its outcome, Cancel. Nothing is written before
//! Generate and the board is never edited.

use signex_app::app::{
    ContextMenuMsg, FabOutput, FabOutputsMsg, FabProblem, FileMsg, LoadedProject, Message,
    ProjectTreeAction,
};
use signex_output::fab::FabError;
use signex_types::format::SnxPcb;
use signex_types::pcb_panel::BoardPanel;
use signex_types::project::{FabOutputKinds, ProjectData, parse_project, write_project};

use super::support::{Bench, audit_board, open_board};

fn send(b: &mut Bench, msg: FabOutputsMsg) {
    b.send(Message::from(msg));
}

fn opened() -> Bench {
    let mut b = open_board(audit_board());
    b.command("fabrication_outputs");
    b
}

/// The dialog aimed at a folder `out` of the bench.
fn aimed() -> (Bench, std::path::PathBuf) {
    let mut b = opened();
    let out = b.out("out");
    send(&mut b, FabOutputsMsg::FolderPicked(Some(out.clone())));
    (b, out)
}

fn written_files(b: &Bench) -> Vec<String> {
    match &b.fab().outcome {
        Some(Ok(written)) => written.files.clone(),
        other => panic!("written, not {other:?}"),
    }
}

#[test]
fn the_dialog_opens_with_the_decided_kinds_ticked_and_writes_nothing() {
    let b = opened();
    let kinds = b.fab().kinds;
    assert_eq!(kinds, FabOutputKinds::default());
    for kind in [
        FabOutput::Gerber,
        FabOutput::Drill,
        FabOutput::Centroid,
        FabOutput::TestNetlist,
        FabOutput::Ipc2581,
        FabOutput::Odb,
        FabOutput::AssemblyPdf,
    ] {
        assert!(kind.is_on(&kinds), "{kind:?} starts on");
    }
    assert!(!FabOutput::AssemblySvg.is_on(&kinds));
    assert!(!FabOutput::Step.is_on(&kinds));
    assert_eq!(b.fab().name, "audit", "named after the board");
    assert!(
        !std::path::Path::new(&b.fab().folder).exists(),
        "nothing is written before Generate"
    );
}

#[test]
fn every_tick_turns_its_kind_on_and_off() {
    let mut b = opened();
    for kind in FabOutput::ALL {
        let was = kind.is_on(&b.fab().kinds);
        send(&mut b, FabOutputsMsg::OutputToggled(kind));
        assert_eq!(kind.is_on(&b.fab().kinds), !was, "{kind:?}");
        send(&mut b, FabOutputsMsg::OutputToggled(kind));
        assert_eq!(kind.is_on(&b.fab().kinds), was, "{kind:?}");
    }
    assert!(!b.dirty(), "a tick is no board edit");
}

#[test]
fn the_odb_folder_box_is_deaf_until_odb_is_ticked_and_keeps_its_value() {
    let mut b = opened();
    send(&mut b, FabOutputsMsg::OdbFolderToggled);
    assert!(b.fab().kinds.odb_folder, "ODB++ is on: the box works");
    send(&mut b, FabOutputsMsg::OutputToggled(FabOutput::Odb));
    send(&mut b, FabOutputsMsg::OdbFolderToggled);
    assert!(b.fab().kinds.odb_folder, "ODB++ off: deaf, value kept");
}

#[test]
fn a_kind_left_unticked_is_not_written() {
    let (mut b, out) = aimed();
    send(&mut b, FabOutputsMsg::OutputToggled(FabOutput::Gerber));
    send(&mut b, FabOutputsMsg::GenerateRequested);
    assert!(out.join("audit-centroid.csv").is_file());
    assert!(!out.join("audit-copper-l1-top.gbr").exists());
}

#[test]
fn generate_writes_the_files_and_keeps_the_dialog_with_the_outcome() {
    let (mut b, out) = aimed();
    b.leaves_document(|b| send(b, FabOutputsMsg::GenerateRequested));
    let files = written_files(&b);
    assert!(files.contains(&"audit-copper-l1-top.gbr".to_string()));
    for name in &files {
        assert!(out.join(name).is_file(), "{name} is on disk");
    }
    assert!(b.fab().has_run(), "the footer offers Generate again, Close");
    assert!(b.fab().generated_at.is_some(), "Generated HH:MM");
}

#[test]
fn an_edit_after_generate_clears_the_outcome() {
    let (mut b, _out) = aimed();
    send(&mut b, FabOutputsMsg::GenerateRequested);
    assert!(b.fab().has_run());
    send(&mut b, FabOutputsMsg::NameEdited("rev-b".into()));
    assert!(
        !b.fab().has_run(),
        "the outcome no longer describes the set"
    );
    assert_eq!(b.fab().generated_at, None);
}

#[test]
fn an_older_file_in_the_folder_is_named_and_left_in_place() {
    let (mut b, out) = aimed();
    std::fs::create_dir_all(&out).expect("the folder");
    let older = out.join("audit-old.drl");
    std::fs::write(&older, "older").expect("an older file");
    send(&mut b, FabOutputsMsg::GenerateRequested);
    match &b.fab().outcome {
        Some(Ok(written)) => assert_eq!(written.stale, ["audit-old.drl"]),
        other => panic!("written, not {other:?}"),
    }
    assert!(older.is_file(), "it is never deleted");
}

#[test]
fn a_name_a_file_cannot_have_is_refused_with_the_text_kept() {
    let (mut b, out) = aimed();
    send(&mut b, FabOutputsMsg::NameEdited("rev/a".into()));
    send(&mut b, FabOutputsMsg::GenerateRequested);
    assert_eq!(
        b.fab().outcome,
        Some(Err(FabProblem::Refused(FabError::BadName)))
    );
    assert_eq!(b.fab().name, "rev/a", "the text is kept");
    assert!(!out.exists(), "nothing is written");
}

#[test]
fn an_empty_name_or_folder_does_not_generate() {
    let (mut b, out) = aimed();
    send(&mut b, FabOutputsMsg::NameEdited("  ".into()));
    send(&mut b, FabOutputsMsg::GenerateRequested);
    assert!(!b.fab().has_run());
    assert!(!out.exists());
    send(&mut b, FabOutputsMsg::NameEdited("audit".into()));
    send(&mut b, FabOutputsMsg::FolderEdited(String::new()));
    send(&mut b, FabOutputsMsg::GenerateRequested);
    assert!(!b.fab().has_run());
    assert!(!out.exists());
}

#[test]
fn a_cancelled_folder_pick_keeps_the_folder() {
    let (mut b, out) = aimed();
    send(&mut b, FabOutputsMsg::FolderPicked(None));
    assert_eq!(b.fab().folder, out.display().to_string());
}

#[test]
fn browse_takes_the_folder_the_picker_answers() {
    let mut b = opened();
    let chosen = b.out("picked");
    b.answer(&chosen);
    send(&mut b, FabOutputsMsg::BrowseRequested);
    assert_eq!(b.fab().folder, chosen.display().to_string());
}

#[test]
fn cancel_closes_the_dialog_and_writes_nothing() {
    let (mut b, out) = aimed();
    b.leaves_document(|b| send(b, FabOutputsMsg::CancelRequested));
    assert!(b.app.ui_state.fab_outputs.is_none(), "the dialog closed");
    assert!(!out.exists(), "nothing is written");
}

#[test]
fn the_whole_panel_box_starts_on_and_changes_what_is_written() {
    let mut board = audit_board();
    board.options.panel = Some(BoardPanel::default());
    let mut b = open_board(board);
    b.command("fabrication_outputs");
    let out = b.out("panel");
    send(&mut b, FabOutputsMsg::FolderPicked(Some(out.clone())));
    assert_eq!(b.fab().panel, Some((2, 2)));
    assert!(b.fab().whole_panel, "a panelised board writes its panel");
    send(&mut b, FabOutputsMsg::GenerateRequested);
    let copper = out.join("audit-copper-l1-top.gbr");
    let panel = std::fs::read(&copper).expect("the panel's copper");
    send(&mut b, FabOutputsMsg::WholePanelToggled);
    assert!(!b.fab().whole_panel);
    send(&mut b, FabOutputsMsg::GenerateRequested);
    let alone = std::fs::read(&copper).expect("the board's copper");
    assert_ne!(panel, alone, "the board alone is another image");
}

#[test]
fn the_whole_panel_box_does_nothing_without_a_panel() {
    let mut b = opened();
    assert_eq!(b.fab().panel, None);
    send(&mut b, FabOutputsMsg::WholePanelToggled);
    assert!(!b.fab().whole_panel);
}

/// A board its project names, with the dialog open on it.
fn opened_in_a_project() -> Bench {
    let mut b = open_board(audit_board());
    let dir = b.dir.path().to_path_buf();
    let prj = dir.join("Audit.snxprj");
    let board_path = dir.join("Audit.snxpcb");
    let text = SnxPcb::new(audit_board()).write_string().expect("encode");
    std::fs::write(&board_path, text).expect("write board");
    let data = ProjectData {
        document_id: None,
        name: "Audit".to_string(),
        description: None,
        dir: dir.to_string_lossy().into_owned(),
        schematic_root: None,
        pcb_file: Some("Audit.snxpcb".to_string()),
        sheets: Vec::new(),
        variant_definitions: Vec::new(),
        active_variant: None,
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
    write_project(&prj, &data).expect("write project");
    let id = b.app.document_state.mint_project_id();
    b.app.document_state.projects.push(LoadedProject {
        id,
        path: prj.clone(),
        data: parse_project(&prj).expect("read project"),
        pending_libraries: Default::default(),
        case_fold: Default::default(),
        case_probed: false,
        case_decided: false,
    });
    b.app.document_state.active_project = Some(id);
    b.send(Message::File(FileMsg::Opened(Some(board_path.clone()))));
    b.path = board_path;
    b.command("fabrication_outputs");
    b
}

#[test]
fn a_tick_on_a_project_board_is_one_project_step_that_undo_restores() {
    let mut b = opened_in_a_project();
    let prj = b.app.document_state.projects[0].path.clone();
    assert!(!b.app.document_state.is_document_dirty(&prj));
    send(&mut b, FabOutputsMsg::OutputToggled(FabOutput::Step));
    let stored = b.app.document_state.projects[0].data.fab_outputs;
    assert!(
        stored.is_some_and(|kinds| kinds.step),
        "the project holds it"
    );
    assert!(
        b.app.document_state.is_document_dirty(&prj),
        "a project edit"
    );
    assert_eq!(
        parse_project(&prj).expect("read project").fab_outputs,
        None,
        "nothing is written before Save"
    );
    send(&mut b, FabOutputsMsg::CancelRequested);
    let id = b.app.document_state.projects[0].id;
    b.send(Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
        ProjectTreeAction::OpenProjectOptions(vec![id.to_string()]),
    )));
    b.undo();
    assert_eq!(b.app.document_state.projects[0].data.fab_outputs, None);
    assert!(
        !b.app.document_state.is_document_dirty(&prj),
        "undo returns to the save point"
    );
}
