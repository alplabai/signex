//! Boards soaked side by side: each worker thread takes the next board
//! until none is left, and the results come back in board order, so the
//! report reads the same whatever thread finished first.

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

/// `work` applied to every item on up to `jobs` threads, in item order.
pub fn map_in_order<T: Sync, R: Send>(
    items: &[T],
    jobs: usize,
    work: impl Fn(&T) -> R + Sync,
) -> Vec<R> {
    let next = AtomicUsize::new(0);
    let done: Mutex<Vec<Option<R>>> = Mutex::new(items.iter().map(|_| None).collect());
    thread::scope(|scope| {
        for _ in 0..jobs.clamp(1, items.len().max(1)) {
            scope.spawn(|| {
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    let Some(item) = items.get(index) else {
                        break;
                    };
                    let result = work(item);
                    done.lock().expect("a result slot")[index] = Some(result);
                }
            });
        }
    });
    done.into_inner()
        .expect("the results")
        .into_iter()
        .map(|result| result.expect("every item ran"))
        .collect()
}
