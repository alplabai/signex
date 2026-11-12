//! #2232 (2026-11-12) — the New PCB and New Schematic setup dialogs and the
//! user's saved setup profiles, driven through UI messages: the menu rows,
//! the Projects tree, the dialog's cards, fields and buttons.

use std::path::{Path, PathBuf};

use signex_app::app::{
    ContextMenuMsg, DocumentSetupMsg, LoadedProject, Message, NewProjectMsg, ProjectTreeAction,
    SetupKind, Signex, TabDocument,
};
use signex_app::io_ports::{HeadlessAnswers, IoPorts};
use signex_types::designator::DesignatorStandard;
use signex_types::format::SnxSchematic;
use signex_types::pcb::PcbBoard;
use signex_types::project::{PcbField, ProjectData, ProjectDefaults, SetupProfile, SheetEntry};
use signex_types::schematic::{SchematicSheet, SheetOrientation};

use crate::menu_support::menu_row;

fn send(app: &mut Signex, message: Message) {
    let task = app.update(message);
    crate::drive::drive(app, task);
}

fn setup(app: &mut Signex, msg: NewProjectMsg) {
    send(app, Message::from(DocumentSetupMsg::Setup(msg)));
}

fn create(app: &mut Signex) {
    send(app, Message::from(DocumentSetupMsg::SubmitRequested));
}

/// A saved project `name` in `dir` with one sheet, made the active one.
fn add_project(
    app: &mut Signex,
    dir: &Path,
    name: &str,
    defaults: ProjectDefaults,
) -> signex_app::app::ProjectId {
    let sheet = format!("{name}.snxsch");
    std::fs::write(
        dir.join(&sheet),
        SnxSchematic::new(SchematicSheet::default())
            .write_string()
            .expect("sheet text"),
    )
    .expect("write sheet");
    let data = ProjectData {
        document_id: None,
        description: None,
        name: name.to_string(),
        dir: dir.to_string_lossy().into_owned(),
        schematic_root: Some(sheet.clone()),
        pcb_file: None,
        sheets: vec![SheetEntry {
            name: sheet.clone(),
            filename: sheet,
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
    let path = dir.join(format!("{name}.snxprj"));
    signex_types::project::write_project(&path, &data).expect("write project");
    let id = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id,
        path,
        data,
        pending_libraries: Default::default(),
        case_fold: Default::default(),
        case_probed: false,
        case_decided: false,
    });
    app.document_state.active_project = Some(id);
    id
}

struct Fixture {
    app: Signex,
    dir: PathBuf,
    _tmp: tempfile::TempDir,
}

fn fixture(defaults: ProjectDefaults) -> Fixture {
    let _ = signex_app::diagnostics::init_logging();
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path().to_path_buf();
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.main_window_id = Some(iced::window::Id::unique());
    app.ui_state.setup_profiles.clear();
    add_project(&mut app, &dir, "demo", defaults);
    Fixture {
        app,
        dir,
        _tmp: tmp,
    }
}

fn board_at<'a>(app: &'a Signex, path: &Path) -> Option<&'a PcbBoard> {
    app.document_state
        .tabs
        .iter()
        .find(|t| t.path == path)
        .and_then(|t| t.cached_document.as_ref())
        .and_then(TabDocument::as_pcb_engine)
        .map(|engine| engine.board())
}

fn copper(board: &PcbBoard) -> usize {
    board
        .layers
        .iter()
        .filter(|l| l.layer_type == "copper")
        .count()
}

/// New PCB opens the dialog, prefilled from the project; nothing is made.
#[test]
fn new_pcb_opens_the_setup_dialog_prefilled_from_the_project() {
    let hobby = signex_types::project::ProjectSetup::from_profile(SetupProfile::Hobby).defaults();
    let mut f = fixture(hobby);
    let tabs = f.app.document_state.tabs.len();
    send(&mut f.app, menu_row("new_pcb_document"));
    let state = f.app.ui_state.document_setup.as_ref().expect("the dialog");
    assert_eq!(state.kind, SetupKind::Pcb);
    assert_eq!(f.app.document_state.tabs.len(), tabs, "no board yet");
    assert!(f.app.document_state.projects[0].data.pcb_file.is_none());
    // Create with the prefill: the project's Hobby rules.
    create(&mut f.app);
    assert!(f.app.ui_state.document_setup.is_none());
    let board = board_at(&f.app, &f.dir.join("demo.snxpcb")).expect("the board");
    assert_eq!(copper(board), 2);
    assert_eq!(board.setup.as_ref().map(|s| s.clearance), Some(0.2));
    assert!(!f.dir.join("demo.snxpcb").exists(), "nothing written");
}

/// Create with the HDI card makes a board with the HDI rules; a typed
/// thickness is taken too.
#[test]
fn create_with_hdi_makes_a_board_with_the_hdi_rules() {
    let mut f = fixture(ProjectDefaults::default());
    let key = tree_key(&mut f.app);
    send(
        &mut f.app,
        Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
            ProjectTreeAction::AddNewPcb(key),
        )),
    );
    assert_open(&f.app, SetupKind::Pcb);
    setup(&mut f.app, NewProjectMsg::ProfilePicked(SetupProfile::Hdi));
    setup(
        &mut f.app,
        NewProjectMsg::RuleEdited(PcbField::Thickness, "1.0".into()),
    );
    create(&mut f.app);
    let board = board_at(&f.app, &f.dir.join("demo.snxpcb")).expect("the board");
    let hdi = SetupProfile::Hdi.pcb(8);
    assert_eq!(copper(board), 6);
    assert_eq!(board.thickness, 1.0);
    let rules = board.setup.as_ref().expect("board setup");
    assert_eq!(rules.clearance, hdi.clearance_mm);
    assert_eq!(rules.trace_width, hdi.track_width_mm);
    assert_eq!(rules.via_diameter, hdi.via_diameter_mm);
    assert_eq!(rules.via_drill, hdi.via_drill_mm);
}

/// Cancel and Esc make nothing and leave the project as it was.
#[test]
fn cancel_makes_nothing() {
    let mut f = fixture(ProjectDefaults::default());
    let tabs = f.app.document_state.tabs.len();
    send(&mut f.app, menu_row("new_pcb_document"));
    setup(&mut f.app, NewProjectMsg::ProfilePicked(SetupProfile::Hdi));
    send(&mut f.app, Message::from(DocumentSetupMsg::CancelRequested));
    assert!(f.app.ui_state.document_setup.is_none());
    assert_eq!(f.app.document_state.tabs.len(), tabs);
    assert!(f.app.document_state.projects[0].data.pcb_file.is_none());
    let project = f.app.document_state.projects[0].path.clone();
    assert!(!f.app.document_state.is_document_dirty(&project));
    // A bad value keeps Create off.
    send(&mut f.app, menu_row("new_pcb_document"));
    setup(
        &mut f.app,
        NewProjectMsg::RuleEdited(PcbField::Clearance, "x".into()),
    );
    create(&mut f.app);
    assert!(f.app.ui_state.document_setup.is_some(), "Create was off");
    assert_eq!(f.app.document_state.tabs.len(), tabs);
}

/// New Schematic: the dialog's size, orientation and designator standard
/// reach the sheet written and the project.
#[test]
fn new_schematic_create_applies_the_sheet_size_and_the_designator_standard() {
    let mut f = fixture(ProjectDefaults::default());
    send(&mut f.app, menu_row("new_schematic_sheet"));
    let state = f.app.ui_state.document_setup.as_ref().expect("the dialog");
    assert_eq!(state.kind, SetupKind::Sheet);
    setup(&mut f.app, NewProjectMsg::SheetSizePicked("A4".into()));
    setup(
        &mut f.app,
        NewProjectMsg::SheetOrientationPicked(SheetOrientation::Portrait),
    );
    setup(
        &mut f.app,
        NewProjectMsg::DesignatorsPicked(DesignatorStandard::Iec81346_2),
    );
    assert!(
        !f.dir.join("Power.snxsch").exists(),
        "nothing before Create"
    );
    // The file dialog answers with the new sheet's path.
    let answers = HeadlessAnswers::default();
    f.app.ui_state.io = IoPorts::headless(answers.clone());
    let target = f.dir.join("Power.snxsch");
    answers.replace_all([("path".to_string(), target.display().to_string())]);
    create(&mut f.app);
    assert!(f.app.ui_state.document_setup.is_none());
    let text = std::fs::read_to_string(&target).expect("the sheet is written");
    let sheet = SnxSchematic::parse(&text).expect("a sheet").sheet;
    assert_eq!(sheet.paper_size, "A4 portrait");
    let project = &f.app.document_state.projects[0];
    assert_eq!(
        project.data.designator_standard(),
        Some(DesignatorStandard::Iec81346_2)
    );
    assert!(
        project
            .data
            .sheets
            .iter()
            .any(|s| s.filename == "Power.snxsch"),
        "the project lists the sheet"
    );
}

/// Save as profile in one project; the profile is offered in a second one
/// and makes its board, and it is in the user's settings.
#[test]
fn a_saved_profile_appears_in_a_second_project() {
    let mut f = fixture(ProjectDefaults::default());
    send(&mut f.app, menu_row("new_pcb_document"));
    setup(&mut f.app, NewProjectMsg::LayersStepped(6));
    setup(
        &mut f.app,
        NewProjectMsg::RuleEdited(PcbField::Clearance, "0.11".into()),
    );
    setup(&mut f.app, NewProjectMsg::SaveProfileOpened);
    setup(
        &mut f.app,
        NewProjectMsg::SaveProfileNameEdited("Team 2232".into()),
    );
    setup(&mut f.app, NewProjectMsg::SaveProfileConfirmed);
    send(&mut f.app, Message::from(DocumentSetupMsg::CancelRequested));
    let saved = f
        .app
        .ui_state
        .setup_profiles
        .iter()
        .find(|p| p.name == "Team 2232")
        .cloned()
        .expect("the profile is saved");
    assert_eq!(saved.pcb.copper_layers, 10);
    assert_eq!(saved.pcb.clearance_mm, 0.11);
    let prefs =
        signex_app::fonts::read_setup_profiles_from(signex_app::fonts::load_prefs_json().as_ref());
    assert!(
        prefs.iter().any(|p| p.id == saved.id),
        "written to prefs.json"
    );

    // A second project: its dialog offers the saved card.
    let other = tempfile::tempdir().expect("tempdir");
    let second = add_project(
        &mut f.app,
        other.path(),
        "second",
        ProjectDefaults::default(),
    );
    send(
        &mut f.app,
        Message::from(DocumentSetupMsg::OpenRequested {
            kind: SetupKind::Pcb,
            project: Some(second),
        }),
    );
    let state = f.app.ui_state.document_setup.as_ref().expect("the dialog");
    assert_eq!(state.project, second);
    setup(
        &mut f.app,
        NewProjectMsg::UserProfilePicked(saved.id.clone()),
    );
    create(&mut f.app);
    let board = board_at(&f.app, &other.path().join("second.snxpcb")).expect("the board");
    assert_eq!(copper(board), 10);
    assert_eq!(board.setup.as_ref().map(|s| s.clearance), Some(0.11));

    // Remove asks first; Keep keeps it, the confirm's Remove takes it off.
    send(&mut f.app, menu_row("new_schematic_sheet"));
    setup(
        &mut f.app,
        NewProjectMsg::UserProfileRemoveRequested(saved.id.clone()),
    );
    setup(&mut f.app, NewProjectMsg::UserProfileRemoveCancelled);
    assert!(
        f.app
            .ui_state
            .setup_profiles
            .iter()
            .any(|p| p.id == saved.id)
    );
    setup(
        &mut f.app,
        NewProjectMsg::UserProfileRemoveRequested(saved.id.clone()),
    );
    assert!(
        f.app
            .ui_state
            .setup_profiles
            .iter()
            .any(|p| p.id == saved.id),
        "a pressed Remove only asks"
    );
    setup(&mut f.app, NewProjectMsg::UserProfileRemoveConfirmed);
    assert!(
        f.app
            .ui_state
            .setup_profiles
            .iter()
            .all(|p| p.id != saved.id)
    );
}

/// A project naming a saved profile the user does not have opens the
/// dialog on its stored values.
#[test]
fn a_missing_profile_falls_back_to_the_stored_values() {
    let mut stored =
        signex_types::project::ProjectSetup::from_profile(SetupProfile::Hobby).defaults();
    stored.profile = Some("user-gone".into());
    let mut f = fixture(stored);
    send(&mut f.app, menu_row("new_pcb_document"));
    let state = f.app.ui_state.document_setup.as_ref().expect("the dialog");
    assert_eq!(state.kind, SetupKind::Pcb);
    create(&mut f.app);
    let board = board_at(&f.app, &f.dir.join("demo.snxpcb")).expect("the board");
    assert_eq!(copper(board), 2, "the stored Hobby rules");
}

/// Press the open setup dialog's Create, if one is open: the step a New PCB
/// entry point takes since #2232.
pub(crate) fn create_if_asked(app: &mut Signex) {
    if app.ui_state.document_setup.is_some() {
        create(app);
    }
}

/// Esc closes either setup dialog and makes nothing.
#[test]
fn esc_closes_the_setup_dialog_without_creating_anything() {
    let mut f = fixture(ProjectDefaults::default());
    let tabs = f.app.document_state.tabs.len();
    let esc = || Message::Input(signex_app::app::InputMsg::EscapePressed { window: None });
    for row in ["new_pcb_document", "new_schematic_sheet"] {
        send(&mut f.app, menu_row(row));
        assert!(f.app.ui_state.document_setup.is_some(), "{row}: open");
        setup(&mut f.app, NewProjectMsg::ProfilePicked(SetupProfile::Hdi));
        send(&mut f.app, esc());
        assert!(
            f.app.ui_state.document_setup.is_none(),
            "{row}: Esc closed it"
        );
    }
    assert_eq!(f.app.document_state.tabs.len(), tabs, "no board opened");
    let project = &f.app.document_state.projects[0];
    assert!(project.data.pcb_file.is_none());
    assert_eq!(project.data.sheets.len(), 1, "no sheet added");
    assert!(!f.app.document_state.is_document_dirty(&project.path));
    assert!(!f.dir.join("demo.snxpcb").exists());
}

/// A UI message refreshes the panel context, which builds the Projects
/// tree; the project row's key, which must exist.
fn tree_key(app: &mut Signex) -> signex_widgets::tree_view::TreeKey {
    send(
        app,
        Message::Ui(signex_app::app::UiMsg::GridPickerSelect(1.0)),
    );
    signex_widgets::tree_view::key_of(&app.document_state.panel_ctx.project_tree, &[0])
        .expect("the project row")
}

/// The setup dialog is open for `kind` and the fixture's project.
fn assert_open(app: &Signex, kind: SetupKind) {
    let state = app.ui_state.document_setup.as_ref().expect("the dialog");
    assert_eq!(state.kind, kind);
    assert_eq!(Some(state.project), app.document_state.active_project);
}

fn palette(app: &mut Signex, label: &str) {
    use signex_app::app::CommandPaletteMsg as P;
    send(app, Message::CommandPalette(P::Open));
    send(app, Message::CommandPalette(P::QueryChanged(label.into())));
    send(app, Message::CommandPalette(P::ExecuteSelected));
}

/// Every entry point opens its setup dialog for the project before anything
/// is made: File ▸ New rows, the Projects tree, Update PCB's Create PCB and
/// the palette.
#[test]
fn every_entry_point_opens_its_setup_dialog_for_the_project() {
    type Entry = (&'static str, SetupKind, fn(&mut Signex));
    let entries: [Entry; 7] = [
        ("File/New/PCB", SetupKind::Pcb, |app| {
            send(app, menu_row("new_pcb_document"))
        }),
        ("File/New/Schematic", SetupKind::Sheet, |app| {
            send(app, menu_row("new_schematic_sheet"))
        }),
        ("Projects ▸ Add New ▸ PCB", SetupKind::Pcb, |app| {
            let key = tree_key(app);
            send(
                app,
                Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
                    ProjectTreeAction::AddNewPcb(key),
                )),
            )
        }),
        (
            "Projects ▸ Add New ▸ Schematic",
            SetupKind::Sheet,
            |app| {
                let key = tree_key(app);
                send(
                    app,
                    Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
                        ProjectTreeAction::AddNewSchematic(key),
                    )),
                )
            },
        ),
        ("Update PCB ▸ Create PCB", SetupKind::Pcb, |app| {
            use signex_app::app::UpdatePcbMsg;
            let project = app.document_state.projects[0].id;
            send(
                app,
                Message::from(UpdatePcbMsg::RequestedForProject(project)),
            );
            send(app, Message::from(UpdatePcbMsg::CreatePcbRequested));
        }),
        ("palette: New PCB", SetupKind::Pcb, |app| {
            palette(app, "New PCB in the project")
        }),
        ("palette: New schematic", SetupKind::Sheet, |app| {
            palette(app, "New schematic sheet in the project")
        }),
    ];
    for (entry, kind, press) in entries {
        let mut f = fixture(ProjectDefaults::default());
        let tabs = f.app.document_state.tabs.len();
        press(&mut f.app);
        assert!(
            f.app.ui_state.document_setup.is_some(),
            "{entry}: no dialog"
        );
        assert_open(&f.app, kind);
        assert_eq!(
            f.app.document_state.tabs.len(),
            tabs,
            "{entry}: nothing made"
        );
        let project = &f.app.document_state.projects[0];
        assert!(project.data.pcb_file.is_none(), "{entry}");
        assert_eq!(project.data.sheets.len(), 1, "{entry}");
    }
}

/// A sheet write that is refused (the file is there) leaves the project's
/// designator standard as it was and the project clean.
#[test]
fn a_refused_sheet_write_changes_no_designator_standard() {
    let mut f = fixture(ProjectDefaults::default());
    let target = f.dir.join("Power.snxsch");
    std::fs::write(&target, b"keep").expect("an existing file");
    send(&mut f.app, menu_row("new_schematic_sheet"));
    assert_open(&f.app, SetupKind::Sheet);
    setup(
        &mut f.app,
        NewProjectMsg::DesignatorsPicked(DesignatorStandard::Iec81346_2),
    );
    let answers = HeadlessAnswers::default();
    f.app.ui_state.io = IoPorts::headless(answers.clone());
    answers.replace_all([
        ("path".to_string(), target.display().to_string()),
        ("overwrite".to_string(), "true".to_string()),
    ]);
    create(&mut f.app);
    assert_eq!(
        std::fs::read(&target).expect("read"),
        b"keep",
        "not replaced"
    );
    let project = &f.app.document_state.projects[0];
    assert_eq!(
        project.data.designator_standard(),
        None,
        "standard unchanged"
    );
    assert_eq!(project.data.sheets.len(), 1, "no sheet listed");
    assert!(
        !f.app.document_state.is_document_dirty(&project.path),
        "the project is not dirtied"
    );
}

/// Project Options: Remove asks first; removing the project's own default
/// profile clears `project_defaults.profile` in the same edit.
#[test]
fn removing_the_projects_default_profile_clears_its_id() {
    use signex_app::app::{ProjectMsg, ProjectTabMsg};
    let mut users = Vec::new();
    let id = signex_types::project::save_user_profile(
        &mut users,
        "Mine 2232",
        SetupProfile::Hdi.pcb(8),
        Default::default(),
        DesignatorStandard::Ieee315,
    );
    let mut defaults =
        signex_types::project::ProjectSetup::from_profile(SetupProfile::Hdi).defaults();
    defaults.profile = Some(id.clone());
    let mut f = fixture(defaults);
    f.app.ui_state.setup_profiles = users;
    let project_id = f.app.document_state.projects[0].id;
    let edit = |msg| {
        Message::Project(ProjectMsg::Tab(ProjectTabMsg::SetupEdited {
            project_id,
            msg,
        }))
    };
    send(
        &mut f.app,
        edit(NewProjectMsg::UserProfileRemoveRequested(id.clone())),
    );
    assert_eq!(f.app.ui_state.setup_profiles.len(), 1, "only asked");
    send(&mut f.app, edit(NewProjectMsg::UserProfileRemoveConfirmed));
    assert!(f.app.ui_state.setup_profiles.is_empty());
    let project = &f.app.document_state.projects[0];
    assert_eq!(
        project.data.project_defaults.profile, None,
        "no dangling id"
    );
    assert_eq!(
        project.data.project_defaults.pcb,
        Some(SetupProfile::Hdi.pcb(8)),
        "the stored values stay"
    );
    assert!(f.app.document_state.is_document_dirty(&project.path));
}
