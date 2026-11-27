//! The board rule edits: `set_teardrop_rule`, `set_pad_teardrop` and
//! `set_pour_properties`, from operands as an agent runs them in the window
//! (`run_operands`) and as the CLI runs them headless; Add Teardrops from
//! its menu row follows the rule. Each edit is one board undo step.

use std::path::{Path, PathBuf};

use serde_json::json;
use signex_types::format::{SNXPCB_FORMAT_V1_12, SnxPcb};
use signex_types::pcb::PcbBoard;
use uuid::Uuid;

use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{EditMsg, Message, Signex};
use crate::keymap::AppCommandId;
use crate::menu_bar::MenuMessage;

const VIA: Uuid = Uuid::from_u128(20);
const VIA_TRACK: Uuid = Uuid::from_u128(21);
const PAD: Uuid = Uuid::from_u128(51);
const PAD_TRACK: Uuid = Uuid::from_u128(52);
const POUR: Uuid = Uuid::from_u128(70);
const FILL: Uuid = Uuid::from_u128(71);

/// A GND via at (20, 0) with a track out of it; a SIG pad at (60, 0) with a
/// track out of it; a GND pour and a GND solid fill far from both.
fn board() -> PcbBoard {
    let rect = |x0: f64, x1: f64| json!([{"x": x0, "y": -5.0}, {"x": x1, "y": -5.0}, {"x": x1, "y": 5.0}, {"x": x0, "y": 5.0}]);
    let track = |n: Uuid, a: f64, b: f64, net: u32| {
        json!({"uuid": n, "start": {"x": a, "y": 0.0}, "end": {"x": b, "y": 0.0},
               "width": 0.25, "layer": "TopCopper", "net": net})
    };
    serde_json::from_value(json!({
        "uuid": Uuid::from_u128(1000),
        "setup": {"grid_size": 0.5, "clearance": 0.2, "via_diameter": 0.6, "via_drill": 0.3},
        "nets": [{"number": 1, "name": "GND"}, {"number": 2, "name": "SIG"}],
        "vias": [{"uuid": VIA, "position": {"x": 20.0, "y": 0.0}, "diameter": 1.0,
                  "drill": 0.5, "layers": ["TopCopper", "BottomCopper"], "net": 1}],
        "segments": [track(VIA_TRACK, 20.0, 25.0, 1), track(PAD_TRACK, 60.0, 65.0, 2)],
        "footprints": [{"uuid": Uuid::from_u128(50), "reference": "U1",
                        "position": {"x": 60.0, "y": 0.0}, "layer": "TopCopper",
                        "pads": [{"uuid": PAD, "number": "1", "pad_type": "smd",
                                  "shape": "circle", "position": {"x": 0.0, "y": 0.0},
                                  "size": {"x": 1.2, "y": 1.2}, "drill": null,
                                  "layers": ["TopCopper"],
                                  "net": {"number": 2, "name": "SIG"}}]}],
        "zones": [
            {"uuid": POUR, "net": 1, "net_name": "GND", "layer": "TopCopper",
             "outline": rect(100.0, 110.0), "fill_type": "pour", "priority": 0},
            {"uuid": FILL, "net": 1, "net_name": "GND", "layer": "TopCopper",
             "outline": rect(120.0, 130.0), "fill_type": "solid"}
        ]
    }))
    .expect("a board")
}

fn app_with_board(dir: &Path) -> (Signex, PathBuf) {
    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    let path = dir.join("rules.snxpcb");
    app.open_pcb_tab(path.clone(), "rules".into(), board());
    (app, path)
}

fn board_of<'a>(app: &'a Signex, path: &Path) -> &'a PcbBoard {
    app.document_state.pcb_board(path).expect("board")
}

fn live(app: &Signex, path: &Path) -> String {
    serde_json::to_string(board_of(app, path)).unwrap()
}

fn args(path: &Path, command: &str, operands: &[&str]) -> (AppCommandId, CommandArgs) {
    let id = AppCommandId::new(command).expect("a catalog id");
    let document = format!("document={}", path.display());
    let mut texts = vec![document.as_str()];
    texts.extend_from_slice(operands);
    let args = CommandArgs::parse_operands(&id, &texts).expect("operands parse");
    (id, args)
}

/// The command from operands, as an agent runs it in the window.
fn agent(app: &mut Signex, path: &Path, command: &str, operands: &[&str]) -> CommandOutcome {
    let (id, args) = args(path, command, operands);
    app.run_operands(&id, &args).0
}

/// The command from operands, as `signex-cli run --headless` runs it.
fn headless(app: &mut Signex, path: &Path, command: &str, operands: &[&str]) -> CommandOutcome {
    let (id, args) = args(path, command, operands);
    app.run_headless_command_in_test(&id, args).0
}

fn answer(outcome: CommandOutcome) -> serde_json::Value {
    match outcome {
        CommandOutcome::Answered(value) => value,
        other => panic!("no answer: {other:?}"),
    }
}

fn is_refused(outcome: &CommandOutcome) -> bool {
    matches!(outcome, CommandOutcome::Disabled { .. })
}

/// Add Teardrops' menu row, as a click runs it.
fn add_teardrops_from_the_menu(app: &mut Signex) {
    let id = AppCommandId::new("add_teardrops").expect("a catalog id");
    let _ = app.update(Message::Menu(MenuMessage::Command(id)));
}

fn undo(app: &mut Signex) {
    let _ = app.update(Message::Edit(EditMsg::Undo));
}

/// The tracks the board's teardrops sit on: a teardrop touches its
/// track's start, which lies in the via or the pad.
fn teardrop_tracks(app: &Signex, path: &Path) -> Vec<Uuid> {
    let board = board_of(app, path);
    let near = |x: f64| {
        board
            .zones
            .iter()
            .filter(|z| z.is_teardrop())
            .any(|z| z.outline.iter().any(|p| (p.x - x).abs() < 2.0))
    };
    let mut tracks = Vec::new();
    if near(20.0) {
        tracks.push(VIA_TRACK);
    }
    if near(60.0) {
        tracks.push(PAD_TRACK);
    }
    tracks
}

#[test]
fn the_teardrop_rule_is_one_step_saves_as_1_12_and_add_teardrops_follows_it() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let before = live(&app, &path);
    let answered = answer(agent(
        &mut app,
        &path,
        "set_teardrop_rule",
        &["vias=false", "length_share=2"],
    ));
    assert_eq!(answered["teardrop_rule"]["vias"], json!(false));
    assert_eq!(answered["teardrop_rule"]["pads"], json!(true));
    assert_eq!(answered["teardrop_rule"]["length_share"], json!(2.0));
    let text = SnxPcb::new(board_of(&app, &path).clone())
        .write_string()
        .expect("the board saves");
    assert!(text.starts_with(&format!("format = \"{SNXPCB_FORMAT_V1_12}\"")));

    add_teardrops_from_the_menu(&mut app);
    assert_eq!(teardrop_tracks(&app, &path), [PAD_TRACK], "no via teardrop");
    undo(&mut app);
    undo(&mut app);
    assert_eq!(live(&app, &path), before, "the rule was one step");
    add_teardrops_from_the_menu(&mut app);
    assert_eq!(teardrop_tracks(&app, &path), [VIA_TRACK, PAD_TRACK]);
}

#[test]
fn a_teardrop_rule_that_cannot_shape_a_teardrop_is_refused_and_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let before = live(&app, &path);
    for operands in [
        vec!["width_share=1.5"],
        vec!["length_share=0"],
        vec!["width_share=wide"],
        vec![],
    ] {
        let outcome = agent(&mut app, &path, "set_teardrop_rule", &operands);
        assert!(is_refused(&outcome), "{operands:?}: {outcome:?}");
    }
    assert_eq!(live(&app, &path), before);
}

#[test]
fn a_pad_takes_its_own_teardrop_setting_against_the_rule_and_back() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    answer(agent(&mut app, &path, "set_teardrop_rule", &["pads=false"]));
    let ids = format!("ids={PAD}");
    let answered = answer(agent(
        &mut app,
        &path,
        "set_pad_teardrop",
        &[ids.as_str(), "teardrop=on"],
    ));
    assert_eq!(
        answered["pads"],
        json!([{"id": PAD, "teardrop": "on", "takes_teardrop": true}])
    );
    add_teardrops_from_the_menu(&mut app);
    assert_eq!(teardrop_tracks(&app, &path), [VIA_TRACK, PAD_TRACK]);

    let answered = answer(agent(
        &mut app,
        &path,
        "set_pad_teardrop",
        &[ids.as_str(), "teardrop=rule"],
    ));
    assert_eq!(answered["pads"][0]["takes_teardrop"], json!(false));
    assert!(
        board_of(&app, &path)
            .options
            .teardrops
            .pad_overrides
            .is_empty()
    );

    let before = live(&app, &path);
    let via = format!("ids={VIA}");
    let not_a_pad = agent(
        &mut app,
        &path,
        "set_pad_teardrop",
        &[via.as_str(), "teardrop=on"],
    );
    assert!(is_refused(&not_a_pad), "{not_a_pad:?}");
    let bad_word = agent(
        &mut app,
        &path,
        "set_pad_teardrop",
        &[ids.as_str(), "teardrop=yes"],
    );
    assert!(is_refused(&bad_word), "{bad_word:?}");
    assert_eq!(live(&app, &path), before);
}

#[test]
fn a_pour_takes_its_priority_and_island_rules_in_one_step() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let before = live(&app, &path);
    let ids = format!("ids={POUR}");
    let answered = answer(agent(
        &mut app,
        &path,
        "set_pour_properties",
        &[
            ids.as_str(),
            "priority=3",
            "islands_below=2.5",
            "dead_copper=true",
        ],
    ));
    assert_eq!(
        answered["pours"],
        json!([{"id": POUR, "priority": 3, "islands_below_mm2": 2.5, "dead_copper": true}])
    );
    let pour = &board_of(&app, &path).zones[0];
    assert_eq!(
        (
            pour.priority,
            pour.min_island_area,
            pour.remove_dead_islands
        ),
        (3, 2.5, true)
    );
    undo(&mut app);
    assert_eq!(live(&app, &path), before, "one step");

    for operands in [
        vec![ids.as_str(), "priority=1.5"],
        vec![ids.as_str(), "islands_below=-1"],
        vec![ids.as_str()],
    ] {
        let outcome = agent(&mut app, &path, "set_pour_properties", &operands);
        assert!(is_refused(&outcome), "{operands:?}: {outcome:?}");
    }
    let fill = format!("ids={POUR},{FILL}");
    let not_a_pour = agent(
        &mut app,
        &path,
        "set_pour_properties",
        &[fill.as_str(), "priority=1"],
    );
    assert!(is_refused(&not_a_pour), "{not_a_pour:?}");
    assert_eq!(live(&app, &path), before);
}

#[test]
fn the_cli_runs_the_three_edits_headless_with_the_same_answers() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let rule = answer(headless(
        &mut app,
        &path,
        "set_teardrop_rule",
        &["width_share=0.5"],
    ));
    assert_eq!(rule["teardrop_rule"]["width_share"], json!(0.5));
    let pad = format!("ids={PAD}");
    let pads = answer(headless(
        &mut app,
        &path,
        "set_pad_teardrop",
        &[pad.as_str(), "teardrop=off"],
    ));
    assert_eq!(pads["pads"][0]["teardrop"], json!("off"));
    let pour = format!("ids={POUR}");
    let pours = answer(headless(
        &mut app,
        &path,
        "set_pour_properties",
        &[pour.as_str(), "priority=2"],
    ));
    assert_eq!(pours["pours"][0]["priority"], json!(2));
    let held = board_of(&app, &path);
    assert_eq!(held.options.teardrops.width_share, 0.5);
    assert_eq!(held.options.teardrops.pad_override(PAD), Some(false));
    assert_eq!(held.zones[0].priority, 2);
}
