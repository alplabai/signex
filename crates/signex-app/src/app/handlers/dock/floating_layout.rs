use crate::dock::{DockMessage, PanelPosition};

use super::super::super::*;

impl Signex {
    pub(super) fn handle_dock_floating_layout_message(&mut self, dock_message: &DockMessage) -> bool {
        match dock_message {
            DockMessage::TabDragStart(..) => {
                self.interaction_state.tab_drag_origin = Some(self.interaction_state.last_mouse_pos);
                true
            }
            DockMessage::FloatingDragEnd(index) => {
                let index = *index;
                if let Some(panel) = self.document_state.dock.floating.get(index) {
                    let (window_width, window_height) = self.ui_state.window_size;
                    let zone = 120.0;
                    let center_x = panel.x + panel.width / 2.0;
                    let center_y = panel.y + panel.height / 4.0;
                    let target = if center_x < zone {
                        Some(PanelPosition::Left)
                    } else if center_x > window_width - zone {
                        Some(PanelPosition::Right)
                    } else if center_y > window_height - zone {
                        Some(PanelPosition::Bottom)
                    } else {
                        None
                    };
                    crate::diagnostics::log_debug(format!(
                        "[dock-back] fp=({:.0},{:.0}) win=({window_width:.0},{window_height:.0}) target={target:?}",
                        panel.x, panel.y
                    ));
                    if let Some(position) = target {
                        self.document_state
                            .dock
                            .update(DockMessage::DockFloatingTo(index, position));
                    }
                }
                true
            }
            _ => false,
        }
    }
}