//! The router's quality bar, measured (#2482, #2483): a seeded soak of
//! random routing and editing actions with board invariants checked after
//! each, and per-board shove convergence, latency and DRC-delta numbers;
//! the soak's stress report (#2967) is a summary table and a JSON file.
//! Short runs go with every test run; the long soak and the latency
//! thresholds are `#[ignore]`d for the scheduled job, run in release:
//!
//! `cargo test --release -p signex-engine --test routing -- --ignored --nocapture`
//!
//! The environment variables (seed, length, one board, the check rates,
//! threads, the report file, the latency assert) are listed in
//! `docs/PERFORMANCE.md`, "Router soak and bench". The short runs take the
//! fixed boards only; the scheduled ones add every routing corpus board.
//! Per PR the corpus is checked by loading alone (`corpus_check`). A
//! failure prints the line that replays it. How it fits together:
//! `docs/PCB_TOOLS.md`, "Router soak and bench".

mod bench;
mod corpus_check;
mod fixtures;
mod invariants;
mod report;
mod rng;
mod session;
mod soak;
mod stats;
mod stress_report;
mod stress_tests;
mod workers;

use std::time::Instant;

use bench::{BoardBench, MEDIAN_LIMIT, P95_LIMIT, ROUNDS_LIMIT, ROUNDS_SHARE};
use fixtures::Corpus;
use soak::Checks;
use stress_report::SoakRun;

const DEFAULT_SEED: u64 = 0x2482;

/// The variable `name` as a decimal or `0x` hex number; unset or empty is
/// `None` (the scheduled job passes an empty dispatch input through).
fn env_number(name: &str) -> Option<u64> {
    let text = std::env::var(name).ok().filter(|t| !t.is_empty())?;
    let parsed = match text.strip_prefix("0x") {
        Some(hex) => u64::from_str_radix(hex, 16),
        None => text.parse(),
    };
    Some(parsed.unwrap_or_else(|_| panic!("{name}={text} is not a number")))
}

/// The boards `SIGNEX_SOAK_BOARD` picks: one by name from every board,
/// else the boards `corpus` picks.
fn boards(corpus: Corpus) -> Vec<fixtures::Fixture> {
    let only = std::env::var("SIGNEX_SOAK_BOARD")
        .ok()
        .filter(|name| !name.is_empty());
    let corpus = if only.is_some() { Corpus::All } else { corpus };
    let boards: Vec<_> = fixtures::boards(corpus)
        .into_iter()
        .filter(|f| only.as_ref().is_none_or(|name| &f.name == name))
        .collect();
    assert!(!boards.is_empty(), "no board named {only:?}");
    boards
}

/// Soak every board on `jobs` threads, one report line each and the stress
/// report (#2967), then fail with every board's failure at once.
fn soak_all(seed: u64, actions: usize, corpus: Corpus, checks: Checks, jobs: usize) {
    let fixtures = boards(corpus);
    let runs = workers::map_in_order(&fixtures, jobs, |fixture| {
        let started = Instant::now();
        let outcome = soak::run_measured(fixture, seed, actions, checks);
        SoakRun::of(&fixture.name, outcome, started.elapsed())
    });
    let mut failures = Vec::new();
    for run in &runs {
        report::emit(&report::soak_line(
            &run.name,
            seed,
            actions,
            &run.line_outcome(),
            run.took,
        ));
        if let Some(failure) = &run.failure {
            report::emit(&failure.to_string());
            failures.push(failure.to_string());
        }
    }
    stress_report::publish(seed, actions, &runs);
    assert!(
        failures.is_empty(),
        "{} of {} boards failed the soak:\n{}",
        failures.len(),
        fixtures.len(),
        failures.join("\n")
    );
}

/// Per-PR: a short soak on the fixed boards, the file round trip after the
/// last action.
#[test]
fn soak_short() {
    let checks = Checks {
        drc_every: 0,
        round_trip_every: 0,
    };
    soak_all(DEFAULT_SEED, 150, Corpus::None, checks, 1);
}

/// Scheduled: 10 000 actions on every board, or a replay of a failure.
#[test]
#[ignore = "long: the scheduled routing soak runs it in release"]
fn soak_long() {
    let seed = env_number("SIGNEX_SOAK_SEED").unwrap_or(DEFAULT_SEED);
    let actions = env_number("SIGNEX_SOAK_ACTIONS").unwrap_or(10_000);
    let checks = Checks {
        drc_every: env_number("SIGNEX_SOAK_DRC_EVERY").unwrap_or(25) as usize,
        round_trip_every: env_number("SIGNEX_SOAK_ROUND_TRIP_EVERY").unwrap_or(1_000) as usize,
    };
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    let jobs = env_number("SIGNEX_SOAK_JOBS").map_or(threads, |n| n as usize);
    soak_all(seed, actions as usize, Corpus::All, checks, jobs);
}

/// A seed replays exactly: the same actions give the same board, with the
/// DRC delta and the file round trip checked along the way.
#[test]
fn a_soak_seed_replays_exactly() {
    let fixture = fixtures::synthetic();
    let checks = Checks {
        drc_every: 3,
        round_trip_every: 20,
    };
    let run = || {
        let (tally, board) = soak::run(&fixture, 7, 60, checks).unwrap_or_else(|f| panic!("{f}"));
        (tally, serde_json::to_string(&board).expect("serializes"))
    };
    let first = run();
    assert!(
        first.0.landed > 0,
        "the run routed something: {:?}",
        first.0
    );
    assert!(
        first.0.drc_checks > 0 && first.0.round_trips > 0,
        "the costlier checks ran: {:?}",
        first.0
    );
    assert_eq!(run(), first);
}

fn bench_all(seed: u64, actions: usize, corpus: Corpus) -> Vec<BoardBench> {
    let benches: Vec<BoardBench> = boards(corpus)
        .iter()
        .map(|fixture| bench::bench(fixture, seed, actions))
        .collect();
    for b in &benches {
        report::emit(&b.line());
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

/// The report lines keep the `key=value` form the scheduled job's report
/// and `docs/PERFORMANCE.md` read: a soak line passed and failed, a bench
/// line with its latency budget.
#[test]
fn the_report_lines_name_every_measure() {
    use std::time::Duration;

    let tally = soak::Tally {
        landed: 3,
        drc_checks: 1,
        round_trips: 2,
        ..soak::Tally::default()
    };
    let passed = report::soak_line("b", 0x2482, 10, &Ok(tally), Duration::from_secs(2));
    assert_eq!(
        passed,
        "ROUTING-SOAK board=b seed=0x2482 actions=10 result=ok landed=3 refused=0 edits=0 \
         undos=0 redos=0 drc_checks=1 round_trips=2 seconds=2.0"
    );
    let failure = soak::Failure {
        board: "b".into(),
        seed: 0x2482,
        index: 4,
        action: soak::Action::Undo,
        message: "m".into(),
    };
    let failed = report::soak_line("b", 0x2482, 10, &Err(failure), Duration::ZERO);
    assert!(failed.ends_with("result=FAILED at_action=4 action=Undo seconds=0.0"));
    let slow = BoardBench {
        name: "b".into(),
        latencies: vec![Duration::from_millis(40)],
        ..BoardBench::default()
    };
    let line = slow.line();
    assert!(
        line.starts_with("ROUTING-BENCH board=b seed=0x0 actions=0 "),
        "{line}"
    );
    assert!(
        line.contains(" p50_ms=40.00 p95_ms=40.00 latency_budget=over "),
        "{line}"
    );
}

/// Per-PR: the DRC delta and convergence on a short run per fixed board.
#[test]
fn bench_short() {
    let benches = bench_all(DEFAULT_SEED, 30, Corpus::None);
    assert!(
        benches.iter().any(|b| b.landed > 0 && !b.shoves.is_empty()),
        "the bench routed and shoved something"
    );
    assert_delta_and_convergence(&benches);
}

/// Scheduled, in release: every threshold of #2483, latency on the median
/// board by copper count. Latency is reported on every run and asserted
/// only with `SIGNEX_ROUTING_ASSERT_LATENCY=1` (the scheduled job).
#[test]
#[ignore = "latency is only meaningful in release: the scheduled job runs it"]
fn bench_thresholds() {
    let seed = env_number("SIGNEX_SOAK_SEED").unwrap_or(DEFAULT_SEED);
    let actions = env_number("SIGNEX_BENCH_ACTIONS").unwrap_or(300) as usize;
    let mut benches = bench_all(seed, actions, Corpus::All);
    assert_delta_and_convergence(&benches);
    benches.sort_by_key(|b| b.copper);
    let median = &benches[benches.len() / 2];
    let (p50, p95) = (median.percentile(0.5), median.percentile(0.95));
    let asserted = report::latency_asserted();
    report::emit(&format!(
        "ROUTING-BENCH-MEDIAN board={} p50_ms={} p95_ms={} latency_budget={} latency_asserted={}",
        median.name,
        report::ms(p50),
        report::ms(p95),
        report::budget(median.within_latency()),
        if asserted { "yes" } else { "no" },
    ));
    if !asserted {
        println!(
            "latency not asserted: set {}=1 to fail on it",
            report::ASSERT_LATENCY
        );
        return;
    }
    assert!(p50 <= MEDIAN_LIMIT, "{}: median {p50:?}", median.name);
    assert!(p95 <= P95_LIMIT, "{}: p95 {p95:?}", median.name);
}
