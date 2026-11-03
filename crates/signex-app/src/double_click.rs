//! One double-click rule for the rows the app times itself, because iced's
//! `button` consumes the mouse events a built-in double click would need.
//! The project tree, the Library panel and the sheet navigator share it.

use std::time::{Duration, Instant};

/// Two clicks on the same row at most this far apart are a double click.
pub const DOUBLE_CLICK_WINDOW: Duration = Duration::from_millis(500);

/// The previous click on a row of type `K`.
#[derive(Debug, Clone)]
pub struct DoubleClick<K> {
    last: Option<(K, Instant)>,
}

impl<K> Default for DoubleClick<K> {
    fn default() -> Self {
        Self { last: None }
    }
}

impl<K: PartialEq> DoubleClick<K> {
    /// Record a click on `key` at `now`; `true` when it completes a double
    /// click. A completed double click is consumed, so a third click starts
    /// a new sequence. `now` is a parameter so tests control the clock.
    pub fn register(&mut self, key: K, now: Instant) -> bool {
        let is_double = matches!(
            &self.last,
            Some((prev, at)) if *prev == key && now.duration_since(*at) <= DOUBLE_CLICK_WINDOW
        );
        self.last = if is_double { None } else { Some((key, now)) };
        is_double
    }
}

impl<K> DoubleClick<K> {
    /// Forget the previous click.
    pub fn clear(&mut self) {
        self.last = None;
    }

    /// The row of the previous click, if one is pending.
    pub fn key(&self) -> Option<&K> {
        self.last.as_ref().map(|(key, _)| key)
    }

    /// Change the pending key in place (a row that moved).
    pub fn rekey(&mut self, change: impl FnOnce(&mut K)) {
        if let Some((key, _)) = self.last.as_mut() {
            change(key);
        }
    }

    /// Replace the pending key, dropping the click when `change` returns
    /// `None` (a row that no longer exists). The click time is kept.
    pub fn remap(&mut self, change: impl FnOnce(K) -> Option<K>) {
        self.last = self
            .last
            .take()
            .and_then(|(key, at)| change(key).map(|key| (key, at)));
    }

    /// Move the pending click's time, for tests that must not depend on
    /// how long settling a task takes.
    #[cfg(test)]
    pub(crate) fn restamp(&mut self, at: Instant) {
        if let Some((_, stamp)) = self.last.as_mut() {
            *stamp = at;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_window_boundary_is_inclusive() {
        let t0 = Instant::now();
        let mut click = DoubleClick::default();
        assert!(!click.register("a", t0));
        assert!(click.register("a", t0 + DOUBLE_CLICK_WINDOW));

        assert!(!click.register("a", t0));
        let just_over = DOUBLE_CLICK_WINDOW + Duration::from_millis(1);
        assert!(!click.register("a", t0 + just_over));
    }

    #[test]
    fn a_different_row_starts_a_new_sequence() {
        let t0 = Instant::now();
        let mut click = DoubleClick::default();
        assert!(!click.register("a", t0));
        assert!(!click.register("b", t0 + Duration::from_millis(10)));
        assert!(click.register("b", t0 + Duration::from_millis(20)));
    }

    #[test]
    fn a_completed_double_click_is_consumed() {
        let t0 = Instant::now();
        let mut click = DoubleClick::default();
        click.register("a", t0);
        assert!(click.register("a", t0 + Duration::from_millis(1)));
        assert!(click.key().is_none());
        assert!(!click.register("a", t0 + Duration::from_millis(2)));
    }

    #[test]
    fn remap_drops_a_row_that_is_gone() {
        let mut click = DoubleClick::default();
        click.register(1, Instant::now());
        click.remap(|_| None);
        assert!(click.key().is_none());
    }
}
