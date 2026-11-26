//! #2470 — the routing tools and the Via tool from their operands, as a
//! headless run gives them: each lands the same tracks and vias as the
//! tool's clicks on the same board, as one undo step, and answers with what
//! it made; a dry run changes nothing; operands the board cannot take are
//! refused with the reason and change nothing.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use signex_drc::ViolationKind;
use signex_i18n::RenderTranslatable;
use signex_types::pcb::{PcbBoard, Point};
use uuid::Uuid;

use super::tests::{app_with_board, board_of, click, escape, grid_only, live, run, undo};
use crate::app::Signex;
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::keymap::AppCommandId;

/// A footprint `P<n>` holding one round 1 mm top pad at (`x`, `y`).
fn pad(n: u128, x: f64, y: f64, net: (u32, &str)) -> Value {
    json!({
        "uuid": Uuid::from_u128(0x100 + n), "reference": format!("P{n}"), "value": "",
        "footprint_id": "Pad", "layer": "TopCopper", "rotation": 0.0,
        "position": {"x": x, "y": y},
        "pads": [{
            "uuid": Uuid::from_u128(0x200 + n), "number": "1", "pad_type": "smd",
            "shape": "circle", "position": {"x": 0.0, "y": 0.0}, "size": {"x": 1.0, "y": 1.0},
            "drill": null, "layers": ["TopCopper"], "net": {"number": net.0, "name": net.1}
        }]
    })
}

fn track(n: u128, a: (f64, f64), b: (f64, f64), net: u32) -> Value {
    json!({
        "uuid": Uuid::from_u128(n), "start": {"x": a.0, "y": a.1}, "end": {"x": b.0, "y": b.1},
        "width": 0.25, "layer": "TopCopper", "net": net
    })
}

/// Net A's pads P1 at (0, 0) and P2 at (10, 0), net B's `footprints` and
/// `segments`; a 0.5 mm grid, 0.25 mm tracks and a 0.2 mm clearance.
fn around_board(footprints: Vec<Value>, segments: Vec<Value>) -> Value {
    let mut all = vec![pad(1, 0.0, 0.0, (1, "A")), pad(2, 10.0, 0.0, (1, "A"))];
    all.extend(footprints);
    json!({
        "uuid": Uuid::from_u128(1),
        "setup": {"grid_size": 0.5, "trace_width": 0.25, "clearance": 0.2},
        "nets": [{"number": 1, "name": "A"}, {"number": 2, "name": "B"}],
        "footprints": all,
        "segments": segments
    })
}

/// Nets `names` (1, 2) end in stubs at (1, 0.5) and (1, -0.5); net C has a
/// pad on the way at (6, 0).
fn pair_board(names: [&str; 2]) -> Value {
    json!({
        "uuid": Uuid::from_u128(1),
        "setup": {"grid_size": 0.5, "trace_width": 0.25, "clearance": 0.2},
        "nets": [
            {"number": 1, "name": names[0]},
            {"number": 2, "name": names[1]},
            {"number": 3, "name": "C"}
        ],
        "footprints": [pad(3, 6.0, 0.0, (3, "C"))],
        "segments": [
            track(21, (0.0, 0.5), (1.0, 0.5), 1),
            track(22, (0.0, -0.5), (1.0, -0.5), 2)
        ]
    })
}

/// `board` open in its tab, listed as the project's board.
fn open(dir: &Path, board: Value) -> (Signex, PathBuf) {
    let board: PcbBoard = serde_json::from_value(board).unwrap();
    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    list_board(&mut app);
    let path = dir.join("board.snxpcb");
    app.open_pcb_tab(path.clone(), "board".into(), board);
    (app, path)
}

/// The project lists `board.snxpcb` as its board, so the board is named in
/// it and checked with its rules.
fn list_board(app: &mut Signex) {
    app.ui_state.first_run_tour_open = false;
    app.document_state.projects[0].data.pcb_file = Some("board.snxpcb".into());
}

fn headless(app: &mut Signex, id: &str, operands: &[String]) -> CommandOutcome {
    let command = AppCommandId::new(id).unwrap();
    let operands: Vec<&str> = operands.iter().map(String::as_str).collect();
    let args = CommandArgs::parse_operands(&command, &operands).expect("operands parse");
    app.run_headless_command_in_test(&command, args).0
}

fn answered(outcome: CommandOutcome) -> Value {
    match outcome {
        CommandOutcome::Answered(answer) => answer,
        other => panic!("not answered: {other:?}"),
    }
}

fn refusal(outcome: CommandOutcome) -> String {
    match outcome {
        CommandOutcome::Disabled { reason } => reason.render_en_us(),
        other => panic!("not refused: {other:?}"),
    }
}

fn document(path: &Path) -> String {
    format!("document={}", path.display())
}

/// The point `name=x,y` for the board point (`x`, `y`): from the board
/// origin with Y up, as the operand reads it.
fn point(app: &Signex, path: &Path, name: &str, x: f64, y: f64) -> String {
    let shown = board_of(app, path).options.origin.to_user(Point::new(x, y));
    format!("{name}={},{}", shown.x, shown.y)
}

/// The copper of the board, uuids left out, in one order.
fn copper(app: &Signex, path: &Path) -> Vec<String> {
    let board = board_of(app, path);
    let tracks = board.segments.iter().map(|s| {
        let (start, end, width) = (s.start, s.end, s.width);
        format!("track {start:?} {end:?} {width} {} {}", s.layer, s.net)
    });
    let vias = board.vias.iter().map(|v| {
        let (at, size) = (v.position, (v.diameter, v.drill));
        format!("via {at:?} {size:?} {:?} {}", v.layers, v.net)
    });
    let mut all: Vec<String> = tracks.chain(vias).collect();
    all.sort();
    all
}

/// No copper of two nets closer than the rules allow.
fn assert_legal(app: &Signex, path: &Path) {
    let board = board_of(app, path);
    let (rules, _) = app.board_design_rules(path, board).expect("usable rules");
    let report = signex_drc::run(board, &rules);
    let broken = report.count(ViolationKind::Clearance) + report.count(ViolationKind::Short);
    assert_eq!(broken, 0, "{:?}", report.violations);
}

#[test]
fn a_route_from_operands_lands_as_the_clicked_route_in_one_step() {
    let net_b = || vec![pad(3, 5.0, 0.0, (2, "B"))];
    let clicked_dir = tempfile::tempdir().unwrap();
    let (mut clicked, clicked_path) = open(clicked_dir.path(), around_board(net_b(), vec![]));
    run(&mut clicked, "place_board_route");
    click(&mut clicked, 0.0, 0.0);
    click(&mut clicked, 10.0, 0.0);
    escape(&mut clicked);

    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = open(dir.path(), around_board(net_b(), vec![]));
    let before = live(&app, &path);
    let operands = [document(&path), "start=P1.1".into(), "end=P2.1".into()];
    let answer = answered(headless(&mut app, "place_board_route", &operands));

    assert_eq!(copper(&app, &path), copper(&clicked, &clicked_path));
    let board = board_of(&app, &path);
    assert!(board.segments.len() >= 3, "around the pad, not through");
    assert_legal(&app, &path);
    assert_eq!(answer["dry_run"], false);
    assert_eq!(answer["layer"], "TopCopper");
    assert_eq!(answer["nets"], json!(["A"]));
    assert_eq!(answer["width_mm"], 0.25);
    let tracks = answer["tracks"].as_array().expect("tracks");
    assert_eq!(tracks.len(), board.segments.len(), "{answer}");
    for made in tracks {
        let id = Uuid::parse_str(made["id"].as_str().unwrap()).unwrap();
        assert!(board.segments.iter().any(|s| s.uuid == id), "{made}");
        assert_eq!(made["net"], "A");
    }
    assert_eq!(answer["pushed"], json!([]));
    assert!(
        app.interaction_state.pcb_tool.is_none(),
        "no tool is armed for it"
    );

    undo(&mut app);
    assert_eq!(live(&app, &path), before, "one undo removes the route");
}

#[test]
fn push_mode_from_operands_moves_the_other_net_and_lists_what_it_pushed() {
    let net_b = vec![pad(3, 2.0, 5.0, (2, "B")), pad(4, 8.0, 5.0, (2, "B"))];
    let run_b = vec![
        track(11, (2.0, 0.3), (8.0, 0.3), 2),
        track(12, (2.0, 0.3), (2.0, 5.0), 2),
        track(13, (8.0, 5.0), (8.0, 0.3), 2),
    ];
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = open(dir.path(), around_board(net_b, run_b));
    let before = live(&app, &path);
    let operands = [
        document(&path),
        "start=P1.1".into(),
        "end=P2.1".into(),
        "mode=shove".into(),
    ];
    let answer = answered(headless(&mut app, "place_board_route", &operands));

    let pushed = answer["pushed"].as_array().expect("pushed");
    assert!(
        pushed.contains(&json!(Uuid::from_u128(11).to_string())),
        "{answer}"
    );
    assert_legal(&app, &path);
    undo(&mut app);
    assert_eq!(live(&app, &path), before, "route and push are one step");
}

#[test]
fn a_dry_run_answers_with_the_route_and_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let net_b = vec![pad(3, 5.0, 0.0, (2, "B"))];
    let (mut app, path) = open(dir.path(), around_board(net_b, vec![]));
    let before = live(&app, &path);
    let operands = [
        document(&path),
        "start=P1.1".into(),
        "end=P2.1".into(),
        "dry_run=true".into(),
    ];
    let answer = answered(headless(&mut app, "place_board_route", &operands));
    assert_eq!(answer["dry_run"], true);
    assert!(
        answer["tracks"].as_array().is_some_and(|t| t.len() >= 3),
        "{answer}"
    );
    assert_eq!(live(&app, &path), before, "nothing landed");
    undo(&mut app);
    assert_eq!(live(&app, &path), before, "and no undo step was made");
}

/// One case: the command, its clicks and its operand points.
type ClickCase<'a> = (&'a str, &'a [(f64, f64)], &'a [(&'a str, f64, f64)]);

#[test]
fn track_quick_route_and_via_from_operands_match_their_clicks() {
    let cases: [ClickCase; 3] = [
        (
            "place_board_track",
            &[(2.0, 0.0), (5.0, 4.0), (5.0, 4.0)],
            &[("start", 2.0, 0.0), ("end", 5.0, 4.0)],
        ),
        (
            "place_board_quick_route",
            &[(2.0, 0.0), (5.0, 4.0)],
            &[("start", 2.0, 0.0), ("end", 5.0, 4.0)],
        ),
        ("place_board_via", &[(3.0, 3.0)], &[("at", 3.0, 3.0)]),
    ];
    for (command, clicks, points) in cases {
        let clicked_dir = tempfile::tempdir().unwrap();
        let (mut clicked, clicked_path) = app_with_board(clicked_dir.path());
        list_board(&mut clicked);
        grid_only(&mut clicked);
        run(&mut clicked, command);
        for &(x, y) in clicks {
            click(&mut clicked, x, y);
        }

        let dir = tempfile::tempdir().unwrap();
        let (mut app, path) = app_with_board(dir.path());
        list_board(&mut app);
        let before = live(&app, &path);
        let mut operands = vec![document(&path)];
        operands.extend(
            points
                .iter()
                .map(|&(name, x, y)| point(&app, &path, name, x, y)),
        );
        let answer = answered(headless(&mut app, command, &operands));

        assert_eq!(
            copper(&app, &path),
            copper(&clicked, &clicked_path),
            "{command}"
        );
        assert_ne!(live(&app, &path), before, "{command} made something");
        let made = answer["tracks"].as_array().map_or(0, Vec::len)
            + answer["vias"].as_array().map_or(0, Vec::len);
        assert!(made > 0, "{command}: {answer}");
        undo(&mut app);
        assert_eq!(live(&app, &path), before, "{command} is one undo step");
    }
}

#[test]
fn a_via_from_operands_takes_its_net_span_and_size() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = open(dir.path(), around_board(vec![], vec![]));
    let operands = [
        document(&path),
        point(&app, &path, "at", 20.0, 20.0),
        "net=A".into(),
        "layers=BottomCopper,TopCopper".into(),
        "via_diameter=0.9".into(),
        "via_drill=0.45".into(),
    ];
    let answer = answered(headless(&mut app, "place_board_via", &operands));
    let via = &answer["vias"][0];
    assert_eq!(via["net"], "A", "{answer}");
    assert_eq!(via["layers"], json!(["TopCopper", "BottomCopper"]));
    assert_eq!(
        (via["diameter_mm"].as_f64(), via["drill_mm"].as_f64()),
        (Some(0.9), Some(0.45))
    );
    let placed = &board_of(&app, &path).vias;
    assert_eq!(placed.len(), 1);
    assert_eq!(placed[0].position, Point::new(20.0, 20.0));
    assert_eq!(placed[0].net, 1);
}

#[test]
fn a_pair_from_operands_lands_as_the_clicked_pair() {
    let names = ["USB_P", "USB_N"];
    let clicked_dir = tempfile::tempdir().unwrap();
    let (mut clicked, clicked_path) = open(clicked_dir.path(), pair_board(names));
    run(&mut clicked, "place_board_diff_pair");
    click(&mut clicked, 1.0, 0.5);
    click(&mut clicked, 11.0, 0.0);
    escape(&mut clicked);

    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = open(dir.path(), pair_board(names));
    let before = live(&app, &path);
    let operands = [
        document(&path),
        "net=USB_N".into(),
        point(&app, &path, "start", 1.0, 0.5),
        point(&app, &path, "end", 11.0, 0.0),
    ];
    let answer = answered(headless(&mut app, "place_board_diff_pair", &operands));

    assert_eq!(copper(&app, &path), copper(&clicked, &clicked_path));
    assert_eq!(answer["nets"], json!(["USB_P", "USB_N"]), "positive first");
    assert_legal(&app, &path);
    undo(&mut app);
    assert_eq!(live(&app, &path), before, "one undo removes the pair");
}

#[test]
fn multi_route_from_operands_runs_the_named_nets_side_by_side() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = open(dir.path(), pair_board(["D0", "D1"]));
    let before = live(&app, &path);
    let tracks_of = |app: &Signex, net: u32| {
        let board = board_of(app, &path);
        board.segments.iter().filter(|s| s.net == net).count()
    };
    let operands = [
        document(&path),
        "nets=D0,D1".into(),
        point(&app, &path, "start", 1.0, 0.0),
        point(&app, &path, "end", 11.0, 0.0),
        "layer=TopCopper".into(),
    ];
    let answer = answered(headless(&mut app, "place_board_multi_route", &operands));
    assert_eq!(answer["nets"], json!(["D0", "D1"]));
    assert!(tracks_of(&app, 1) >= 2, "D0 routed past its stub");
    assert!(tracks_of(&app, 2) >= 2, "D1 routed past its stub");
    assert_legal(&app, &path);
    undo(&mut app);
    assert_eq!(live(&app, &path), before, "one undo removes the route");
}

#[test]
fn operands_the_board_cannot_take_are_refused_with_the_reason() {
    let dir = tempfile::tempdir().unwrap();
    let net_b = vec![pad(3, 5.0, 0.0, (2, "B"))];
    let (mut app, path) = open(dir.path(), around_board(net_b, vec![]));
    let before = live(&app, &path);
    let route = across;
    let cases: Vec<(&str, Vec<&str>, &str)> = vec![
        (
            "place_board_route",
            vec!["start=Q9.1", "end=P2.1"],
            "Q9.1 names no pad",
        ),
        (
            "place_board_route",
            route(&["layer=TopSilkscreen"]),
            "not a copper layer",
        ),
        (
            "place_board_route",
            route(&["net=NOPE"]),
            "no net named NOPE",
        ),
        (
            "place_board_route",
            route(&["corner=30"]),
            "`corner` is one of",
        ),
        (
            "place_board_route",
            route(&["mode=fly"]),
            "`mode` is walkaround or shove",
        ),
        (
            "place_board_route",
            route(&["through=1;2"]),
            "`through` lists",
        ),
        (
            "place_board_route",
            route(&["width=-1"]),
            "`width` takes a length",
        ),
        (
            "place_board_route",
            vec!["start=20,20", "end=25,20"],
            "name the copper layer",
        ),
        (
            "place_board_track",
            vec!["start=P1.1", "end=P3.1"],
            "is on net B",
        ),
        (
            "place_board_diff_pair",
            vec!["net=A", "start=P1.1", "end=P2.1"],
            "in no differential pair",
        ),
        (
            "place_board_multi_route",
            vec!["nets=A", "start=P1.1", "end=P2.1"],
            "two or more nets",
        ),
        (
            "place_board_via",
            vec!["at=1,1", "layers=TopCopper"],
            "`layers` names two",
        ),
        (
            "place_board_via",
            vec!["at=1,1", "via_diameter=0.3", "via_drill=0.5"],
            "wider than its copper",
        ),
    ];
    for (command, operands, why) in cases {
        let mut all = vec![document(&path)];
        all.extend(operands.iter().map(|s| s.to_string()));
        let reason = refusal(headless(&mut app, command, &all));
        assert!(reason.contains(why), "{command} {operands:?}: {reason}");
        assert_eq!(live(&app, &path), before, "{command} changed nothing");
    }
    // Without operands a headless run is asked for the board, not armed.
    let command = AppCommandId::new("place_board_route").unwrap();
    let bare = app.run_headless_command_in_test(&command, CommandArgs::none());
    let bare = refusal(bare.0);
    assert!(bare.contains("needs the operand `document`"), "{bare}");
}

/// The operands of a one-net route from P1 to P2, and `extra`.
fn across<'a>(extra: &[&'a str]) -> Vec<&'a str> {
    let mut all = vec!["start=P1.1", "end=P2.1"];
    all.extend(extra);
    all
}
