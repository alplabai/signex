//! The stress report of a soak run (#2967): per board the actions run, the
//! failure with the seed, action index and the command that replays it,
//! the histogram of the rounds each shove took and the action latency at
//! p50, p95 and p99. Printed (and appended to the report file) as a
//! summary table; with `SIGNEX_ROUTING_REPORT_JSON` set, also written to
//! that file as one JSON document. Format: `docs/PERFORMANCE.md`, "Router
//! soak and bench".

use std::collections::BTreeMap;
use std::time::Duration;

use serde_json::{Value, json};

use crate::bench::ROUNDS_LIMIT;
use crate::report::{emit, ms};
use crate::soak::{Action, Failure, Outcome, Tally};
use crate::stats::{converged_share, percentile, rounds_histogram};

/// The file the JSON report is written to.
pub const REPORT_JSON: &str = "SIGNEX_ROUTING_REPORT_JSON";
/// What the JSON document is, and its version: a reader checks both.
pub const FORMAT: &str = "signex-routing-stress";
pub const VERSION: u32 = 1;

/// One board's soak, without the board it left.
pub struct SoakRun {
    pub name: String,
    pub tally: Tally,
    pub latencies: Vec<(Action, Duration)>,
    pub failure: Option<Failure>,
    pub took: Duration,
}

impl SoakRun {
    pub fn of(name: &str, outcome: Outcome, took: Duration) -> Self {
        SoakRun {
            name: name.to_string(),
            tally: outcome.tally,
            latencies: outcome.latencies,
            failure: outcome.result.err(),
            took,
        }
    }

    /// The outcome as the soak's report line takes it.
    pub fn line_outcome(&self) -> Result<Tally, Failure> {
        match &self.failure {
            None => Ok(self.tally.clone()),
            Some(failure) => Err(failure.clone()),
        }
    }

    fn samples(&self) -> Vec<Duration> {
        self.latencies.iter().map(|(_, took)| *took).collect()
    }

    /// The actions run of the `actions` asked: up to the failing one.
    fn actions_run(&self, actions: usize) -> usize {
        self.failure.as_ref().map_or(actions, |f| f.index + 1)
    }
}

/// Milliseconds as a JSON number, to the microsecond.
fn ms_number(duration: Duration) -> f64 {
    (duration.as_secs_f64() * 1e6).round() / 1e3
}

fn latency_json(samples: &[Duration]) -> Value {
    json!({
        "samples": samples.len(),
        "p50": ms_number(percentile(samples, 0.5)),
        "p95": ms_number(percentile(samples, 0.95)),
        "p99": ms_number(percentile(samples, 0.99)),
    })
}

/// One board's entry in the JSON report.
pub fn board_json(run: &SoakRun, actions: usize) -> Value {
    let t = &run.tally;
    let mut by_action: BTreeMap<Action, Vec<Duration>> = BTreeMap::new();
    for (action, took) in &run.latencies {
        by_action.entry(*action).or_default().push(*took);
    }
    let by_action: serde_json::Map<String, Value> = by_action
        .iter()
        .map(|(action, samples)| (format!("{action:?}"), latency_json(samples)))
        .collect();
    let failure = run.failure.as_ref().map(|f| {
        json!({
            "index": f.index,
            "action": format!("{:?}", f.action),
            "message": f.message,
            "replay": f.replay(),
        })
    });
    json!({
        "board": run.name,
        "result": if run.failure.is_some() { "failed" } else { "ok" },
        "actions": actions,
        "actions_run": run.actions_run(actions),
        "seconds": (run.took.as_secs_f64() * 10.0).round() / 10.0,
        "counts": {
            "landed": t.landed, "refused": t.refused, "edits": t.edits,
            "drags": t.drags, "vias_placed": t.vias_placed, "vias_moved": t.vias_moved,
            "undos": t.undos, "redos": t.redos, "drc_checks": t.drc_checks,
            "round_trips": t.round_trips, "link_checks": t.link_checks,
        },
        "shove": {
            "tries": t.shoves.len(),
            "rounds_limit": ROUNDS_LIMIT,
            "rounds_histogram": rounds_histogram(&t.shoves),
            "converged_pct": (converged_share(&t.shoves) * 1000.0).round() / 10.0,
        },
        "latency_ms": latency_json(&run.samples()),
        "latency_ms_by_action": by_action,
        "failure": failure,
    })
}

/// The JSON report of a run of `actions` actions per board from `seed`.
pub fn document(seed: u64, actions: usize, runs: &[SoakRun]) -> Value {
    json!({
        "format": FORMAT,
        "version": VERSION,
        "seed": format!("{seed:#x}"),
        "actions": actions,
        "boards": runs.iter().map(|run| board_json(run, actions)).collect::<Vec<_>>(),
        "failed": runs.iter().filter(|run| run.failure.is_some()).count(),
    })
}

/// The summary table of a run of `actions` actions per board: a header and
/// one row per board.
pub fn table(runs: &[SoakRun], actions: usize) -> Vec<String> {
    let mut rows = vec![
        format!(
            "| board | result | actions run | landed | shoves | within {ROUNDS_LIMIT} rounds % \
             | p50 ms | p95 ms | p99 ms | failed at |"
        ),
        "|---|---|---|---|---|---|---|---|---|---|".to_string(),
    ];
    for run in runs {
        let samples = run.samples();
        let t = &run.tally;
        let (result, at) = match &run.failure {
            None => ("ok", "-".to_string()),
            Some(f) => ("FAILED", format!("{} ({:?})", f.index, f.action)),
        };
        rows.push(format!(
            "| {} | {result} | {} | {} | {} | {:.1} | {} | {} | {} | {at} |",
            run.name,
            run.actions_run(actions),
            t.landed,
            t.shoves.len(),
            converged_share(&t.shoves) * 100.0,
            ms(percentile(&samples, 0.5)),
            ms(percentile(&samples, 0.95)),
            ms(percentile(&samples, 0.99)),
        ));
    }
    rows
}

/// Print the table (appended to the report file when one is set) and
/// write the JSON report when `SIGNEX_ROUTING_REPORT_JSON` names a file.
pub fn publish(seed: u64, actions: usize, runs: &[SoakRun]) {
    for row in table(runs, actions) {
        emit(&row);
    }
    let Some(path) = std::env::var(REPORT_JSON).ok().filter(|p| !p.is_empty()) else {
        return;
    };
    let text = serde_json::to_string_pretty(&document(seed, actions, runs))
        .expect("the report serializes");
    std::fs::write(&path, text + "\n").unwrap_or_else(|e| panic!("{REPORT_JSON}={path}: {e}"));
}
