//! Dock area state: construction, panel management, and update handling.

use super::types::*;
use crate::panels::{self, PanelKind};

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

    pub fn add_panel(&mut self, position: PanelPosition, kind: PanelKind) {
        let region = match position {
            PanelPosition::Left => &mut self.left,
            PanelPosition::Right => &mut self.right,
            PanelPosition::Bottom => &mut self.bottom,
        };
        if region.panels.contains(&kind) {
            return;
        }
        region.panels.push(kind);
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
                if idx < self.floating.len() {
                    let fp = self.floating.remove(idx);
                    if !self.right.panels.contains(&fp.kind) {
                        self.right.panels.push(fp.kind);
                        self.right.active = self.right.panels.len() - 1;
                        self.right.collapsed = false;
                    }
                }
            }
            DockMessage::DockFloatingTo(idx, target) => {
                if idx < self.floating.len() {
                    let fp = self.floating.remove(idx);
                    let region = match target {
                        PanelPosition::Left => &mut self.left,
                        PanelPosition::Right => &mut self.right,
                        PanelPosition::Bottom => &mut self.bottom,
                    };
                    if !region.panels.contains(&fp.kind) {
                        region.panels.push(fp.kind);
                    }
                    region.active = region
                        .panels
                        .iter()
                        .position(|k| *k == fp.kind)
                        .unwrap_or(0);
                    region.collapsed = false;
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
}
