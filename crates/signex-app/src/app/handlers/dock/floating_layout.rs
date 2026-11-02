use crate::dock::{DockMessage, DockTabMenu, PanelPosition, drop_target};

use super::super::super::*;

impl Signex {
    /// Drop target of the floating panel at `index` for the current window
    /// and region sizes. The single decision behind the live preview and
    /// the release.
    pub(crate) fn dock_drop_target(&self, index: usize) -> Option<PanelPosition> {
        let panel = self.document_state.dock.floating.get(index)?;
        let ui = &self.ui_state;
        let geometry = self.document_state.dock.drop_geometry(
            ui.window_size,
            (ui.left_width, ui.right_width, ui.bottom_height),
        );
        drop_target(panel, &geometry)
    }

    /// Store the region the dragged floating panel would dock into, for the
    /// view to draw. Call after every move of a dragged panel.
    pub(crate) fn refresh_dock_drop_preview(&mut self) {
        let target = self
            .document_state
            .dock
            .floating
            .iter()
            .position(|fp| fp.dragging)
            .and_then(|index| self.dock_drop_target(index));
        self.document_state.dock.drop_preview = target;
    }

    pub(super) fn handle_dock_floating_layout_message(
        &mut self,
        dock_message: &DockMessage,
    ) -> bool {
        match dock_message {
            DockMessage::TabDragStart(..) => {
                self.interaction_state.tab_drag_origin =
                    Some(self.interaction_state.last_mouse_pos);
                false
            }
            DockMessage::TabContextMenuRequested(position, index) => {
                self.close_all_context_menus();
                let (x, y) = self.interaction_state.last_mouse_pos;
                let kind = self
                    .document_state
                    .dock
                    .panel_kinds(*position)
                    .get(*index)
                    .copied();
                self.document_state.dock.tab_menu = kind.map(|kind| DockTabMenu {
                    position: *position,
                    kind,
                    x,
                    y,
                });
                true
            }
            DockMessage::FloatingDragEnd(index) => {
                let index = *index;
                let target = self.dock_drop_target(index);
                crate::diagnostics::log_debug(format!("[dock-back] fp={index} target={target:?}"));
                if let Some(position) = target {
                    self.document_state
                        .dock
                        .update(DockMessage::DockFloatingTo(index, position));
                }
                self.document_state.dock.drop_preview = None;
                true
            }
            _ => false,
        }
    }
}
