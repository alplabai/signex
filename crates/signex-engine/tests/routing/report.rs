//! The report: one `key=value` line per board from the soak and the bench,
//! in the form `docs/PERFORMANCE.md` describes ("Router soak and bench").
//! Every line is printed; with `SIGNEX_ROUTING_REPORT` set it is also
//! appended to that file, which the scheduled job uploads.

use std::fs::OpenOptions;
use std::io::Write;
use std::time::Duration;

use crate::soak::{Failure, Tally};

/// The file the report lines are appended to.
const REPORT: &str = "SIGNEX_ROUTING_REPORT";
/// Set to `1` where the latency thresholds are asserted (the scheduled
/// job): elsewhere latency is measured and reported only, since a shared
/// or debug machine times too noisily to fail on.
pub const ASSERT_LATENCY: &str = "SIGNEX_ROUTING_ASSERT_LATENCY";

pub fn latency_asserted() -> bool {
    std::env::var(ASSERT_LATENCY).is_ok_and(|v| v == "1")
}

/// Print `line` and append it to the report file when one is set.
pub fn emit(line: &str) {
    println!("{line}");
    let Some(path) = std::env::var(REPORT).ok().filter(|p| !p.is_empty()) else {
        return;
    };
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .unwrap_or_else(|e| panic!("{REPORT}={path}: {e}"));
    writeln!(file, "{line}").unwrap_or_else(|e| panic!("{REPORT}={path}: {e}"));
}

/// Milliseconds with two decimals.
pub fn ms(duration: Duration) -> String {
    format!("{:.2}", duration.as_secs_f64() * 1000.0)
}

/// `ok` or `over`, for a threshold.
pub fn budget(within: bool) -> &'static str {
    if within { "ok" } else { "over" }
}

/// One board's soak line.
pub fn soak_line(
    name: &str,
    seed: u64,
    actions: usize,
    outcome: &Result<Tally, Failure>,
    took: Duration,
) -> String {
    let head = format!("ROUTING-SOAK board={name} seed={seed:#x} actions={actions}");
    let seconds = took.as_secs_f64();
    match outcome {
        Ok(t) => format!(
            "{head} result=ok landed={} refused={} edits={} undos={} redos={} drc_checks={} \
             round_trips={} seconds={seconds:.1}",
            t.landed, t.refused, t.edits, t.undos, t.redos, t.drc_checks, t.round_trips,
        ),
        Err(f) => format!(
            "{head} result=FAILED at_action={} action={:?} seconds={seconds:.1}",
            f.index, f.action
        ),
    }
}
