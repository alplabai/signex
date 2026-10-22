//! Dock area state: construction, panel management, and update handling.

use super::types::*;
use crate::panels::{self, PanelKind};

impl Default for DockArea {
    fn default() -> Self {
        Self::new()
    }
}

impl DockArea {
    pub fn new() -> Self {
        Self {
            left: DockRegion {
                panels: Vec::new(),
                active: 0,
                collapsed: false,
                tab_offset: 0,
            },
            right: DockRegion {
                panels: Vec::new(),
                active: 0,
                collapsed: false,
                tab_offset: 0,
            },
            bottom: DockRegion {
                panels: Vec::new(),
                active: 0,
                collapsed: false,
                tab_offset: 0,
            },
            floating: Vec::new(),
            tab_drag: None,
            hovered_tab: None,
        }
    }

    /// Where `kind` currently lives, searching every region in display
    /// order and then the floating list. `None` when it is not in the
    /// dock at all — note that it may still own a detached OS window,
    /// which only `Signex::show_panel` can see.
    pub fn locate(&self, kind: PanelKind) -> Option<PanelSite> {
        for position in [
            PanelPosition::Left,
            PanelPosition::Right,
            PanelPosition::Bottom,
        ] {
            let region = self.region(position);
            if let Some(idx) = region.panels.iter().position(|k| *k == kind) {
                return Some(PanelSite::Docked(position, idx));
            }
        }
        self.floating
            .iter()
            .position(|fp| fp.kind == kind)
            .map(PanelSite::Floating)
    }

    /// Dock `kind` at `position`, but only when it is not already open
    /// somewhere in the dock. Placement only: an existing panel is
    /// left exactly where and how it is, active tab included.
    ///
    /// The guard spans the whole dock, not just `position` (#641).
    /// Per-region dedupe let one kind be docked up to three times, and
    /// `fonts::write_dock_layout` then persisted the duplicates. The
    /// same global guard is what de-duplicates a `prefs.json` written
    /// before this fix: `fonts::read_dock_layout` replays a saved
    /// layout through here region by region, so the second and third
    /// copies are dropped on load.
    pub fn add_panel(&mut self, position: PanelPosition, kind: PanelKind) {
        if self.locate(kind).is_some() {
            return;
        }
        self.region_mut(position).panels.push(kind);
    }

    /// "Open panel `kind`" — dock it at its default region when it is
    /// nowhere yet, otherwise bring the copy that already exists into
    /// view. Every user-facing open gesture goes through this or
    /// [`Self::show_panel_at`].
    pub fn show_panel(&mut self, kind: PanelKind) {
        self.show_panel_at(PanelPosition::default_for(kind), kind);
    }

    /// [`Self::show_panel`] with an explicit region, for gestures that
    /// name one — dropping a floating panel onto a dock zone.
    pub fn show_panel_at(&mut self, position: PanelPosition, kind: PanelKind) {
        match self.locate(kind) {
            Some(PanelSite::Docked(pos, idx)) => self.reveal_tab(pos, idx),
            // A floating panel already paints over the whole main
            // window, so it needs no revealing. Deliberately not
            // re-ordered to the top of `floating` either: the drag
            // subscription addresses floating panels by index, and
            // shuffling the vector mid-drag would move the wrong one.
            Some(PanelSite::Floating(_)) => {}
            None => {
                let region = self.region_mut(position);
                region.panels.push(kind);
                let idx = region.panels.len() - 1;
                self.reveal_tab(position, idx);
            }
        }
    }

    /// Make the tab at `idx` the active one in `position` and expand
    /// the region if the user had collapsed it. Opening a panel that
    /// is docked behind another tab has to do this — otherwise "open
    /// Signal" looks like it did nothing.
    fn reveal_tab(&mut self, position: PanelPosition, idx: usize) {
        let region = self.region_mut(position);
        if idx < region.panels.len() {
            region.active = idx;
            region.collapsed = false;
        }
    }

    fn region(&self, position: PanelPosition) -> &DockRegion {
        match position {
            PanelPosition::Left => &self.left,
            PanelPosition::Right => &self.right,
            PanelPosition::Bottom => &self.bottom,
        }
    }

    fn region_mut(&mut self, position: PanelPosition) -> &mut DockRegion {
        match position {
            PanelPosition::Left => &mut self.left,
            PanelPosition::Right => &mut self.right,
            PanelPosition::Bottom => &mut self.bottom,
        }
    }

    pub fn update(&mut self, msg: DockMessage) {
        match msg {
            DockMessage::SelectTab(pos, idx) => {
                let region = match pos {
                    PanelPosition::Left => &mut self.left,
                    PanelPosition::Right => &mut self.right,
                    PanelPosition::Bottom => &mut self.bottom,
                };
                if idx < region.panels.len() {
                    region.active = idx;
                    region.collapsed = false;
                }
            }
            DockMessage::TabScroll(pos, delta) => {
                let region = match pos {
                    PanelPosition::Left => &mut self.left,
                    PanelPosition::Right => &mut self.right,
                    PanelPosition::Bottom => &mut self.bottom,
                };
                let new_off = region.tab_offset as i32 + delta;
                let max_off = region.panels.len().saturating_sub(1) as i32;
                region.tab_offset = new_off.clamp(0, max_off) as usize;
            }
            DockMessage::ToggleCollapse(pos) => {
                let region = match pos {
                    PanelPosition::Left => &mut self.left,
                    PanelPosition::Right => &mut self.right,
                    PanelPosition::Bottom => &mut self.bottom,
                };
                region.collapsed = !region.collapsed;
            }
            DockMessage::ClosePanel(pos, idx) => {
                let region = match pos {
                    PanelPosition::Left => &mut self.left,
                    PanelPosition::Right => &mut self.right,
                    PanelPosition::Bottom => &mut self.bottom,
                };
                if idx < region.panels.len() {
                    region.panels.remove(idx);
                    if region.active >= region.panels.len() && region.active > 0 {
                        region.active -= 1;
                    }
                }
            }
            DockMessage::TabDragStart(pos, idx) => {
                self.tab_drag = Some((pos, idx));
            }
            DockMessage::TabHoverEnter(pos, idx) => {
                self.hovered_tab = Some((pos, idx));
            }
            DockMessage::TabHoverExit(pos, idx) => {
                if self.hovered_tab == Some((pos, idx)) {
                    self.hovered_tab = None;
                }
            }
            DockMessage::ReorderTab { pos, from, to } => {
                let region = match pos {
                    PanelPosition::Left => &mut self.left,
                    PanelPosition::Right => &mut self.right,
                    PanelPosition::Bottom => &mut self.bottom,
                };
                if from < region.panels.len() && to < region.panels.len() {
                    let panel = region.panels.remove(from);
                    region.panels.insert(to, panel);
                    region.active = to;
                }
            }
            DockMessage::TabClick(pos, idx) => {
                // Mouse-up on tab: if UndockPanel did not fire, treat
                // as click → select. If the drag started on a
                // different tab in the same region, reorder the
                // panels vector instead so the user can drag tabs to
                // shuffle them within the strip.
                if let Some((drag_pos, from)) = self.tab_drag.take() {
                    if drag_pos == pos && from != idx {
                        let region = match pos {
                            PanelPosition::Left => &mut self.left,
                            PanelPosition::Right => &mut self.right,
                            PanelPosition::Bottom => &mut self.bottom,
                        };
                        if from < region.panels.len() && idx < region.panels.len() {
                            let panel = region.panels.remove(from);
                            region.panels.insert(idx, panel);
                            region.active = idx;
                            region.collapsed = false;
                        }
                    } else {
                        let region = match pos {
                            PanelPosition::Left => &mut self.left,
                            PanelPosition::Right => &mut self.right,
                            PanelPosition::Bottom => &mut self.bottom,
                        };
                        if idx < region.panels.len() {
                            region.active = idx;
                            region.collapsed = false;
                        }
                    }
                }
            }
            DockMessage::UndockPanel(pos, idx) => {
                self.tab_drag = None;
                let region = match pos {
                    PanelPosition::Left => &mut self.left,
                    PanelPosition::Right => &mut self.right,
                    PanelPosition::Bottom => &mut self.bottom,
                };
                if idx < region.panels.len() {
                    let kind = region.panels.remove(idx);
                    if region.active >= region.panels.len() && region.active > 0 {
                        region.active -= 1;
                    }
                    // Create floating panel at cursor position
                    self.floating.push(FloatingPanel {
                        kind,
                        x: 300.0,
                        y: 150.0,
                        width: 280.0,
                        height: 400.0,
                        dragging: true, // start dragging immediately
                    });
                }
            }
            DockMessage::StartDragFloating(idx) => {
                if let Some(fp) = self.floating.get_mut(idx) {
                    fp.dragging = true;
                }
            }
            DockMessage::FloatingDragEnd(idx) => {
                // Stop the drag; dock-zone detection handled by app before this.
                if let Some(fp) = self.floating.get_mut(idx) {
                    fp.dragging = false;
                }
            }
            DockMessage::MoveFloating(idx, dx, dy) => {
                if let Some(fp) = self.floating.get_mut(idx) {
                    fp.x += dx;
                    fp.y += dy;
                }
            }
            DockMessage::DockFloating(idx) => {
                // The floating panel's own "dock" button — no drop
                // target, so the kind goes to its home region rather
                // than always to the right column.
                if idx < self.floating.len() {
                    let fp = self.floating.remove(idx);
                    self.show_panel(fp.kind);
                }
            }
            DockMessage::DockFloatingTo(idx, target) => {
                // Dropped onto a dock zone: honour the zone the user
                // aimed at, unless the kind is somehow already docked
                // elsewhere, in which case reveal that one.
                if idx < self.floating.len() {
                    let fp = self.floating.remove(idx);
                    self.show_panel_at(target, fp.kind);
                }
            }
            // Panel messages are handled by app.rs before reaching here.
            DockMessage::Panel(_) => {}
            DockMessage::Library(_) => {
                // Routed by `handle_dock_message` directly into the
                // library subsystem before reaching this update path.
                // Reaching here is harmless — no dock state to mutate.
            }
        }
    }

    /// Check if a dock region is collapsed.
    pub fn is_collapsed(&self, position: PanelPosition) -> bool {
        match position {
            PanelPosition::Left => self.left.collapsed,
            PanelPosition::Right => self.right.collapsed,
            PanelPosition::Bottom => self.bottom.collapsed,
        }
    }

    /// Check if a dock region currently contains any panels.
    pub fn has_panels(&self, position: PanelPosition) -> bool {
        match position {
            PanelPosition::Left => !self.left.panels.is_empty(),
            PanelPosition::Right => !self.right.panels.is_empty(),
            PanelPosition::Bottom => !self.bottom.panels.is_empty(),
        }
    }

    /// Panels currently docked in `position`, in display order. Used
    /// by the Panels menu to mark open panels with a ✓.
    pub fn panel_kinds(&self, position: PanelPosition) -> &[panels::PanelKind] {
        match position {
            PanelPosition::Left => &self.left.panels,
            PanelPosition::Right => &self.right.panels,
            PanelPosition::Bottom => &self.bottom.panels,
        }
    }

    /// Which tab is active in `position`. Test-facing: the field is
    /// private and `view.rs` reads it through `super::`.
    #[cfg(test)]
    pub(super) fn active_tab(&self, position: PanelPosition) -> usize {
        self.region(position).active
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dock::{PanelSite, types::FloatingPanel};

    fn floating(kind: PanelKind) -> FloatingPanel {
        FloatingPanel {
            kind,
            x: 0.0,
            y: 0.0,
            width: 280.0,
            height: 400.0,
            dragging: false,
        }
    }

    /// #641: the guard used to be per-region, so the same kind could
    /// be docked in all three at once.
    #[test]
    fn a_kind_docked_in_one_region_is_not_added_to_another() {
        let mut dock = DockArea::new();
        dock.add_panel(PanelPosition::Left, PanelKind::Signal);

        dock.add_panel(PanelPosition::Bottom, PanelKind::Signal);
        dock.add_panel(PanelPosition::Right, PanelKind::Signal);

        assert_eq!(dock.panel_kinds(PanelPosition::Left), [PanelKind::Signal]);
        assert!(dock.panel_kinds(PanelPosition::Bottom).is_empty());
        assert!(dock.panel_kinds(PanelPosition::Right).is_empty());
    }

    /// A saved layout written before the fix holds duplicates.
    /// `fonts::read_dock_layout` replays it through `add_panel` region
    /// by region, so the widened guard is also the migration.
    #[test]
    fn replaying_a_duplicated_saved_layout_keeps_only_the_first_copy() {
        let mut dock = DockArea::new();
        for (pos, kind) in [
            (PanelPosition::Left, PanelKind::Signal),
            (PanelPosition::Right, PanelKind::Signal),
            (PanelPosition::Right, PanelKind::Messages),
            (PanelPosition::Bottom, PanelKind::Signal),
            (PanelPosition::Bottom, PanelKind::Messages),
        ] {
            dock.add_panel(pos, kind);
        }

        assert_eq!(dock.panel_kinds(PanelPosition::Left), [PanelKind::Signal]);
        assert_eq!(
            dock.panel_kinds(PanelPosition::Right),
            [PanelKind::Messages]
        );
        assert!(dock.panel_kinds(PanelPosition::Bottom).is_empty());
    }

    #[test]
    fn add_panel_leaves_an_existing_panel_where_it_is() {
        let mut dock = DockArea::new();
        dock.add_panel(PanelPosition::Left, PanelKind::Projects);
        dock.add_panel(PanelPosition::Left, PanelKind::Signal);
        dock.update(DockMessage::SelectTab(PanelPosition::Left, 0));

        // Replay must not steal the active tab — only `show_panel` reveals.
        dock.add_panel(PanelPosition::Bottom, PanelKind::Signal);

        assert_eq!(dock.active_tab(PanelPosition::Left), 0);
    }

    #[test]
    fn show_panel_docks_a_new_kind_at_its_home_region_and_focuses_it() {
        let mut dock = DockArea::new();
        dock.add_panel(PanelPosition::Left, PanelKind::Projects);

        dock.show_panel(PanelKind::Signal);

        assert_eq!(
            dock.panel_kinds(PanelPosition::Left),
            [PanelKind::Projects, PanelKind::Signal]
        );
        assert_eq!(dock.active_tab(PanelPosition::Left), 1);
    }

    /// The papercut behind the "open X did nothing" half of #641:
    /// X was already docked, just behind another tab.
    #[test]
    fn show_panel_reveals_a_panel_hidden_behind_another_tab() {
        let mut dock = DockArea::new();
        dock.add_panel(PanelPosition::Left, PanelKind::Signal);
        dock.add_panel(PanelPosition::Left, PanelKind::Projects);
        dock.update(DockMessage::SelectTab(PanelPosition::Left, 1));
        dock.update(DockMessage::ToggleCollapse(PanelPosition::Left));

        dock.show_panel(PanelKind::Signal);

        assert_eq!(
            dock.panel_kinds(PanelPosition::Left),
            [PanelKind::Signal, PanelKind::Projects]
        );
        assert_eq!(dock.active_tab(PanelPosition::Left), 0);
        assert!(!dock.is_collapsed(PanelPosition::Left));
    }

    #[test]
    fn show_panel_at_honours_an_explicit_drop_target_for_a_new_kind() {
        let mut dock = DockArea::new();

        dock.show_panel_at(PanelPosition::Bottom, PanelKind::Signal);

        assert_eq!(dock.panel_kinds(PanelPosition::Bottom), [PanelKind::Signal]);
        assert!(dock.panel_kinds(PanelPosition::Left).is_empty());
    }

    #[test]
    fn a_floating_panel_blocks_a_second_docked_copy() {
        let mut dock = DockArea::new();
        dock.floating.push(floating(PanelKind::Signal));

        dock.show_panel(PanelKind::Signal);
        dock.add_panel(PanelPosition::Bottom, PanelKind::Signal);

        assert_eq!(dock.locate(PanelKind::Signal), Some(PanelSite::Floating(0)));
        assert!(dock.panel_kinds(PanelPosition::Left).is_empty());
        assert!(dock.panel_kinds(PanelPosition::Bottom).is_empty());
        assert_eq!(dock.floating.len(), 1);
    }

    #[test]
    fn locate_finds_a_docked_panel_by_region_and_index() {
        let mut dock = DockArea::new();
        dock.add_panel(PanelPosition::Left, PanelKind::Projects);
        dock.add_panel(PanelPosition::Left, PanelKind::Signal);

        assert_eq!(
            dock.locate(PanelKind::Signal),
            Some(PanelSite::Docked(PanelPosition::Left, 1))
        );
        assert_eq!(dock.locate(PanelKind::Drc), None);
    }

    /// Undock leaves exactly one copy — floating, not docked.
    #[test]
    fn undocking_then_reopening_does_not_produce_a_second_copy() {
        let mut dock = DockArea::new();
        dock.add_panel(PanelPosition::Left, PanelKind::Signal);
        dock.update(DockMessage::UndockPanel(PanelPosition::Left, 0));

        dock.show_panel(PanelKind::Signal);

        assert!(dock.panel_kinds(PanelPosition::Left).is_empty());
        assert_eq!(dock.floating.len(), 1);
    }

    /// The floating panel's own dock button carries no drop target, so
    /// it used to send every kind to the right column.
    #[test]
    fn re_docking_a_floating_panel_sends_it_to_its_home_region() {
        let mut dock = DockArea::new();
        dock.floating.push(floating(PanelKind::Signal));

        dock.update(DockMessage::DockFloating(0));

        assert_eq!(dock.panel_kinds(PanelPosition::Left), [PanelKind::Signal]);
        assert!(dock.floating.is_empty());
    }

    #[test]
    fn dropping_a_floating_panel_on_a_zone_docks_it_there() {
        let mut dock = DockArea::new();
        dock.floating.push(floating(PanelKind::Signal));

        dock.update(DockMessage::DockFloatingTo(0, PanelPosition::Bottom));

        assert_eq!(dock.panel_kinds(PanelPosition::Bottom), [PanelKind::Signal]);
        assert_eq!(dock.active_tab(PanelPosition::Bottom), 0);
        assert!(dock.floating.is_empty());
    }
}
