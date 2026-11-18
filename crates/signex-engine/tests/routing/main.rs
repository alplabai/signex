//! The router's quality bar, measured (#2482, #2483): a seeded soak of
//! random routing and editing actions with board invariants checked after
//! each, and per-board shove convergence, latency and DRC-delta numbers.
//! Short runs go with every test run; the long soak and the latency
//! thresholds are `#[ignore]`d for the scheduled job, run in release:
//!
//! `cargo test --release -p signex-engine --test routing -- --ignored --nocapture`
//!
//! Environment: `SIGNEX_SOAK_SEED` (default 0x2482), `SIGNEX_SOAK_ACTIONS`
//! (default 10 000), `SIGNEX_SOAK_BOARD` (one board by name, searched in
//! the whole routing corpus even for a short run). The short runs take a
//! fixed sample of six routing corpus boards (`fixtures::SHORT_SAMPLE`)
//! with a few actions each, the scheduled ones every board. A failure
//! prints the line that replays it. How it fits together:
//! `docs/PCB_TOOLS.md`, "Router soak and bench".

mod bench;
mod fixtures;
mod rng;
mod session;
mod soak;

use bench::{BoardBench, MEDIAN_LIMIT, P95_LIMIT, ROUNDS_LIMIT, ROUNDS_SHARE};
use fixtures::{Corpus, Fixture};

const DEFAULT_SEED: u64 = 0x2482;

/// Actions per board: one count for the fixed boards, one for the routing
/// corpus boards, whose size makes each action dearer.
#[derive(Debug, Clone, Copy)]
struct Actions {
    fixed: usize,
    corpus: usize,
}

impl Actions {
    fn same(n: usize) -> Self {
        Self {
            fixed: n,
            corpus: n,
        }
    }

    fn for_board(self, fixture: &Fixture) -> usize {
        if fixture.from_corpus {
            self.corpus
        } else {
            self.fixed
        }
    }
}

/// The per-PR soak: 150 actions on the fixed boards, 8 on each sampled
/// corpus board. A count, not a clock, so the run is the same every time.
const SOAK_SHORT: Actions = Actions {
    fixed: 150,
    corpus: 8,
};

/// The per-PR bench: 30 actions on the fixed boards, 4 on each sampled
/// corpus board.
const BENCH_SHORT: Actions = Actions {
    fixed: 30,
    corpus: 4,
};

fn env_number(name: &str) -> Option<u64> {
    let text = std::env::var(name).ok()?;
    let parsed = match text.strip_prefix("0x") {
        Some(hex) => u64::from_str_radix(hex, 16),
        None => text.parse(),
    };
    Some(parsed.unwrap_or_else(|_| panic!("{name}={text} is not a number")))
}

/// The boards `SIGNEX_SOAK_BOARD` picks: one by name from every board,
/// else the boards `corpus` picks.
fn boards(corpus: Corpus) -> Vec<fixtures::Fixture> {
    let only = std::env::var("SIGNEX_SOAK_BOARD").ok();
    let corpus = if only.is_some() { Corpus::All } else { corpus };
    let boards: Vec<_> = fixtures::boards(corpus)
        .into_iter()
        .filter(|f| only.as_ref().is_none_or(|name| &f.name == name))
        .collect();
    assert!(!boards.is_empty(), "no board named {only:?}");
    boards
}

fn soak_all(seed: u64, actions: Actions, corpus: Corpus) {
    for fixture in boards(corpus) {
        let count = actions.for_board(&fixture);
        match soak::run(&fixture, seed, count) {
            Ok((tally, _)) => println!("soak {:<24} {count} actions: {tally:?}", fixture.name),
            Err(failure) => panic!("{failure}"),
        }
    }
}

/// Per-PR: a short soak on the fixed boards and the short corpus sample.
#[test]
fn soak_short() {
    soak_all(DEFAULT_SEED, SOAK_SHORT, Corpus::Short);
}

/// Scheduled: 10 000 actions on every board, or a replay of a failure.
#[test]
#[ignore = "long: the scheduled routing soak runs it in release"]
fn soak_long() {
    let seed = env_number("SIGNEX_SOAK_SEED").unwrap_or(DEFAULT_SEED);
    let actions = env_number("SIGNEX_SOAK_ACTIONS").unwrap_or(10_000);
    soak_all(seed, Actions::same(actions as usize), Corpus::All);
}

/// A seed replays exactly: the same actions give the same board.
#[test]
fn a_soak_seed_replays_exactly() {
    let fixture = fixtures::synthetic();
    let run = || {
        let (tally, board) = soak::run(&fixture, 7, 60).unwrap_or_else(|f| panic!("{f}"));
        (tally, serde_json::to_string(&board).expect("serializes"))
    };
    let first = run();
    assert!(
        first.0.landed > 0,
        "the run routed something: {:?}",
        first.0
    );
    assert_eq!(run(), first);
}

fn bench_all(seed: u64, actions: Actions, corpus: Corpus) -> Vec<BoardBench> {
    let benches: Vec<BoardBench> = boards(corpus)
        .iter()
        .map(|fixture| bench::bench(fixture, seed, actions.for_board(fixture)))
        .collect();
    for b in &benches {
        println!("bench {}", b.line());
    }
    benches
}

/// Every route's DRC delta is its own copper, and shoves converge.
fn assert_delta_and_convergence(benches: &[BoardBench]) {
    for b in benches {
        assert!(b.breaks.is_empty(), "{}: {:#?}", b.name, b.breaks);
        assert!(
            b.converged_share() >= ROUNDS_SHARE,
            "{}: {:.1}% of shoves converged within {ROUNDS_LIMIT} rounds",
            b.name,
            b.converged_share() * 100.0
        );
    }
}

/// A shove that ran out of rounds never counts as converged, whatever its
/// rounds; one that cleared the way or found the push impossible counts
/// while its rounds stay within the limit.
#[test]
fn a_shove_that_never_settled_does_not_converge() {
    use session::ShoveTry;

    let try_of = |rounds, cleared, settled| ShoveTry {
        rounds,
        cleared,
        settled,
    };
    let unsettled = BoardBench {
        shoves: vec![try_of(5, false, false), try_of(0, false, false)],
        ..BoardBench::default()
    };
    assert_eq!(unsettled.converged_share(), 0.0);
    let mixed = BoardBench {
        shoves: vec![
            try_of(0, true, true),
            try_of(ROUNDS_LIMIT, true, true),
            try_of(1, false, true),
            try_of(ROUNDS_LIMIT + 1, true, true),
            try_of(5, false, false),
        ],
        ..BoardBench::default()
    };
    assert_eq!(mixed.converged_share(), 0.6);
    assert_eq!(BoardBench::default().converged_share(), 1.0);
}

/// A track over a same-net track counts whether it repeats it exactly or
/// shares only part of its line; meeting end to end, running beside it or
/// another net's track does not.
#[test]
fn an_overlap_is_any_shared_stretch_of_one_net_and_layer() {
    use signex_types::pcb::Segment;

    let track = |id: u128, from: (f64, f64), to: (f64, f64), net: u32| -> Segment {
        serde_json::from_value(serde_json::json!({
            "uuid": uuid::Uuid::from_u128(id),
            "start": {"x": from.0, "y": from.1}, "end": {"x": to.0, "y": to.1},
            "width": 0.25, "layer": "TopCopper", "net": net
        }))
        .expect("a track")
    };
    let laid = track(1, (0.0, 0.0), (4.0, 0.0), 1);
    let cases = [
        (track(2, (4.0, 0.0), (0.0, 0.0), 1), true, "the same ends"),
        (track(2, (2.0, 0.0), (9.0, 0.0), 1), true, "part of it"),
        (track(2, (1.0, 0.0), (3.0, 0.0), 1), true, "inside it"),
        (track(2, (4.0, 0.0), (9.0, 0.0), 1), false, "end to end"),
        (track(2, (0.0, 0.5), (4.0, 0.5), 1), false, "beside it"),
        (track(2, (0.0, 0.0), (4.0, 0.0), 2), false, "another net"),
        (track(1, (0.0, 0.0), (4.0, 0.0), 1), false, "itself"),
    ];
    for (other, expected, case) in cases {
        assert_eq!(soak::overlaps(&laid, &other), expected, "{case}");
    }
}

/// The DRC delta check passes a new finding on the route's own copper and
/// one it had before (its gap may move), and fails one off the route.
#[test]
fn the_drc_delta_check_tells_a_side_effect_from_the_route_own() {
    use signex_engine::pcb::route::RouteDraft;
    use signex_types::check::{CheckReport, Finding};
    use signex_types::pcb::Segment;
    use signex_types::violation::Severity;

    let finding = |items: &[&str], message: &str| Finding {
        rule: "builtin::clearance".into(),
        severity: Severity::Error,
        message: message.into(),
        evidence: serde_json::Value::Null,
        items: items.iter().map(|s| s.to_string()).collect(),
        sheet: "board".into(),
        key: message.into(),
    };
    let report = |findings| CheckReport::new("drc", findings, Vec::new(), Vec::new(), Vec::new());
    let own = uuid::Uuid::from_u128(7);
    let track: Segment = serde_json::from_value(serde_json::json!({
        "uuid": own, "start": {"x": 0.0, "y": 0.0}, "end": {"x": 1.0, "y": 0.0},
        "width": 0.25, "layer": "TopCopper", "net": 1
    }))
    .expect("a track");
    let draft = RouteDraft {
        tracks: vec![track],
        ..RouteDraft::default()
    };
    let before = report(vec![finding(&["a", "b"], "0.1 mm")]);
    let fine = report(vec![
        finding(&["a", "b"], "0.15 mm"),
        finding(&["c", &own.to_string()], "own"),
    ]);
    assert_eq!(bench::drc_delta(&before, &fine, &draft), Ok(()));
    let side_effect = report(vec![finding(&["c", "d"], "pushed")]);
    assert!(bench::drc_delta(&before, &side_effect, &draft).is_err());
}

/// Per-PR: the DRC delta and convergence on a short run per board, the
/// routing corpus sampled as in `soak_short`.
#[test]
fn bench_short() {
    let benches = bench_all(DEFAULT_SEED, BENCH_SHORT, Corpus::Short);
    assert!(
        benches.iter().any(|b| b.landed > 0 && !b.shoves.is_empty()),
        "the bench routed and shoved something"
    );
    assert_delta_and_convergence(&benches);
}

/// Scheduled, in release: every threshold of #2483, latency on the median
/// board by copper count.
#[test]
#[ignore = "latency is only meaningful in release: the scheduled job runs it"]
fn bench_thresholds() {
    let seed = env_number("SIGNEX_SOAK_SEED").unwrap_or(DEFAULT_SEED);
    let mut benches = bench_all(seed, Actions::same(300), Corpus::All);
    assert_delta_and_convergence(&benches);
    benches.sort_by_key(|b| b.copper);
    let median = &benches[benches.len() / 2];
    let (p50, p95) = (median.percentile(0.5), median.percentile(0.95));
    println!("median board {}: p50 {p50:?}, p95 {p95:?}", median.name);
    assert!(p50 <= MEDIAN_LIMIT, "{}: median {p50:?}", median.name);
    assert!(p95 <= P95_LIMIT, "{}: p95 {p95:?}", median.name);
}
