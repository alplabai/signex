//! What the pane menu does to a split (mockup row 207): Tile Grid's panes,
//! Move Tab to Other Pane and Close Pane. Generic over the tab key, as the
//! rest of the model. Design: docs/MENU_BAR.md, "Panes".

use crate::app::TileLayout;

use super::model::{Pane, PaneSplit, SplitRefusal};

impl<K: Clone + PartialEq> PaneSplit<K> {
    /// Panes in `direction` holding `groups` of `docked` (positions in the
    /// strip order), the pane with `active` focused and showing it. Every
    /// tab is listed, and tabs opened later join the focused pane. `None`
    /// with fewer than two panes.
    pub fn from_groups(
        docked: &[K],
        groups: &[Vec<usize>],
        direction: TileLayout,
        active: &K,
    ) -> Option<Self> {
        let panes: Vec<Pane<K>> = groups
            .iter()
            .map(|group| {
                let tabs: Vec<K> = group
                    .iter()
                    .filter_map(|&at| docked.get(at).cloned())
                    .collect();
                let shown = tabs.first().cloned();
                Pane { tabs, shown }
            })
            .filter(|pane| !pane.tabs.is_empty())
            .collect();
        if panes.len() < 2 {
            return None;
        }
        let focused = panes
            .iter()
            .position(|pane| pane.tabs.contains(active))
            .unwrap_or(0);
        let mut split = Self {
            direction,
            panes,
            rest: focused,
            focused,
        };
        if split.panes[focused].tabs.contains(active) {
            split.panes[focused].shown = Some(active.clone());
        }
        Some(split)
    }

    /// Move Tab to Other Pane: `tab`, shown in the focused pane, moves to
    /// the next pane (the first after the last), which takes the focus and
    /// shows it. A pane left with no tab closes; refused when that would
    /// leave one pane.
    pub fn move_tab(&mut self, docked: &[K], tab: &K) -> Result<(), SplitRefusal> {
        self.prune(docked);
        let shown = self.resolve(docked);
        let at = docked
            .iter()
            .position(|key| key == tab)
            .ok_or(SplitRefusal::NotDocked)?;
        let from = self.focused_of(&shown, at).ok_or(SplitRefusal::NotDocked)?;
        if shown.len() < 2 || (shown[from].tabs.len() < 2 && shown.len() < 3) {
            return Err(SplitRefusal::LastTab);
        }
        let to = (from + 1) % shown.len();
        let (from_pane, to_pane) = (shown[from].pane, shown[to].pane);
        for pane in &mut self.panes {
            pane.tabs.retain(|key| key != tab);
        }
        self.panes[to_pane].tabs.push(tab.clone());
        if self.panes[from_pane].shown.as_ref() == Some(tab) {
            self.panes[from_pane].shown = None;
        }
        self.panes[to_pane].shown = Some(tab.clone());
        self.focused = to_pane;
        self.prune(docked);
        Ok(())
    }

    /// Close Pane: shown pane `n` closes and its tabs join the next pane
    /// (the one before, for the last), which takes the focus. No document
    /// closes. Whether two panes or more are left.
    pub fn close_pane(&mut self, docked: &[K], n: usize) -> Result<bool, SplitRefusal> {
        let shown = self.resolve(docked);
        if n >= shown.len() {
            return Err(SplitRefusal::NotTiled);
        }
        if shown.len() < 2 {
            return Err(SplitRefusal::LastTab);
        }
        let into = if n + 1 < shown.len() { n + 1 } else { n - 1 };
        let (gone, kept) = (shown[n].pane, shown[into].pane);
        let moved: Vec<K> = shown[n]
            .tabs
            .iter()
            .filter_map(|&at| docked.get(at).cloned())
            .collect();
        for key in moved {
            if !self.panes[kept].tabs.contains(&key) {
                self.panes[kept].tabs.push(key);
            }
        }
        self.panes[gone].tabs.clear();
        self.panes[gone].shown = None;
        if self.rest == gone {
            self.rest = kept;
        }
        self.focused = kept;
        Ok(self.prune(docked))
    }
}
