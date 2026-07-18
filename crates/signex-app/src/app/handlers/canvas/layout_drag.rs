use super::super::super::*;
use crate::dock::DockMessage;

impl Signex {
    pub(crate) fn handle_layout_drag_started(&mut self, target: DragTarget) {
        crate::diagnostics::log_debug(format!("[drag] START {target:?}"));
        self.interaction_state.dragging = Some(target);
        self.interaction_state.drag_start_pos = None;
        self.interaction_state.drag_start_size = match target {
            DragTarget::LeftPanel => self.ui_state.left_width,
            DragTarget::RightPanel => self.ui_state.right_width,
            DragTarget::BottomPanel => self.ui_state.bottom_height,
            DragTarget::ComponentsSplit => self.document_state.panel_ctx.components_split,
        };
    }

    pub(crate) fn handle_layout_drag_moved(&mut self, x: f32, y: f32) {
        self.interaction_state.last_mouse_pos = (x, y);
        // BOM column-resize drag: while a header's resize handle is
        // pressed, every mouse-move tick recomputes the column's
        // width as `start_width + (current_x - start_x)`. Min width
        // floors at 40 px so the column doesn't disappear.
        if let Some(preview) = self.document_state.bom_preview.as_mut()
            && let Some(resize) = preview.column_resize
        {
            let new_width = (resize.start_width + (x - resize.start_x)).max(40.0);
            preview.column_widths.insert(resize.idx, new_width);
        }
        // PDF preview pan-drag: while panning, every move tick adds
        // (cursor - press_origin) to the original pan offset so the
        // page slides under the cursor 1:1.
        if let Some(preview) = self.document_state.preview.as_mut()
            && let Some((origin, ox, oy)) = preview.panning
        {
            preview.pan = (origin.0 + (x - ox), origin.1 + (y - oy));
        }
        // Modal drag — accumulate delta into the per-modal offset so the
        // dialog slides under the cursor.
        if let Some((modal, last_x, last_y)) = self.ui_state.modal_dragging {
            let dx = x - last_x;
            let dy = y - last_y;
            let entry = self
                .ui_state
                .modal_offsets
                .entry(modal)
                .or_insert((0.0, 0.0));
            entry.0 += dx;
            entry.1 += dy;
            self.ui_state.modal_dragging = Some((modal, x, y));
        }
        if let Some(target) = self.interaction_state.dragging {
            let pos = match target {
                DragTarget::LeftPanel | DragTarget::RightPanel => x,
                DragTarget::BottomPanel | DragTarget::ComponentsSplit => y,
            };
            if self.interaction_state.drag_start_pos.is_none() {
                self.interaction_state.drag_start_pos = Some(pos);
            }
            if let Some(start) = self.interaction_state.drag_start_pos {
                let delta = pos - start;
                let (current, new_val) = match target {
                    DragTarget::LeftPanel => (
                        self.ui_state.left_width,
                        (self.interaction_state.drag_start_size + delta).clamp(100.0, 500.0),
                    ),
                    DragTarget::RightPanel => (
                        self.ui_state.right_width,
                        (self.interaction_state.drag_start_size - delta).clamp(100.0, 500.0),
                    ),
                    DragTarget::BottomPanel => (
                        self.ui_state.bottom_height,
                        (self.interaction_state.drag_start_size - delta).clamp(60.0, 400.0),
                    ),
                    DragTarget::ComponentsSplit => (
                        self.document_state.panel_ctx.components_split,
                        (self.interaction_state.drag_start_size + delta).clamp(80.0, 600.0),
                    ),
                };
                let new_val = new_val.round();
                if (current - new_val).abs() >= 1.0 {
                    match target {
                        DragTarget::LeftPanel => self.ui_state.left_width = new_val,
                        DragTarget::RightPanel => self.ui_state.right_width = new_val,
                        DragTarget::BottomPanel => self.ui_state.bottom_height = new_val,
                        DragTarget::ComponentsSplit => {
                            self.document_state.panel_ctx.components_split = new_val
                        }
                    }
                }
            }
        }

        if let (Some((pos, idx)), Some((ox, oy))) = (
            self.document_state.dock.tab_drag,
            self.interaction_state.tab_drag_origin,
        ) {
            let dx = x - ox;
            let dy = y - oy;
            // Undock when drag clearly exits tab-strip intent.
            // Vertical exit keeps previous behavior, and a large horizontal
            // sweep also counts as exit so users can drag a right-docked tab
            // directly toward the left edge to re-dock there.
            let moved_far = (dx * dx + dy * dy).sqrt() > 60.0;
            let left_strip_vertically = dy.abs() > 28.0;
            let left_strip_horizontally = dx.abs() > 180.0;
            let left_strip = left_strip_vertically || left_strip_horizontally;
            if moved_far && left_strip {
                self.document_state
                    .dock
                    .update(DockMessage::UndockPanel(pos, idx));
                self.interaction_state.tab_drag_origin = None;
            }
        }

        for fp in &mut self.document_state.dock.floating {
            if fp.dragging {
                fp.x = x - fp.width / 2.0;
                fp.y = y - 15.0;
            }
        }
    }

    /// Returns the tab index to undock when the user is dragging a
    /// document tab and the cursor crosses the main window boundary.
    pub(crate) fn check_tab_auto_detach(&self, cursor_x: f32, cursor_y: f32) -> Option<usize> {
        let (idx, _, _) = self.ui_state.tab_dragging?;
        // Skip if this tab is already undocked (owned by another window).
        let tab = self.document_state.tabs.get(idx)?;
        if self.ui_state.windows.values().any(
            |k| matches!(k, super::super::super::state::WindowKind::UndockedTab { path, .. } if path == &tab.path),
        ) {
            return None;
        }
        let (ww, wh) = self.ui_state.window_size;
        const EDGE_THRESHOLD: f32 = 12.0;
        let past = cursor_x < -EDGE_THRESHOLD
            || cursor_x > ww + EDGE_THRESHOLD
            || cursor_y < -EDGE_THRESHOLD
            || cursor_y > wh + EDGE_THRESHOLD;
        if past { Some(idx) } else { None }
    }

    /// Scan the floating-panel list for one whose drag just crossed the
    /// main window boundary. Returns the index into `dock.floating` so
    /// the dispatcher can chain a `DetachFloatingPanel(idx)` task.
    pub(crate) fn check_floating_panel_auto_detach(
        &self,
        cursor_x: f32,
        cursor_y: f32,
    ) -> Option<usize> {
        let (ww, wh) = self.ui_state.window_size;
        const EDGE_THRESHOLD: f32 = 12.0;
        let past = cursor_x < -EDGE_THRESHOLD
            || cursor_x > ww + EDGE_THRESHOLD
            || cursor_y < -EDGE_THRESHOLD
            || cursor_y > wh + EDGE_THRESHOLD;
        if !past {
            return None;
        }
        self.document_state
            .dock
            .floating
            .iter()
            .position(|fp| fp.dragging)
    }

    /// Altium-style auto-detach. While the user drags a modal's title
    /// bar, watch the cursor; if it crosses the main window boundary by
    /// more than `EDGE_THRESHOLD`, pop the modal out into its own OS
    /// window. Returns the modal that should detach, if any, so the
    /// dispatcher can chain a `DetachModal` task onto the DragMove path.
    pub(crate) fn check_modal_auto_detach(
        &self,
        cursor_x: f32,
        cursor_y: f32,
    ) -> Option<super::super::super::state::ModalId> {
        let (modal, _, _) = self.ui_state.modal_dragging?;
        // Skip if it's already detached — another path owns it now.
        if self
            .ui_state
            .windows
            .values()
            .any(|k| matches!(k, super::super::super::state::WindowKind::DetachedModal(m) if *m == modal))
        {
            return None;
        }
        let (ww, wh) = self.ui_state.window_size;
        // Dead zone so a brief accidental graze doesn't flip the modal out.
        const EDGE_THRESHOLD: f32 = 12.0;
        let past_left = cursor_x < -EDGE_THRESHOLD;
        let past_right = cursor_x > ww + EDGE_THRESHOLD;
        let past_top = cursor_y < -EDGE_THRESHOLD;
        let past_bottom = cursor_y > wh + EDGE_THRESHOLD;
        if past_left || past_right || past_top || past_bottom {
            Some(modal)
        } else {
            None
        }
    }

    pub(crate) fn handle_layout_drag_finished(&mut self) {
        // Always release any in-flight BOM column resize on a
        // global mouse-up — the resize handle's `on_release`
        // doesn't fire reliably when the cursor leaves the handle's
        // 4 px hit zone during the drag.
        if let Some(preview) = self.document_state.bom_preview.as_mut() {
            preview.column_resize = None;
        }
        // Same belt-and-braces release for the PDF preview pan
        // drag — the on_release on the viewport mouse_area can miss
        // when the cursor leaves the modal during a fast drag.
        if let Some(preview) = self.document_state.preview.as_mut() {
            preview.panning = None;
        }
        if self.interaction_state.dragging.is_some() {
            crate::diagnostics::log_debug("[drag] END");
            self.interaction_state.dragging = None;
            self.interaction_state.drag_start_pos = None;
            self.ui_state.tab_dragging = None;
            return;
        }

        self.document_state.dock.tab_drag = None;
        self.interaction_state.tab_drag_origin = None;
        self.ui_state.tab_dragging = None;
        let (mx, my) = self.interaction_state.last_mouse_pos;
        let (ww, wh) = self.ui_state.window_size;
        let dock_zone = 120.0;
        let has_dragging = self
            .document_state
            .dock
            .floating
            .iter()
            .any(|fp| fp.dragging);
        crate::diagnostics::log_debug(format!(
            "[dock-end] mouse=({mx:.0},{my:.0}) win=({ww:.0},{wh:.0}) floating={} dragging={has_dragging}",
            self.document_state.dock.floating.len()
        ));
        if let Some(drag_idx) = self
            .document_state
            .dock
            .floating
            .iter()
            .position(|fp| fp.dragging)
        {
            let target = if mx < dock_zone {
                Some(PanelPosition::Left)
            } else if mx > ww - dock_zone {
                Some(PanelPosition::Right)
            } else if my > wh - dock_zone {
                Some(PanelPosition::Bottom)
            } else {
                None
            };
            crate::diagnostics::log_debug(format!("[dock-end] target={target:?}"));
            if let Some(pos) = target {
                self.document_state
                    .dock
                    .update(DockMessage::DockFloatingTo(drag_idx, pos));
            } else {
                self.document_state.dock.floating[drag_idx].dragging = false;
            }
        } else {
            for fp in &mut self.document_state.dock.floating {
                fp.dragging = false;
            }
        }
    }
}
