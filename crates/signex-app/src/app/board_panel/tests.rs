//! #2410 — panelisation through `Signex::update` (Tools rows, the Panel
//! section of Properties, undo and redo, the Fabrication Outputs dialog) and
//! through the headless path an agent and the CLI take.

use std::path::{Path, PathBuf};

use signex_types::pcb::PcbBoard;
use signex_types::pcb_panel::{BoardPanel, PanelField, PanelSeparation};
use uuid::Uuid;

use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::handlers::menu::export::settle;
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{EditMsg, FabOutputsMsg, Message, Signex};
use crate::dock::DockMessage;
use crate::keymap::AppCommandId;
use crate::menu_bar::MenuMessage;
use crate::panels::{BoardField, PanelMsg, PcbBoardMsg};

/// A 50 x 40 mm board with R1 on top.
fn board(outline: bool) -> PcbBoard {
    let outline = if outline {
        serde_json::json!([
            {"x": 0.0, "y": 0.0}, {"x": 50.0, "y": 0.0},
            {"x": 50.0, "y": 40.0}, {"x": 0.0, "y": 40.0}
        ])
    } else {
        serde_json::json!([])
    };
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(1),
        "setup": {"grid_size": 1.27},
        "outline": outline,
        "layers": [{"id": 0, "name": "TopCopper", "layer_type": "signal"}],
        "nets": [{"number": 1, "name": "GND"}],
        "footprints": [{
            "uuid": Uuid::from_u128(0x10), "reference": "R1", "value": "10k",
            "footprint_id": "R_0603", "position": {"x": 10.0, "y": 10.0},
            "layer": "TopCopper",
            "pads": [{"uuid": Uuid::from_u128(0x11), "number": "1", "pad_type": "smd",
                      "shape": "rect", "position": {"x": 0.0, "y": 0.0},
                      "size": {"x": 1.0, "y": 1.0}, "drill": null,
                      "layers": ["TopCopper"], "net": {"number": 1, "name": "GND"}}]
        }]
    }))
    .expect("the fixture board")
}

/// An app with the board open in its tab, in a project that names it.
fn app_with(dir: &Path, outline: bool) -> (Signex, PathBuf) {
    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    app.ui_state.first_run_tour_open = false;
    app.ui_state.unit = signex_types::coord::Unit::Mm;
    app.document_state.projects[0].data.pcb_file = Some("board.snxpcb".into());
    let path = dir.join("board.snxpcb");
    app.open_pcb_tab(path.clone(), "board".into(), board(outline));
    (app, path)
}

fn run(app: &mut Signex, id: &str) {
    let task = app.update(Message::Menu(MenuMessage::command(id)));
    settle(app, task);
}

fn properties(app: &mut Signex, msg: PcbBoardMsg) {
    let task = app.update(Message::Dock(DockMessage::Panel(PanelMsg::PcbBoard(msg))));
    settle(app, task);
}

fn type_in(app: &mut Signex, field: PanelField, text: &str) {
    let field = BoardField::Panel(field);
    properties(app, PcbBoardMsg::FieldTyped(field, text.into()));
    properties(app, PcbBoardMsg::FieldSubmitted(field));
}

fn panel(app: &Signex, path: &Path) -> Option<BoardPanel> {
    app.document_state
        .pcb_board(path)
        .expect("board")
        .options
        .panel
}

fn undo(app: &mut Signex) {
    let _ = app.update(Message::Edit(EditMsg::Undo));
}

fn redo(app: &mut Signex) {
    let _ = app.update(Message::Edit(EditMsg::Redo));
}

fn headless(app: &mut Signex, id: &str, operands: &[&str]) -> CommandOutcome {
    let command = AppCommandId::new(id).unwrap();
    let args = CommandArgs::parse_operands(&command, operands).expect("operands parse");
    app.run_headless_command_in_test(&command, args).0
}

fn answer(outcome: CommandOutcome) -> serde_json::Value {
    match outcome {
        CommandOutcome::Answered(answer) => answer,
        other => panic!("not answered: {other:?}"),
    }
}

#[test]
fn the_tools_rows_panelize_and_remove_as_one_undo_step_each() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path(), true);
    run(&mut app, "panelize_board");
    assert_eq!(panel(&app, &path), Some(BoardPanel::default()));
    let marks = app.interaction_state.pcb_canvas.board_marks();
    assert!(marks.panel.is_some(), "the canvas previews the panel");

    // Panelizing again keeps the panel and adds no step.
    run(&mut app, "panelize_board");
    undo(&mut app);
    assert_eq!(panel(&app, &path), None);
    redo(&mut app);
    assert_eq!(panel(&app, &path), Some(BoardPanel::default()));

    run(&mut app, "remove_board_panel");
    assert_eq!(panel(&app, &path), None);
    assert!(
        app.interaction_state
            .pcb_canvas
            .board_marks()
            .panel
            .is_none()
    );
    undo(&mut app);
    assert_eq!(panel(&app, &path), Some(BoardPanel::default()));
}

#[test]
fn a_board_without_an_outline_is_not_panelized() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path(), false);
    run(&mut app, "panelize_board");
    assert_eq!(panel(&app, &path), None);
    // Remove Panel on a board without one changes nothing either.
    run(&mut app, "remove_board_panel");
    assert_eq!(panel(&app, &path), None);
}

#[test]
fn the_properties_panel_section_edits_each_setting_as_an_undo_step() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path(), true);
    properties(&mut app, PcbBoardMsg::PanelizeClicked);
    assert_eq!(panel(&app, &path), Some(BoardPanel::default()));

    type_in(&mut app, PanelField::Columns, "3");
    type_in(&mut app, PanelField::RailLeft, "4.5");
    let edited = panel(&app, &path).expect("a panel");
    assert_eq!((edited.columns, edited.rails.left), (3, 4.5));

    // Not a whole number, a broken rule and tabs wider than an edge are
    // refused and change nothing.
    type_in(&mut app, PanelField::Rows, "two");
    type_in(&mut app, PanelField::Rows, "0");
    type_in(&mut app, PanelField::TabWidth, "60");
    assert_eq!(panel(&app, &path), Some(edited));
    assert!(app.ui_state.pcb_board_panel.drafts.is_empty());

    properties(
        &mut app,
        PcbBoardMsg::PanelSeparationPicked(PanelSeparation::VScore),
    );
    assert_eq!(
        panel(&app, &path).map(|p| p.separation),
        Some(PanelSeparation::VScore)
    );
    undo(&mut app);
    assert_eq!(panel(&app, &path), Some(edited));
    undo(&mut app);
    assert_eq!(panel(&app, &path).map(|p| p.rails.left), Some(0.0));

    properties(&mut app, PcbBoardMsg::PanelRemoveClicked);
    assert_eq!(panel(&app, &path), None);
}

#[test]
fn an_agent_panelizes_edits_reads_and_removes_the_panel_as_json() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path(), true);
    let document = format!("document={}", path.display());

    let read = answer(headless(&mut app, "board_panel", &[&document]));
    assert_eq!(read["panel"], serde_json::Value::Null);

    let made = answer(headless(&mut app, "panelize_board", &[&document]));
    assert_eq!(made["changed"], true);
    assert_eq!(made["panel"]["columns"], 2);
    assert_eq!(made["panel"]["separation"], "mouse_bites");
    assert_eq!(made["layout"]["copies"], 4);
    assert_eq!(made["layout"]["width_mm"], 102.0);

    let set = answer(headless(
        &mut app,
        "set_board_panel",
        &[&document, "field=separation", "value=v_score"],
    ));
    assert_eq!(set["panel"]["separation"], "v_score");
    assert_eq!(set["layout"]["tabs"], 0);
    assert_eq!(
        panel(&app, &path).map(|p| p.separation),
        Some(PanelSeparation::VScore)
    );

    for operands in [
        ["field=columns", "value=0"],
        ["field=gap", "value=1"],
        ["field=rows", "value=many"],
    ] {
        let outcome = headless(
            &mut app,
            "set_board_panel",
            &[&document, operands[0], operands[1]],
        );
        assert!(
            matches!(outcome, CommandOutcome::Disabled { .. }),
            "{operands:?}: {outcome:?}"
        );
    }

    let removed = answer(headless(&mut app, "remove_board_panel", &[&document]));
    assert_eq!(removed["panel"], serde_json::Value::Null);
    let again = headless(&mut app, "remove_board_panel", &[&document]);
    assert!(
        matches!(again, CommandOutcome::Disabled { .. }),
        "{again:?}"
    );
}

#[test]
fn the_fabrication_outputs_write_the_whole_panel_by_default() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path(), true);
    run(&mut app, "panelize_board");
    run(&mut app, "fabrication_outputs");
    let state = app.ui_state.fab_outputs.as_ref().expect("the dialog");
    assert_eq!(state.panel, Some((2, 2)));
    assert!(state.whole_panel);

    let out = dir.path().join("fab");
    let task = app.update(Message::from(FabOutputsMsg::FolderEdited(
        out.display().to_string(),
    )));
    settle(&mut app, task);
    let task = app.update(Message::from(FabOutputsMsg::GenerateRequested));
    settle(&mut app, task);
    let centroid = std::fs::read_to_string(out.join("board-centroid.csv")).expect("centroid");
    assert_eq!(centroid.matches("R1,").count(), 4, "{centroid}");
    let npth = std::fs::read_to_string(out.join("board-npth.drl")).expect("panel holes");
    assert!(npth.contains("C2.000"), "{npth}");

    // Unticked, the board alone.
    let task = app.update(Message::from(FabOutputsMsg::WholePanelToggled));
    settle(&mut app, task);
    let task = app.update(Message::from(FabOutputsMsg::GenerateRequested));
    settle(&mut app, task);
    let centroid = std::fs::read_to_string(out.join("board-centroid.csv")).expect("centroid");
    assert_eq!(centroid.matches("R1,").count(), 1, "{centroid}");
    assert_eq!(
        panel(&app, &path),
        Some(BoardPanel::default()),
        "the board keeps its panel"
    );
}
