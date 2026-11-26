//! #2232 — `new_pcb_document profile=..` and `new_schematic_sheet profile=..`
//! run headless: the document is made with the named profile, no dialog.

use std::path::Path;

use signex_types::designator::DesignatorStandard;
use signex_types::project::{
    ProjectData, ProjectDefaults, ProjectSetup, SetupProfile, SheetDefaults, SheetEntry,
    save_user_profile,
};

use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;

struct Fixture {
    app: Signex,
    dir: std::path::PathBuf,
    _tmp: tempfile::TempDir,
}

fn fixture(defaults: ProjectDefaults) -> Fixture {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path().to_path_buf();
    let (mut app, _boot) = Signex::new();
    app.document_state.setup_profiles.clear();
    let data = ProjectData {
        document_id: None,
        description: None,
        name: "demo".to_string(),
        dir: dir.to_string_lossy().into_owned(),
        schematic_root: Some("demo.snxsch".to_string()),
        pcb_file: None,
        sheets: vec![SheetEntry {
            name: "demo.snxsch".to_string(),
            filename: "demo.snxsch".to_string(),
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
        project_defaults: defaults,
        agent_journal: Default::default(),
        schematic_design: Default::default(),
        fab_outputs: None,
        router_tool_diameter: None,
        bom_alternates: Vec::new(),
        requirements: Default::default(),
        parameters: Default::default(),
        unknown: Default::default(),
    };
    let path = dir.join("demo.snxprj");
    signex_types::project::write_project(&path, &data).expect("write project");
    let id = app.document_state.mint_project_id();
    app.document_state.projects.push(crate::app::LoadedProject {
        id,
        path,
        data,
        pending_libraries: Default::default(),
        case_fold: Default::default(),
        case_probed: false,
        case_decided: false,
    });
    app.document_state.active_project = Some(id);
    Fixture {
        app,
        dir,
        _tmp: tmp,
    }
}

fn run(app: &mut Signex, command: &str, operands: &[&str]) -> CommandOutcome {
    let id = AppCommandId::new(command).expect("a command id");
    let args = if operands.is_empty() {
        CommandArgs::none()
    } else {
        CommandArgs::parse_operands(&id, operands).expect("operands parse")
    };
    let (outcome, task) = app.run_headless_command_in_test(&id, args);
    crate::task_driver::TaskDriver::new()
        .settle(app, task)
        .expect("the task chain settled");
    outcome
}

fn board_copper(app: &Signex, path: &Path) -> Option<(usize, f64)> {
    let board = app.document_state.first_tab_board_engine(path)?.board();
    let copper = board
        .layers
        .iter()
        .filter(|l| l.layer_type == "copper")
        .count();
    Some((copper, board.setup.as_ref()?.clearance))
}

/// #2470: a headless New PCB answers with the board it made.
pub(super) fn assert_answers_board(outcome: &CommandOutcome, path: &Path) {
    let CommandOutcome::Answered(answer) = outcome else {
        panic!("New PCB answers with its board: {outcome:?}");
    };
    assert_eq!(answer["kind"], "pcb", "{answer}");
    assert_eq!(
        answer["path"].as_str().map(Path::new),
        Some(path),
        "{answer}"
    );
    let id = answer["id"].as_str().expect("the board's id");
    assert!(id.parse::<uuid::Uuid>().is_ok(), "{answer}");
}

#[test]
fn headless_new_pcb_with_a_built_in_profile_makes_its_board_without_a_dialog() {
    let mut f = fixture(ProjectDefaults::default());
    let outcome = run(&mut f.app, "new_pcb_document", &["profile=hdi"]);
    assert_answers_board(&outcome, &f.dir.join("demo.snxpcb"));
    assert!(f.app.ui_state.document_setup.is_none(), "no dialog");
    let hdi = SetupProfile::Hdi.pcb(8);
    assert_eq!(
        board_copper(&f.app, &f.dir.join("demo.snxpcb")),
        Some((8, hdi.clearance_mm))
    );
    assert!(!f.dir.join("demo.snxpcb").exists(), "opened unsaved");
}

#[test]
fn headless_new_pcb_with_the_project_profile_takes_the_stored_rules() {
    let advanced = ProjectSetup::from_profile(SetupProfile::Advanced).defaults();
    let mut f = fixture(advanced);
    let outcome = run(&mut f.app, "new_pcb_document", &["profile=project"]);
    assert_answers_board(&outcome, &f.dir.join("demo.snxpcb"));
    assert_eq!(
        board_copper(&f.app, &f.dir.join("demo.snxpcb")),
        Some((6, 0.1))
    );
}

#[test]
fn headless_new_pcb_without_a_profile_takes_the_project_setup_as_before() {
    let advanced = ProjectSetup::from_profile(SetupProfile::Advanced).defaults();
    let mut f = fixture(advanced);
    let outcome = run(&mut f.app, "new_pcb_document", &[]);
    assert_answers_board(&outcome, &f.dir.join("demo.snxpcb"));
    assert!(f.app.ui_state.document_setup.is_none(), "no dialog");
    assert_eq!(
        board_copper(&f.app, &f.dir.join("demo.snxpcb")),
        Some((6, 0.1))
    );
}

#[test]
fn headless_new_pcb_with_an_unknown_profile_is_refused() {
    let mut f = fixture(ProjectDefaults::default());
    let outcome = run(&mut f.app, "new_pcb_document", &["profile=nope"]);
    assert!(
        matches!(&outcome, CommandOutcome::Disabled { reason } if signex_i18n::RenderTranslatable::render_en_us(reason).contains("nope")),
        "{outcome:?}"
    );
    assert!(f.app.document_state.tabs.is_empty());
    assert!(f.app.ui_state.document_setup.is_none());
}

#[test]
fn headless_new_schematic_with_a_saved_profile_writes_its_sheet() {
    let mut f = fixture(ProjectDefaults::default());
    save_user_profile(
        &mut f.app.document_state.setup_profiles,
        "Big sheets",
        SetupProfile::Standard.pcb(8),
        SheetDefaults {
            paper_size: "A2".into(),
            show_title_block: false,
        },
        DesignatorStandard::Din40719_2,
    );
    let outcome = run(
        &mut f.app,
        "new_schematic_sheet",
        &["profile=Big sheets", "name=Power"],
    );
    assert!(matches!(outcome, CommandOutcome::Ran), "{outcome:?}");
    let text = std::fs::read_to_string(f.dir.join("Power.snxsch")).expect("the sheet");
    let sheet = signex_types::format::SnxSchematic::parse(&text)
        .expect("a sheet")
        .sheet;
    assert_eq!(sheet.paper_size, "A2");
    assert!(!sheet.frame.show_title_block);
    let project = &f.app.document_state.projects[0];
    assert_eq!(
        project.data.designator_standard(),
        Some(DesignatorStandard::Din40719_2)
    );
    assert!(
        project
            .data
            .sheets
            .iter()
            .any(|s| s.filename == "Power.snxsch")
    );
    // No operands: the project's setup and the first free `Sheet<n>`.
    let outcome = run(&mut f.app, "new_schematic_sheet", &[]);
    assert!(matches!(outcome, CommandOutcome::Ran), "{outcome:?}");
    assert!(f.dir.join("Sheet1.snxsch").exists());
}

#[test]
fn the_gui_command_opens_the_dialog_instead() {
    let mut f = fixture(ProjectDefaults::default());
    let _ = f
        .app
        .update(Message::from(super::DocumentSetupMsg::OpenRequested {
            kind: super::SetupKind::Sheet,
            project: None,
        }));
    let state = f.app.ui_state.document_setup.as_ref().expect("the dialog");
    assert_eq!(state.kind, super::SetupKind::Sheet);
    assert_eq!(state.project_name, "demo");
}

#[test]
fn the_default_sheet_name_skips_a_file_already_on_disk() {
    let mut f = fixture(ProjectDefaults::default());
    std::fs::write(f.dir.join("Sheet1.snxsch"), b"keep").expect("a stray file");
    let outcome = run(&mut f.app, "new_schematic_sheet", &[]);
    assert!(matches!(outcome, CommandOutcome::Ran), "{outcome:?}");
    assert_eq!(
        std::fs::read(f.dir.join("Sheet1.snxsch")).expect("read"),
        b"keep"
    );
    assert!(f.dir.join("Sheet2.snxsch").exists());
}

#[test]
fn a_missing_profile_is_said_once_per_session() {
    let mut stored = ProjectSetup::from_profile(SetupProfile::Advanced).defaults();
    stored.profile = Some("user-gone-2232".into());
    let mut f = fixture(stored);
    let needle = "names a setup profile that was removed or is not among your saved profiles";
    let before = crate::diagnostics::logged_count(needle);
    for _ in 0..2 {
        let _ = f
            .app
            .update(Message::from(super::DocumentSetupMsg::OpenRequested {
                kind: super::SetupKind::Pcb,
                project: None,
            }));
        let _ = f
            .app
            .update(Message::from(super::DocumentSetupMsg::CancelRequested));
    }
    assert_eq!(crate::diagnostics::logged_count(needle), before + 1);
}

#[test]
fn the_dialog_closes_when_its_project_closes() {
    let mut f = fixture(ProjectDefaults::default());
    let _ = f
        .app
        .update(Message::from(super::DocumentSetupMsg::OpenRequested {
            kind: super::SetupKind::Sheet,
            project: None,
        }));
    assert!(f.app.ui_state.document_setup.is_some());
    f.app.document_state.projects.clear();
    f.app.document_state.active_project = None;
    let _ = f.app.update(Message::noop());
    assert!(f.app.ui_state.document_setup.is_none());
}

/// #2470: `document` names the project the board joins and `name` its file;
/// nothing is written, the board and the project stay unsaved.
#[test]
fn headless_new_pcb_names_its_project_and_its_board_file() {
    let mut f = fixture(ProjectDefaults::default());
    f.app.document_state.active_project = None;
    let project = f.dir.join("demo.snxprj");
    let document = format!("document={}", project.display());
    let outcome = run(&mut f.app, "new_pcb_document", &[&document, "name=Main"]);
    let board = f.dir.join("Main.snxpcb");
    assert_answers_board(&outcome, &board);
    assert!(!board.exists(), "opened unsaved");
    let loaded = &f.app.document_state.projects[0];
    assert_eq!(loaded.data.pcb_file.as_deref(), Some("Main.snxpcb"));
    assert!(f.app.document_state.is_document_dirty(&project));
}

#[test]
fn headless_new_pcb_keeps_the_board_the_project_names() {
    let mut f = fixture(ProjectDefaults::default());
    f.app.document_state.projects[0].data.pcb_file = Some("demo.snxpcb".into());
    let outcome = run(&mut f.app, "new_pcb_document", &["name=Other"]);
    assert!(
        matches!(&outcome, CommandOutcome::Disabled { reason } if signex_i18n::RenderTranslatable::render_en_us(reason).contains("already names the board")),
        "{outcome:?}"
    );
    assert!(f.app.document_state.tabs.is_empty());
    // The name it names is the board it makes.
    let outcome = run(&mut f.app, "new_pcb_document", &["name=demo.snxpcb"]);
    assert_answers_board(&outcome, &f.dir.join("demo.snxpcb"));
}

#[test]
fn headless_new_pcb_refuses_a_name_that_is_not_a_file_name() {
    let mut f = fixture(ProjectDefaults::default());
    let outcome = run(&mut f.app, "new_pcb_document", &["name=../away"]);
    assert!(
        matches!(&outcome, CommandOutcome::Disabled { reason } if signex_i18n::RenderTranslatable::render_en_us(reason).contains("not a usable board file name")),
        "{outcome:?}"
    );
    assert!(f.app.document_state.tabs.is_empty());
}

/// The menu's New PCB (its setup dialog, Create) and the operand form make
/// the same board at the same path.
#[test]
fn the_dialogs_create_and_the_operand_form_make_the_same_board() {
    let advanced = || ProjectSetup::from_profile(SetupProfile::Advanced).defaults();
    let mut gui = fixture(advanced());
    let _ = gui
        .app
        .update(Message::from(super::DocumentSetupMsg::OpenRequested {
            kind: super::SetupKind::Pcb,
            project: None,
        }));
    let task = gui
        .app
        .update(Message::from(super::DocumentSetupMsg::SubmitRequested));
    crate::task_driver::TaskDriver::new()
        .settle(&mut gui.app, task)
        .expect("the task chain settled");
    let mut cli = fixture(advanced());
    let outcome = run(&mut cli.app, "new_pcb_document", &[]);
    assert_answers_board(&outcome, &cli.dir.join("demo.snxpcb"));
    let made = board_copper(&gui.app, &gui.dir.join("demo.snxpcb"));
    assert!(made.is_some(), "the dialog made the board");
    assert_eq!(made, board_copper(&cli.app, &cli.dir.join("demo.snxpcb")));
}
