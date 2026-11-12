//! #2232 — `new_pcb_document profile=..` and `new_schematic_sheet profile=..`
//! run headless: the document is made with the named profile, no dialog.

use std::path::Path;

use signex_types::designator::DesignatorStandard;
use signex_types::project::{
    ProjectData, ProjectDefaults, ProjectSetup, SetupProfile, SheetDefaults, SheetEntry,
    save_user_profile,
};

use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::{Message, Signex, TabDocument};
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
    app.ui_state.setup_profiles.clear();
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
        libraries: Vec::new(),
        enable_git: false,
        net_classes: Vec::new(),
        rules: None,
        annotation: None,
        schematic_defaults: Default::default(),
        project_defaults: defaults,
        agent_journal: Default::default(),
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
    let board = app
        .document_state
        .tabs
        .iter()
        .find(|t| t.path == path)
        .and_then(|t| t.cached_document.as_ref())
        .and_then(TabDocument::as_pcb_engine)?
        .board();
    let copper = board
        .layers
        .iter()
        .filter(|l| l.layer_type == "copper")
        .count();
    Some((copper, board.setup.as_ref()?.clearance))
}

#[test]
fn headless_new_pcb_with_a_built_in_profile_makes_its_board_without_a_dialog() {
    let mut f = fixture(ProjectDefaults::default());
    let outcome = run(&mut f.app, "new_pcb_document", &["profile=hdi"]);
    assert!(matches!(outcome, CommandOutcome::Ran), "{outcome:?}");
    assert!(f.app.ui_state.document_setup.is_none(), "no dialog");
    let hdi = SetupProfile::Hdi.pcb(8);
    assert_eq!(
        board_copper(&f.app, &f.dir.join("demo.snxpcb")),
        Some((6, hdi.clearance_mm))
    );
    assert!(!f.dir.join("demo.snxpcb").exists(), "opened unsaved");
}

#[test]
fn headless_new_pcb_with_the_project_profile_takes_the_stored_rules() {
    let hobby = ProjectSetup::from_profile(SetupProfile::Hobby).defaults();
    let mut f = fixture(hobby);
    let outcome = run(&mut f.app, "new_pcb_document", &["profile=project"]);
    assert!(matches!(outcome, CommandOutcome::Ran), "{outcome:?}");
    assert_eq!(
        board_copper(&f.app, &f.dir.join("demo.snxpcb")),
        Some((2, 0.2))
    );
}

#[test]
fn headless_new_pcb_without_a_profile_takes_the_project_setup_as_before() {
    let hobby = ProjectSetup::from_profile(SetupProfile::Hobby).defaults();
    let mut f = fixture(hobby);
    let outcome = run(&mut f.app, "new_pcb_document", &[]);
    assert!(matches!(outcome, CommandOutcome::Ran), "{outcome:?}");
    assert!(f.app.ui_state.document_setup.is_none(), "no dialog");
    assert_eq!(
        board_copper(&f.app, &f.dir.join("demo.snxpcb")),
        Some((2, 0.2))
    );
}

#[test]
fn headless_new_pcb_with_an_unknown_profile_is_refused() {
    let mut f = fixture(ProjectDefaults::default());
    let outcome = run(&mut f.app, "new_pcb_document", &["profile=nope"]);
    assert!(
        matches!(&outcome, CommandOutcome::Disabled { reason } if reason.contains("nope")),
        "{outcome:?}"
    );
    assert!(f.app.document_state.tabs.is_empty());
    assert!(f.app.ui_state.document_setup.is_none());
}

#[test]
fn headless_new_schematic_with_a_saved_profile_writes_its_sheet() {
    let mut f = fixture(ProjectDefaults::default());
    save_user_profile(
        &mut f.app.ui_state.setup_profiles,
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
