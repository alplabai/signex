//! Back / forward history of the schematic sheets the user has been on.

use std::path::{Path, PathBuf};

/// The most sheets remembered each way.
pub const HISTORY_CAP: usize = 64;

#[derive(Debug, Default, Clone)]
pub struct SheetHistory {
    back: Vec<PathBuf>,
    forward: Vec<PathBuf>,
    current: Option<PathBuf>,
}

impl SheetHistory {
    /// The user is now on `path`. Moving to a new sheet drops the forward
    /// list; being told about the sheet already current changes nothing, which
    /// is what keeps a back or forward step from recording itself.
    pub fn visit(&mut self, path: &Path) {
        if self.current.as_deref() == Some(path) {
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

    /// Step back: the sheet to go to, now the current one.
    pub fn back(&mut self) -> Option<PathBuf> {
        let target = self.back.pop()?;
        if let Some(from) = self.current.replace(target.clone()) {
            self.forward.push(from);
        }
        Some(target)
    }

    /// Step forward: the sheet to go to, now the current one.
    pub fn forward(&mut self) -> Option<PathBuf> {
        let target = self.forward.pop()?;
        if let Some(from) = self.current.replace(target.clone()) {
            self.back.push(from);
        }
        Some(target)
    }

    pub fn current(&self) -> Option<&Path> {
        self.current.as_deref()
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

    #[test]
    fn back_walks_to_earlier_sheets_and_stops_at_the_first() {
        let mut h = visited(&["a", "b", "c"]);
        assert_eq!(h.back(), Some(p("b")));
        assert_eq!(h.back(), Some(p("a")));
        assert_eq!(h.back(), None);
        assert_eq!(h.current(), Some(Path::new("a")));
    }

    #[test]
    fn forward_returns_along_the_way_back_and_stops_at_the_last() {
        let mut h = visited(&["a", "b", "c"]);
        h.back();
        h.back();
        assert_eq!(h.forward(), Some(p("b")));
        assert_eq!(h.forward(), Some(p("c")));
        assert_eq!(h.forward(), None);
    }

    #[test]
    fn a_step_does_not_record_itself_when_the_tab_switch_reports_it() {
        let mut h = visited(&["a", "b"]);
        let target = h.back().expect("back");
        h.visit(&target);
        assert!(h.can_go_forward(), "forward survives the reported visit");
        assert_eq!(h.forward(), Some(p("b")));
    }

    #[test]
    fn visiting_a_new_sheet_after_going_back_drops_forward() {
        let mut h = visited(&["a", "b", "c"]);
        h.back();
        h.visit(Path::new("d"));
        assert!(!h.can_go_forward());
        assert_eq!(h.back(), Some(p("b")));
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
        while h.back().is_some() {
            steps += 1;
        }
        assert_eq!(steps, HISTORY_CAP);
    }
}
