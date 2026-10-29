//! Back / forward history of the schematic sheets the user has been on.

use std::path::{Path, PathBuf};

/// The most sheets remembered each way.
pub const HISTORY_CAP: usize = 64;

#[derive(Debug, Default, Clone)]
pub struct SheetHistory {
    back: Vec<PathBuf>,
    forward: Vec<PathBuf>,
    current: Option<PathBuf>,
    /// A step is opening a sheet: the focus change it causes is not a visit.
    held: bool,
}

impl SheetHistory {
    /// The user is now on `path`. Moving to a new sheet drops the forward
    /// list; being told about the sheet already current changes nothing, which
    /// is what keeps a back or forward step from recording itself.
    pub fn visit(&mut self, path: &Path) {
        if self.held || self.current.as_deref() == Some(path) {
            return;
        }
        if let Some(previous) = self.current.replace(path.to_path_buf()) {
            self.back.push(previous);
            if self.back.len() > HISTORY_CAP {
                self.back.remove(0);
            }
        }
        self.forward.clear();
    }

    /// The sheet a step back or forward would go to, without moving.
    pub fn peek(&self, forward: bool) -> Option<&Path> {
        let stack = if forward { &self.forward } else { &self.back };
        stack.last().map(PathBuf::as_path)
    }

    /// Forget the sheet a step would go to: it cannot be opened.
    pub fn drop_entry(&mut self, forward: bool) {
        let stack = if forward {
            &mut self.forward
        } else {
            &mut self.back
        };
        stack.pop();
    }

    /// While `held`, focus changes are not visits: a step opening a sheet
    /// must not clear the forward list through a different spelling of it.
    pub fn hold(&mut self, held: bool) {
        self.held = held;
    }

    /// A step was carried out and the sheet shown is `landed`: the entry is
    /// consumed and the sheet left goes to the other side.
    pub fn arrive(&mut self, forward: bool, landed: &Path) {
        let (from, to) = if forward {
            (&mut self.forward, &mut self.back)
        } else {
            (&mut self.back, &mut self.forward)
        };
        from.pop();
        if let Some(left) = self.current.replace(landed.to_path_buf()) {
            to.push(left);
        }
    }

    pub fn current(&self) -> Option<&Path> {
        self.current.as_deref()
    }

    /// Whether `path` is anywhere in the history.
    pub fn contains(&self, path: &Path) -> bool {
        self.current.as_deref() == Some(path)
            || self.back.iter().chain(&self.forward).any(|p| p == path)
    }

    pub fn can_go_back(&self) -> bool {
        !self.back.is_empty()
    }

    pub fn can_go_forward(&self) -> bool {
        !self.forward.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(name: &str) -> PathBuf {
        PathBuf::from(name)
    }

    fn visited(names: &[&str]) -> SheetHistory {
        let mut h = SheetHistory::default();
        for n in names {
            h.visit(Path::new(n));
        }
        h
    }

    /// A step as the handler does it: peek, hold, "open", arrive.
    fn step(h: &mut SheetHistory, forward: bool) -> Option<PathBuf> {
        let target = h.peek(forward)?.to_path_buf();
        h.hold(true);
        h.hold(false);
        h.arrive(forward, &target);
        Some(target)
    }

    #[test]
    fn back_walks_to_earlier_sheets_and_stops_at_the_first() {
        let mut h = visited(&["a", "b", "c"]);
        assert_eq!(step(&mut h, false), Some(p("b")));
        assert_eq!(step(&mut h, false), Some(p("a")));
        assert_eq!(step(&mut h, false), None);
        assert_eq!(h.current(), Some(Path::new("a")));
    }

    #[test]
    fn forward_returns_along_the_way_back_and_stops_at_the_last() {
        let mut h = visited(&["a", "b", "c"]);
        step(&mut h, false);
        step(&mut h, false);
        assert_eq!(step(&mut h, true), Some(p("b")));
        assert_eq!(step(&mut h, true), Some(p("c")));
        assert_eq!(step(&mut h, true), None);
    }

    #[test]
    fn peeking_moves_nothing_and_a_dropped_entry_is_gone_for_good() {
        let mut h = visited(&["a", "b", "c"]);
        assert_eq!(h.peek(false), Some(Path::new("b")));
        assert_eq!(h.peek(false), Some(Path::new("b")));
        h.drop_entry(false);
        assert!(!h.contains(Path::new("b")));
        assert_eq!(step(&mut h, false), Some(p("a")));
        assert!(!h.contains(Path::new("b")), "not pushed to forward either");
    }

    #[test]
    fn opening_through_another_spelling_during_a_step_keeps_forward() {
        let mut h = visited(&["a", "b", "c"]);
        step(&mut h, false);
        // The step opens "b", but the tab reports it as "B": held, so no visit.
        let target = h.peek(false).expect("a").to_path_buf();
        h.hold(true);
        h.visit(Path::new("A"));
        h.hold(false);
        h.arrive(false, Path::new("A"));
        assert!(h.can_go_forward(), "forward survives the mismatch");
        assert_eq!(h.current(), Some(Path::new("A")));
        assert_eq!(target, p("a"));
    }

    #[test]
    fn a_step_does_not_record_itself_when_the_tab_switch_reports_it() {
        let mut h = visited(&["a", "b"]);
        let target = step(&mut h, false).expect("back");
        h.visit(&target);
        assert!(h.can_go_forward(), "forward survives the reported visit");
        assert_eq!(step(&mut h, true), Some(p("b")));
    }

    #[test]
    fn visiting_a_new_sheet_after_going_back_drops_forward() {
        let mut h = visited(&["a", "b", "c"]);
        step(&mut h, false);
        h.visit(Path::new("d"));
        assert!(!h.can_go_forward());
        assert_eq!(step(&mut h, false), Some(p("b")));
    }

    #[test]
    fn revisiting_the_current_sheet_is_not_a_step() {
        let h = visited(&["a", "a", "a"]);
        assert!(!h.can_go_back());
        assert_eq!(h.current(), Some(Path::new("a")));
    }

    #[test]
    fn the_history_is_bounded() {
        let mut h = SheetHistory::default();
        for i in 0..(HISTORY_CAP + 10) {
            h.visit(Path::new(&format!("s{i}")));
        }
        let mut steps = 0;
        while step(&mut h, false).is_some() {
            steps += 1;
        }
        assert_eq!(steps, HISTORY_CAP);
    }
}
