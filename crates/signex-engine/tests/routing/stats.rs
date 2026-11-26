//! The numbers the soak and the bench both report (#2483, #2967): latency
//! percentiles, the share of shoves that converged and the histogram of
//! the rounds they took.

use std::time::Duration;

use crate::bench::ROUNDS_LIMIT;
use crate::session::ShoveTry;

/// The sample at `share` (0 to 1) of `samples` sorted, nearest rank; zero
/// with none.
pub fn percentile(samples: &[Duration], share: f64) -> Duration {
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    let Some(last) = sorted.len().checked_sub(1) else {
        return Duration::ZERO;
    };
    sorted[((last as f64) * share).round() as usize]
}

/// Whether a shove converged: it settled (the way cleared, or a push found
/// impossible so the router walks around) within [`ROUNDS_LIMIT`] rounds.
pub fn converged(shove: &ShoveTry) -> bool {
    shove.settled && shove.rounds <= ROUNDS_LIMIT
}

/// The share of `shoves` that converged; 1 with none.
pub fn converged_share(shoves: &[ShoveTry]) -> f64 {
    if shoves.is_empty() {
        return 1.0;
    }
    let within = shoves.iter().filter(|s| converged(s)).count();
    within as f64 / shoves.len() as f64
}

/// How many of `shoves` converged in 0, 1, ... [`ROUNDS_LIMIT`] rounds,
/// then, last, how many did not converge (past the limit or never settled).
pub fn rounds_histogram(shoves: &[ShoveTry]) -> Vec<usize> {
    let mut buckets = vec![0; ROUNDS_LIMIT + 2];
    for shove in shoves {
        let bucket = if converged(shove) {
            shove.rounds
        } else {
            ROUNDS_LIMIT + 1
        };
        buckets[bucket] += 1;
    }
    buckets
}
