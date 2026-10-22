//! Shared request throttle for the distributor adapters.

use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

/// Block until `interval` has passed since the last request recorded in
/// `last`, then record now. The lock is held across the sleep so concurrent
/// callers queue and each sees the previous caller's timestamp.
///
/// Sleeps while holding the lock: call it from a blocking context (e.g.
/// `spawn_blocking`), never from an async task or the UI thread.
pub(super) fn polite_wait(last: &Mutex<Option<Instant>>, interval: Duration) {
    // The data is a plain `Option<Instant>`, valid even after a poisoning panic.
    let mut guard = last.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(prev) = *guard {
        let elapsed = prev.elapsed();
        if elapsed < interval {
            std::thread::sleep(interval - elapsed);
        }
    }
    *guard = Some(Instant::now());
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    const CALLERS: u32 = 4;
    /// Timer/scheduler tolerance on the lower bound of the elapsed time.
    const TIMER_SLACK: Duration = Duration::from_millis(10);

    #[test]
    fn concurrent_callers_are_spaced_by_the_interval() {
        let interval = Duration::from_millis(100);
        // Seeded so every caller has to sleep.
        let last = Arc::new(Mutex::new(Some(Instant::now())));

        let start = Instant::now();
        let handles: Vec<_> = (0..CALLERS)
            .map(|_| {
                let last = Arc::clone(&last);
                std::thread::spawn(move || polite_wait(&last, interval))
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
        let total = start.elapsed();

        // Callers are served one at a time, each `interval` after the previous
        // one. Load only lengthens `total`, so this lower bound cannot flake.
        let expected = interval * CALLERS - TIMER_SLACK;
        assert!(
            total >= expected,
            "{CALLERS} callers finished in {total:?}, expected at least {expected:?}"
        );
    }
}
