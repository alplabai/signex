//! The router's quality bar, measured (#2482, #2483): a seeded soak of
//! random routing and editing actions with board invariants checked after
//! each, and per-board shove convergence, latency and DRC-delta numbers.
//! Short runs go with every test run; the long soak and the latency
//! thresholds are `#[ignore]`d for the scheduled job, run in release:
//!
//! `cargo test --release -p signex-engine --test routing -- --ignored --nocapture`
//!
//! Environment: `SIGNEX_SOAK_SEED` (default 0x2482), `SIGNEX_SOAK_ACTIONS`
//! (default 10 000), `SIGNEX_SOAK_BOARD` (one board by name). A failure
//! prints the line that replays it. How it fits together:
//! `docs/PCB_TOOLS.md`, "Router soak and bench".

mod bench;
mod fixtures;
mod rng;
mod session;
mod soak;

use bench::{BoardBench, MEDIAN_LIMIT, P95_LIMIT, ROUNDS_LIMIT, ROUNDS_SHARE};

const DEFAULT_SEED: u64 = 0x2482;

fn env_number(name: &str) -> Option<u64> {
    let text = std::env::var(name).ok()?;
    let parsed = match text.strip_prefix("0x") {
        Some(hex) => u64::from_str_radix(hex, 16),
        None => text.parse(),
    };
    Some(parsed.unwrap_or_else(|_| panic!("{name}={text} is not a number")))
}

/// The boards `SIGNEX_SOAK_BOARD` picks: one by name, else all.
fn boards() -> Vec<fixtures::Fixture> {
    let only = std::env::var("SIGNEX_SOAK_BOARD").ok();
    let boards: Vec<_> = fixtures::all()
        .into_iter()
        .filter(|f| only.as_ref().is_none_or(|name| &f.name == name))
        .collect();
    assert!(!boards.is_empty(), "no board named {only:?}");
    boards
}

fn soak_all(seed: u64, actions: usize) {
    for fixture in boards() {
        match soak::run(&fixture, seed, actions) {
            Ok((tally, _)) => println!("soak {:<24} {actions} actions: {tally:?}", fixture.name),
            Err(failure) => panic!("{failure}"),
        }
    }
}

/// Per-PR: a short soak on every board.
#[test]
fn soak_short() {
    soak_all(DEFAULT_SEED, 150);
}

/// Scheduled: 10 000 actions per board, or a replay of a failure.
#[test]
#[ignore = "long: the scheduled routing soak runs it in release"]
fn soak_long() {
    let seed = env_number("SIGNEX_SOAK_SEED").unwrap_or(DEFAULT_SEED);
    let actions = env_number("SIGNEX_SOAK_ACTIONS").unwrap_or(10_000);
    soak_all(seed, actions as usize);
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

fn bench_all(seed: u64, actions: usize) -> Vec<BoardBench> {
    let benches: Vec<BoardBench> = boards()
        .iter()
        .map(|fixture| bench::bench(fixture, seed, actions))
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
            "{}: {:.1}% of shoves within {ROUNDS_LIMIT} rounds",
            b.name,
            b.converged_share() * 100.0
        );
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

/// Per-PR: the DRC delta and convergence on a short run per board.
#[test]
fn bench_short() {
    let benches = bench_all(DEFAULT_SEED, 30);
    assert!(
        benches.iter().any(|b| b.landed > 0 && !b.rounds.is_empty()),
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
    let mut benches = bench_all(seed, 300);
    assert_delta_and_convergence(&benches);
    benches.sort_by_key(|b| b.copper);
    let median = &benches[benches.len() / 2];
    let (p50, p95) = (median.percentile(0.5), median.percentile(0.95));
    println!("median board {}: p50 {p50:?}, p95 {p95:?}", median.name);
    assert!(p50 <= MEDIAN_LIMIT, "{}: median {p50:?}", median.name);
    assert!(p95 <= P95_LIMIT, "{}: p95 {p95:?}", median.name);
}
