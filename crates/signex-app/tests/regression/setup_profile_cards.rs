//! #2538 (R13, decision 2026-11-18 and F06): New PCB and New Schematic in a
//! project that saved a setup show one summary row instead of the profile
//! cards; "Use another profile..." unfolds them and "Keep project setup"
//! folds them back to the project's setup. A project with no saved setup
//! shows the cards and opens with Create on.

use signex_app::app::{DocumentSetupMsg, Message, NewProjectMsg, SetupKind, Signex};
use signex_types::project::{ProjectDefaults, ProjectSetup, SetupProfile};

use crate::document_setup::{add_project, board_at, copper, create, send, setup};

fn app_with(defaults: ProjectDefaults) -> (Signex, tempfile::TempDir) {
    let _ = signex_app::diagnostics::init_logging();
    let tmp = tempfile::tempdir().expect("tempdir");
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.main_window_id = Some(iced::window::Id::unique());
    app.ui_state.setup_profiles.clear();
    add_project(&mut app, tmp.path(), "demo", defaults);
    (app, tmp)
}

fn open(app: &mut Signex, kind: SetupKind) {
    send(
        app,
        Message::from(DocumentSetupMsg::OpenRequested {
            kind,
            project: None,
        }),
    );
}

#[test]
fn new_pcb_in_a_project_with_a_setup_shows_the_summary_and_the_link_unfolds_the_cards() {
    let saved = ProjectSetup::from_profile(SetupProfile::Standard).defaults();
    let (mut app, tmp) = app_with(saved);
    open(&mut app, SetupKind::Pcb);
    let state = app.ui_state.document_setup.as_ref().expect("the dialog");
    assert!(state.shows_project_summary(), "the summary row shows");
    assert!(!state.shows_cards(), "the cards are folded away");
    let _ = app.view(app.ui_state.main_window_id.expect("main window"));

    // Use another profile... unfolds the cards; a pick there is the board's.
    send(
        &mut app,
        Message::from(DocumentSetupMsg::OtherProfileToggled),
    );
    let state = app.ui_state.document_setup.as_ref().expect("the dialog");
    assert!(state.shows_project_summary() && state.shows_cards());
    let _ = app.view(app.ui_state.main_window_id.expect("main window"));
    setup(&mut app, NewProjectMsg::ProfilePicked(SetupProfile::Hdi));

    // Keep project setup folds them and takes the project's setup back.
    send(
        &mut app,
        Message::from(DocumentSetupMsg::OtherProfileToggled),
    );
    let state = app.ui_state.document_setup.as_ref().expect("the dialog");
    assert!(!state.shows_cards());

    // Unfold, pick HDI and Create: the board has the HDI rules.
    send(
        &mut app,
        Message::from(DocumentSetupMsg::OtherProfileToggled),
    );
    setup(&mut app, NewProjectMsg::ProfilePicked(SetupProfile::Hdi));
    create(&mut app);
    let board = board_at(&app, &tmp.path().join("demo.snxpcb")).expect("the board");
    assert_eq!(copper(board), 8);
    assert_eq!(board.setup.as_ref().map(|s| s.clearance), Some(0.075));
}

#[test]
fn keep_project_setup_makes_the_board_with_the_project_values() {
    let saved = ProjectSetup::from_profile(SetupProfile::Advanced).defaults();
    let (mut app, tmp) = app_with(saved);
    open(&mut app, SetupKind::Pcb);
    send(
        &mut app,
        Message::from(DocumentSetupMsg::OtherProfileToggled),
    );
    setup(&mut app, NewProjectMsg::ProfilePicked(SetupProfile::Hdi));
    send(
        &mut app,
        Message::from(DocumentSetupMsg::OtherProfileToggled),
    );
    create(&mut app);
    let board = board_at(&app, &tmp.path().join("demo.snxpcb")).expect("the board");
    assert_eq!(copper(board), 6, "the project's Advanced setup");
    assert_eq!(board.setup.as_ref().map(|s| s.clearance), Some(0.1));
}

#[test]
fn new_pcb_in_a_project_without_a_setup_shows_the_cards_and_create_is_on() {
    let (mut app, tmp) = app_with(ProjectDefaults::default());
    open(&mut app, SetupKind::Pcb);
    let state = app.ui_state.document_setup.as_ref().expect("the dialog");
    assert!(!state.shows_project_summary());
    assert!(state.shows_cards());
    assert!(state.create_enabled(), "Standard values open valid");
    let _ = app.view(app.ui_state.main_window_id.expect("main window"));
    create(&mut app);
    let board = board_at(&app, &tmp.path().join("demo.snxpcb")).expect("the board");
    assert_eq!(copper(board), 4, "Standard on its 4 layers");
}

#[test]
fn new_schematic_in_a_project_with_sheet_defaults_shows_the_summary() {
    let saved = ProjectDefaults {
        sheet: Some(signex_types::project::SheetDefaults {
            paper_size: "A4".into(),
            show_title_block: true,
        }),
        ..ProjectDefaults::default()
    };
    let (mut app, _tmp) = app_with(saved);
    open(&mut app, SetupKind::Sheet);
    let state = app.ui_state.document_setup.as_ref().expect("the dialog");
    assert!(state.shows_project_summary());
    assert!(!state.shows_cards());
    let _ = app.view(app.ui_state.main_window_id.expect("main window"));
    send(
        &mut app,
        Message::from(DocumentSetupMsg::OtherProfileToggled),
    );
    let state = app.ui_state.document_setup.as_ref().expect("the dialog");
    assert!(state.shows_cards(), "Your profiles unfold");
    let _ = app.view(app.ui_state.main_window_id.expect("main window"));
}
