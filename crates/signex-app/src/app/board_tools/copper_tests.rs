//! #2408 — Via Stitching, Teardrops and Gloss through the app: once from
//! the menu row through `Signex::update`, once from operands as an agent or
//! the CLI runs them. Each is one undo step and adds no short or clearance
//! finding to the design rule check.

use std::path::{Path, PathBuf};

use pcb_geom::{ItemRef, PickKind};
use serde_json::json;
use signex_drc::{DrcRules, ViolationKind};
use signex_types::pcb::PcbBoard;
use uuid::Uuid;

use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{EditMsg, Message, Signex};
use crate::keymap::AppCommandId;
use crate::menu_bar::MenuMessage;

pub(super) fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn track(n: u128, a: (f64, f64), b: (f64, f64), net: u32) -> serde_json::Value {
    json!({"uuid": id(n), "start": {"x": a.0, "y": a.1}, "end": {"x": b.0, "y": b.1},
           "width": 0.25, "layer": "TopCopper", "net": net})
}

fn part(n: u128, x: f64, y: f64, net: u32) -> serde_json::Value {
    json!({"uuid": id(n), "reference": format!("U{n}"), "position": {"x": x, "y": y},
           "layer": "TopCopper",
           "pads": [{"uuid": id(n + 1), "number": "1", "pad_type": "smd", "shape": "circle",
                     "position": {"x": 0.0, "y": 0.0}, "size": {"x": 1.2, "y": 1.2},
                     "drill": null, "layers": ["TopCopper"],
                     "net": {"number": net, "name": "SIG"}}]})
}

/// A GND (net 1) Fill over (-5,-5)..(5,5) crossed at y = 0 by a SIG (net 2)
/// track; a GND via at (20, 0) with a track out of it; a bent SIG route
/// (40,0)-(45,0)-(45,5); four net-less lines closing a loop at x 100..110;
/// two SIG parts at (60, 0) and (70, 0).
pub(super) fn board() -> PcbBoard {
    let rect = |x0: f64, y0: f64, x1: f64, y1: f64| json!([{"x": x0, "y": y0}, {"x": x1, "y": y0}, {"x": x1, "y": y1}, {"x": x0, "y": y1}]);
    serde_json::from_value(json!({
        "uuid": id(1000),
        "setup": {"grid_size": 0.5, "clearance": 0.2, "via_diameter": 0.6, "via_drill": 0.3},
        "nets": [{"number": 1, "name": "GND"}, {"number": 2, "name": "SIG"}],
        "zones": [{"uuid": id(10), "net": 1, "net_name": "GND", "layer": "TopCopper",
                   "outline": rect(-5.0, -5.0, 5.0, 5.0), "fill_type": "solid"}],
        "vias": [{"uuid": id(20), "position": {"x": 20.0, "y": 0.0}, "diameter": 1.0,
                  "drill": 0.5, "layers": ["TopCopper", "BottomCopper"], "net": 1}],
        "segments": [track(11, (-10.0, 0.0), (10.0, 0.0), 2),
                     track(21, (20.0, 0.0), (25.0, 0.0), 1),
                     track(30, (40.0, 0.0), (45.0, 0.0), 2),
                     track(31, (45.0, 0.0), (45.0, 5.0), 2),
                     track(80, (100.0, 0.0), (110.0, 0.0), 0),
                     track(81, (110.0, 0.0), (110.0, 8.0), 0),
                     track(82, (110.0, 8.0), (100.0, 8.0), 0),
                     track(83, (100.0, 8.0), (100.0, 0.0), 0)],
        "footprints": [part(50, 60.0, 0.0, 2), part(60, 70.0, 0.0, 2)]
    }))
    .expect("a board")
}

pub(super) fn app_with_board(dir: &Path) -> (Signex, PathBuf) {
    app_with(dir, board())
}

/// An app with `board` open in a tab.
pub(super) fn app_with(dir: &Path, board: PcbBoard) -> (Signex, PathBuf) {
    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    let path = dir.join("tools.snxpcb");
    app.open_pcb_tab(path.clone(), "tools".into(), board);
    (app, path)
}

pub(super) fn live(app: &Signex, path: &Path) -> String {
    serde_json::to_string(app.document_state.pcb_board(path).expect("board")).unwrap()
}

pub(super) fn board_of<'a>(app: &'a Signex, path: &Path) -> &'a PcbBoard {
    app.document_state.pcb_board(path).expect("board")
}

/// The command's menu row, as a click runs it.
pub(super) fn run(app: &mut Signex, command: &str) {
    let id = AppCommandId::new(command).expect("a catalog id");
    let _ = app.update(Message::Menu(MenuMessage::Command(id)));
}

pub(super) fn undo(app: &mut Signex) {
    let _ = app.update(Message::Edit(EditMsg::Undo));
}

/// The command from `operands` (with `document` naming the board), as an
/// agent or the CLI runs it.
pub(super) fn run_operands(
    app: &mut Signex,
    path: &Path,
    command: &str,
    operands: &[&str],
) -> CommandOutcome {
    let id = AppCommandId::new(command).expect("a catalog id");
    let document = format!("document={}", path.display());
    let mut texts = vec![document.as_str()];
    texts.extend_from_slice(operands);
    let args = CommandArgs::parse_operands(&id, &texts).expect("operands parse");
    app.run_operands(&id, &args).0
}

/// Shorts and clearance findings of the board as the design rule check
/// reads it.
pub(super) fn drc_findings(app: &Signex, path: &Path) -> usize {
    let board = board_of(app, path);
    let rules = DrcRules::for_board(board, &[]).expect("rules");
    let report = signex_drc::run(board, &rules);
    report.count(ViolationKind::Short) + report.count(ViolationKind::Clearance)
}

fn select_zone(app: &mut Signex, path: &Path, zone: Uuid) {
    let board = board_of(app, path);
    let found = board.zones.iter().find(|z| z.uuid == zone).expect("zone");
    let kind = PickKind::of_zone(found).expect("a pickable area");
    app.select_pcb_items(&[ItemRef::Zone { zone, kind }]);
}

fn answered_uuids(outcome: &CommandOutcome, key: &str) -> usize {
    match outcome {
        CommandOutcome::Answered(value) => value
            .get(key)
            .and_then(|list| list.as_array())
            .map_or(0, Vec::len),
        other => panic!("not answered: {other:?}"),
    }
}

#[test]
fn via_stitching_fills_the_selected_area_in_one_step_and_keeps_drc_clean() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let before = live(&app, &path);
    assert_eq!(drc_findings(&app, &path), 0);
    select_zone(&mut app, &path, id(10));
    run(&mut app, "stitch_vias");
    let board = board_of(&app, &path);
    let new: Vec<_> = board.vias.iter().filter(|v| v.uuid != id(20)).collect();
    // A 2 mm grid over the 10 mm area, less the row on the SIG track.
    assert_eq!(new.len(), 20, "{new:?}");
    assert!(new.iter().all(|v| v.net == 1 && v.diameter == 0.6));
    assert!(new.iter().all(|v| v.position.y != 0.0));
    assert_eq!(drc_findings(&app, &path), 0, "no new short or clearance");
    undo(&mut app);
    assert_eq!(live(&app, &path), before, "one undo step");
}

#[test]
fn via_stitching_from_operands_answers_with_the_vias_it_added() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let before = live(&app, &path);
    let outcome = run_operands(
        &mut app,
        &path,
        "stitch_vias",
        &["start=-4,-4", "end=4,4", "net=GND", "pitch=2"],
    );
    let added = answered_uuids(&outcome, "added");
    assert!(added > 0);
    assert_eq!(board_of(&app, &path).vias.len(), 1 + added);
    assert_eq!(drc_findings(&app, &path), 0);
    undo(&mut app);
    assert_eq!(live(&app, &path), before);

    let refused = run_operands(&mut app, &path, "stitch_vias", &["start=-4,-4", "end=4,4"]);
    assert!(
        matches!(&refused, CommandOutcome::Disabled { reason } if reason.contains("no net")),
        "{refused:?}"
    );
}

#[test]
fn teardrops_are_added_once_removed_and_undone_in_steps() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let before = live(&app, &path);
    let teardrops = |app: &Signex| {
        board_of(app, &path)
            .zones
            .iter()
            .filter(|z| z.is_teardrop())
            .count()
    };
    run(&mut app, "add_teardrops");
    assert_eq!(teardrops(&app), 1, "the GND track out of its via");
    let drop = board_of(&app, &path)
        .zones
        .iter()
        .find(|z| z.is_teardrop())
        .unwrap();
    assert_eq!((drop.net, drop.layer.as_str()), (1, "TopCopper"));
    assert_eq!(drc_findings(&app, &path), 0);
    run(&mut app, "add_teardrops");
    assert_eq!(teardrops(&app), 1, "adding again replaces them");
    run(&mut app, "remove_teardrops");
    assert_eq!(teardrops(&app), 0);
    undo(&mut app);
    assert_eq!(teardrops(&app), 1);
    undo(&mut app);
    undo(&mut app);
    assert_eq!(live(&app, &path), before, "each run was one step");

    let outcome = run_operands(&mut app, &path, "add_teardrops", &["net=SIG"]);
    assert!(
        matches!(&outcome, CommandOutcome::Disabled { reason } if reason.contains("no track")),
        "{outcome:?}"
    );
    let outcome = run_operands(&mut app, &path, "add_teardrops", &["net=GND"]);
    assert_eq!(answered_uuids(&outcome, "added"), 1);
}

#[test]
fn gloss_cuts_the_selected_routes_corner_in_one_step() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let before = live(&app, &path);
    let gloss = AppCommandId::new("gloss_selected_routes").unwrap();
    let (outcome, _) = app.dispatch_command_reporting(&gloss, CommandArgs::none(), None);
    assert!(
        matches!(outcome, CommandOutcome::Disabled { .. }),
        "greyed with nothing selected"
    );
    app.select_pcb_items(&[ItemRef::Track(id(30)), ItemRef::Track(id(31))]);
    run(&mut app, "gloss_selected_routes");
    let board = board_of(&app, &path);
    assert!(!board.segments.iter().any(|s| s.uuid == id(31)));
    let kept = board.segments.iter().find(|s| s.uuid == id(30)).unwrap();
    assert_eq!((kept.end.x, kept.end.y), (45.0, 5.0));
    assert_eq!(drc_findings(&app, &path), 0);
    undo(&mut app);
    assert_eq!(live(&app, &path), before);

    let ids = format!("ids={},{}", id(30), id(31));
    let outcome = run_operands(&mut app, &path, "gloss_selected_routes", &[&ids]);
    assert_eq!(answered_uuids(&outcome, "removed"), 1);
    assert_eq!(answered_uuids(&outcome, "changed"), 1);
}

#[test]
fn stitching_keeps_vias_out_of_another_nets_fill_on_another_layer() {
    let dir = tempfile::tempdir().unwrap();
    let mut board = board();
    let fill: signex_types::pcb::Zone = serde_json::from_value(json!({
        "uuid": id(12), "net": 2, "net_name": "SIG", "layer": "BottomCopper",
        "outline": [{"x": 0.0, "y": 0.0}, {"x": 5.0, "y": 0.0},
                    {"x": 5.0, "y": 5.0}, {"x": 0.0, "y": 5.0}],
        "fill_type": "solid"}))
    .expect("a zone");
    board.zones.push(fill);
    let (mut app, path) = app_with(dir.path(), board);
    select_zone(&mut app, &path, id(10));
    run(&mut app, "stitch_vias");
    let new: Vec<_> = board_of(&app, &path)
        .vias
        .iter()
        .filter(|v| v.uuid != id(20))
        .map(|v| v.position)
        .collect();
    assert!(!new.is_empty() && new.len() < 20, "{new:?}");
    for p in &new {
        assert!(!(p.x >= -0.6 && p.y >= -0.6), "{p:?} through the SIG fill");
    }
}

#[test]
fn stitching_with_only_tracks_selected_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let before = live(&app, &path);
    app.select_pcb_items(&[ItemRef::Track(id(11))]);
    run(&mut app, "stitch_vias");
    assert_eq!(live(&app, &path), before, "no pour stitched");
}

#[test]
fn copper_tools_refuse_a_clearance_rule_they_cannot_read() {
    let dir = tempfile::tempdir().unwrap();
    let mut board = board();
    board.setup.as_mut().expect("a setup").clearance = 1e30;
    let (mut app, path) = app_with(dir.path(), board);
    let before = live(&app, &path);
    select_zone(&mut app, &path, id(10));
    run(&mut app, "stitch_vias");
    app.clear_pcb_selection();
    run(&mut app, "add_teardrops");
    assert_eq!(live(&app, &path), before, "no copper on a guessed rule");

    let outcome = run_operands(&mut app, &path, "add_teardrops", &["net=GND"]);
    assert!(
        matches!(&outcome, CommandOutcome::Disabled { reason } if reason.contains("clearance rule")),
        "{outcome:?}"
    );
}

#[test]
fn gloss_keeps_a_branch_that_ends_on_the_route() {
    let dir = tempfile::tempdir().unwrap();
    let mut board = board();
    let branch = serde_json::from_value(track(32, (42.0, 0.0), (42.0, -4.0), 2)).unwrap();
    board.segments.push(branch);
    let (mut app, path) = app_with(dir.path(), board);
    let before = live(&app, &path);
    app.select_pcb_items(&[ItemRef::Track(id(30)), ItemRef::Track(id(31))]);
    run(&mut app, "gloss_selected_routes");
    assert_eq!(live(&app, &path), before, "the cut would strand the branch");
}

#[test]
fn a_teardrop_follows_a_track_that_enters_its_via_off_centre() {
    let dir = tempfile::tempdir().unwrap();
    let mut board = board();
    let off = board
        .segments
        .iter_mut()
        .find(|s| s.uuid == id(21))
        .unwrap();
    off.start.y = 0.2;
    off.end.y = 0.2;
    let (mut app, path) = app_with(dir.path(), board);
    run(&mut app, "add_teardrops");
    let drop = board_of(&app, &path)
        .zones
        .iter()
        .find(|z| z.is_teardrop())
        .expect("a teardrop");
    let near = |y: f64| {
        drop.outline
            .iter()
            .any(|p| (p.y - y).abs() < 1e-6 && p.x > 20.0)
    };
    // The tip sits on the track's two edges, 0.2 mm off the via's axis.
    assert!(near(0.325) && near(0.075), "{:?}", drop.outline);
}

/// #2451: the copper tools keep the written DRC rules, not only the board
/// and net-class clearance. A VCC track 0.3 mm from a stitching row: the
/// board's 0.2 mm lets the row in, a written 0.5 mm VCC rule keeps it out.
#[test]
fn stitching_keeps_a_written_clearance_rule() {
    let dir = tempfile::tempdir().unwrap();
    let mut board = board();
    board.nets[1].name = "VCC".into();
    let vcc = board.segments.iter_mut().find(|s| s.uuid == id(11)).unwrap();
    vcc.start.y = -1.275;
    vcc.end.y = -1.275;
    let (mut app, path) = app_with(dir.path(), board);
    app.ui_state.machine_drc_rules = signex_types::project::DrcRuleSet::default();
    // Edge to edge from the VCC track: via radius 0.3, track half width 0.125.
    let gaps = |app: &Signex| -> Vec<f64> {
        board_of(app, &path)
            .vias
            .iter()
            .filter(|v| v.uuid != id(20))
            .map(|v| (v.position.y + 1.275).abs() - 0.425)
            .collect()
    };
    let checked = |app: &Signex| {
        let board = board_of(app, &path);
        let (rules, _) = app.board_design_rules(&path, board).expect("rules");
        let report = signex_drc::run(board, &rules);
        report.count(ViolationKind::Short) + report.count(ViolationKind::Clearance)
    };
    let before = live(&app, &path);

    // With no written rule the board's 0.2 mm lets a row in 0.3 mm off.
    select_zone(&mut app, &path, id(10));
    run(&mut app, "stitch_vias");
    assert!(gaps(&app).iter().any(|g| *g < 0.5), "{:?}", gaps(&app));
    undo(&mut app);
    assert_eq!(live(&app, &path), before);

    app.ui_state.machine_drc_rules.dsl = Some(
        "rule vcc_wide on clearance when net == \"VCC\" require clearance >= 0.5mm then error"
            .into(),
    );
    select_zone(&mut app, &path, id(10));
    run(&mut app, "stitch_vias");
    let kept = gaps(&app);
    assert!(!kept.is_empty(), "the rest of the area is stitched");
    assert!(kept.iter().all(|g| *g >= 0.5 - 1e-9), "{kept:?}");
    assert_eq!(checked(&app), 0, "the rule's DRC finds nothing the tool wrote");
    undo(&mut app);

    let outcome = run_operands(
        &mut app,
        &path,
        "stitch_vias",
        &["start=-4,-4", "end=4,4", "net=GND", "pitch=2"],
    );
    assert!(answered_uuids(&outcome, "added") > 0);
    assert!(gaps(&app).iter().all(|g| *g >= 0.5 - 1e-9), "{:?}", gaps(&app));
    assert_eq!(checked(&app), 0);
}
