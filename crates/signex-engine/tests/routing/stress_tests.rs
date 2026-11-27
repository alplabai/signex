//! Tests of the stress harness itself (#2967): the new actions run, the
//! connectivity and short checks tell a defect from an edit's own effect,
//! and the report names every measure the issue asks for.

use std::collections::BTreeSet;
use std::time::Duration;

use pcb_geom::{PointNm, RatLine};
use serde_json::json;
use signex_engine::pcb::route::RouteDraft;
use signex_types::pcb::{PcbBoard, Segment};
use uuid::Uuid;

use crate::fixtures;
use crate::invariants::{joined_net, route_net_not_worse, route_shorts, untouched_links_kept};
use crate::session::{ShoveTry, rules_of};
use crate::soak::{self, Action, Checks, Failure, Tally};
use crate::stats::rounds_histogram;
use crate::stress_report::{FORMAT, SoakRun, VERSION, board_json, document, table};

/// Every action kind lands on the synthetic board with every invariant
/// held, and the run records shoves and latencies.
#[test]
fn every_action_kind_runs_on_the_synthetic_board() {
    let checks = Checks {
        drc_every: 0,
        round_trip_every: 0,
    };
    let outcome = soak::run_measured(&fixtures::synthetic(), 7, 300, checks);
    if let Err(failure) = &outcome.result {
        panic!("{failure}");
    }
    let t = &outcome.tally;
    assert!(t.landed > 0 && !t.shoves.is_empty(), "{t:?}");
    assert!(
        t.drags > 0 && t.vias_placed > 0 && t.vias_moved > 0,
        "{t:?}"
    );
    assert!(t.undos > 0 && t.redos > 0 && t.link_checks > 0, "{t:?}");
    let timed: BTreeSet<Action> = outcome.latencies.iter().map(|(a, _)| *a).collect();
    for action in [
        Action::Route,
        Action::DragTrack,
        Action::PlaceVia,
        Action::Undo,
    ] {
        assert!(timed.contains(&action), "{action:?} was not timed");
    }
}

fn line(net: u32, from: i64, to: i64) -> RatLine {
    RatLine {
        net,
        from: PointNm::new(from, 0),
        to: PointNm::new(to, 0),
    }
}

/// A net the action did not touch must keep its exact lines; a touched or
/// poured net may change.
#[test]
fn untouched_nets_keep_their_links() {
    let before = [line(1, 0, 10), line(2, 0, 10), line(3, 0, 10)];
    let none = BTreeSet::new();
    assert_eq!(untouched_links_kept(&before, &before, &none, &none), Ok(()));
    let after = [line(2, 0, 10), line(3, 0, 10)];
    assert_eq!(
        untouched_links_kept(&before, &after, &BTreeSet::from([1]), &none),
        Ok(())
    );
    assert!(untouched_links_kept(&before, &after, &none, &none).is_err());
    assert_eq!(
        untouched_links_kept(&before, &after, &none, &BTreeSet::from([1])),
        Ok(())
    );
    let moved = [line(1, 0, 10), line(2, 0, 12), line(3, 0, 10)];
    let err = untouched_links_kept(&before, &moved, &BTreeSet::from([1]), &none);
    assert!(err.is_err_and(|e| e.contains("net 2")));
}

fn track(id: u128, from: (f64, f64), to: (f64, f64), net: u32) -> Segment {
    serde_json::from_value(json!({
        "uuid": Uuid::from_u128(id),
        "start": {"x": from.0, "y": from.1}, "end": {"x": to.0, "y": to.1},
        "width": 0.25, "layer": "TopCopper", "net": net
    }))
    .expect("a track")
}

/// A route's net never ends with more open links; the net a draft joins is
/// its own unless it pushed a track of that net or the net is poured.
#[test]
fn a_route_never_leaves_its_net_more_open() {
    let before = [line(1, 0, 10), line(1, 10, 20)];
    assert_eq!(route_net_not_worse(&before, &before[..1], 1), Ok(()));
    assert_eq!(route_net_not_worse(&before, &before, 1), Ok(()));
    let worse = [line(1, 0, 10), line(1, 10, 20), line(1, 20, 30)];
    assert!(route_net_not_worse(&before, &worse, 1).is_err());

    let draft = RouteDraft {
        tracks: vec![track(1, (0.0, 0.0), (1.0, 0.0), 4)],
        ..RouteDraft::default()
    };
    let none = BTreeSet::new();
    assert_eq!(joined_net(&draft, &none), Some(4));
    assert_eq!(joined_net(&draft, &BTreeSet::from([4])), None);
    let pushed_own = RouteDraft {
        shoved: vec![track(2, (0.0, 1.0), (1.0, 1.0), 4)],
        ..draft.clone()
    };
    assert_eq!(joined_net(&pushed_own, &none), None);
}

fn board_of(segments: Vec<Segment>) -> PcbBoard {
    serde_json::from_value(json!({
        "uuid": Uuid::from_u128(1),
        "setup": {"grid_size": 0.5, "trace_width": 0.25, "clearance": 0.2},
        "nets": [{"number": 1, "name": "A"}, {"number": 2, "name": "B"}],
        "segments": segments
    }))
    .expect("a board")
}

/// A route's new track across another net's track is a short the design
/// rule check confirms; one clear of it is not.
#[test]
fn a_route_short_is_confirmed_by_the_design_rule_check() {
    let other = track(0x10, (0.0, 0.0), (4.0, 0.0), 2);
    let across = track(0x20, (2.0, -2.0), (2.0, 2.0), 1);
    let board = board_of(vec![other.clone(), across.clone()]);
    let draft = RouteDraft {
        tracks: vec![across],
        ..RouteDraft::default()
    };
    let shorted = route_shorts(&board, &draft, &rules_of(&board, &[]));
    assert!(shorted.is_err_and(|e| e.contains("shorts another net")));

    let clear = track(0x20, (8.0, -2.0), (8.0, 2.0), 1);
    let board = board_of(vec![other, clear.clone()]);
    let draft = RouteDraft {
        tracks: vec![clear],
        ..RouteDraft::default()
    };
    assert_eq!(route_shorts(&board, &draft, &rules_of(&board, &[])), Ok(()));
}

fn shove(rounds: usize, settled: bool) -> ShoveTry {
    ShoveTry {
        rounds,
        cleared: settled,
        settled,
    }
}

/// The histogram counts converged shoves by their rounds, then, last, the
/// ones past the limit or never settled.
#[test]
fn the_rounds_histogram_counts_unconverged_last() {
    let shoves = [
        shove(0, true),
        shove(3, true),
        shove(9, true),
        shove(2, false),
    ];
    assert_eq!(
        rounds_histogram(&shoves),
        vec![1, 0, 0, 1, 0, 0, 0, 0, 0, 2]
    );
    assert_eq!(rounds_histogram(&[]), vec![0; 10]);
}

fn failed_run() -> SoakRun {
    SoakRun {
        name: "b".into(),
        tally: Tally {
            landed: 2,
            shoves: vec![
                shove(0, true),
                shove(3, true),
                shove(9, true),
                shove(2, false),
            ],
            ..Tally::default()
        },
        latencies: vec![
            (Action::Route, Duration::from_millis(4)),
            (Action::Undo, Duration::from_millis(1)),
            (Action::Route, Duration::from_millis(20)),
        ],
        failure: Some(Failure {
            board: "b".into(),
            seed: 0x2967,
            index: 4,
            action: Action::DragTrack,
            message: "m".into(),
        }),
        took: Duration::from_secs(3),
    }
}

/// The JSON entry names the failure with its replay line, the shove
/// histogram and the latency percentiles; the document says what it is;
/// the table has a row per board.
#[test]
fn the_stress_report_names_every_measure() {
    let run = failed_run();
    let entry = board_json(&run, 10);
    assert_eq!(entry["result"], "failed");
    assert_eq!(entry["actions"], 10);
    assert_eq!(entry["actions_run"], 5);
    assert_eq!(entry["counts"]["landed"], 2);
    assert_eq!(
        entry["shove"]["rounds_histogram"],
        json!([1, 0, 0, 1, 0, 0, 0, 0, 0, 2])
    );
    assert_eq!(entry["shove"]["rounds_limit"], 8);
    assert_eq!(entry["shove"]["converged_pct"], 50.0);
    assert_eq!(entry["latency_ms"]["samples"], 3);
    assert_eq!(entry["latency_ms"]["p50"], 4.0);
    assert_eq!(entry["latency_ms"]["p95"], 20.0);
    assert_eq!(entry["latency_ms"]["p99"], 20.0);
    assert_eq!(entry["latency_ms_by_action"]["Route"]["samples"], 2);
    assert_eq!(entry["failure"]["index"], 4);
    assert_eq!(entry["failure"]["action"], "DragTrack");
    let replay = entry["failure"]["replay"].as_str().expect("a replay line");
    assert!(
        replay.starts_with("SIGNEX_SOAK_BOARD=b SIGNEX_SOAK_SEED=0x2967 SIGNEX_SOAK_ACTIONS=5 "),
        "{replay}"
    );

    let doc = document(0x2967, 10, &[run]);
    assert_eq!(doc["format"], FORMAT);
    assert_eq!(doc["version"], VERSION);
    assert_eq!(doc["seed"], "0x2967");
    assert_eq!(doc["failed"], 1);
    assert_eq!(doc["boards"].as_array().map(Vec::len), Some(1));

    let rows = table(&[failed_run()], 10);
    assert_eq!(rows.len(), 3);
    assert!(
        rows[2].starts_with(
            "| b | FAILED | 5 | 2 | 4 | 50.0 | 4.00 | 20.00 | 20.00 | 4 (DragTrack) |"
        ),
        "{}",
        rows[2]
    );
}

/// The failure line keeps the form the scheduled job greps (`replay:`) and
/// replays through the command line.
#[test]
fn a_failure_prints_the_line_that_replays_it() {
    let failure = failed_run().failure.expect("a failure");
    let text = failure.to_string();
    assert!(text.starts_with("routing soak on b: seed 0x2967, action 4 (DragTrack): m\n"));
    assert!(text.ends_with(&format!("  replay: {}", failure.replay())));
}
