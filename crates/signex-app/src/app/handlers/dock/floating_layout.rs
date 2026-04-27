use crate::dock::{DockMessage, PanelPosition};

use super::super::super::*;

impl Signex {
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
            DockMessage::FloatingDragEnd(index) => {
                let index = *index;
                if let Some(panel) = self.document_state.dock.floating.get(index) {
                    let (window_width, window_height) = self.ui_state.window_size;
                    let center_x = panel.x + panel.width / 2.0;
                    let center_y = panel.y + panel.height / 4.0;

                    // Use real dock region footprints (or collapsed rails) as
                    // drop targets so users can dock panels more freely.
                    let left_zone_w = if self.document_state.dock.has_panels(PanelPosition::Left) {
                        if self.document_state.dock.is_collapsed(PanelPosition::Left) {
                            40.0
                        } else {
                            self.ui_state.left_width.max(80.0)
                        }
                    } else {
                        140.0
                    };

                    let right_zone_w = if self.document_state.dock.has_panels(PanelPosition::Right)
                    {
                        if self.document_state.dock.is_collapsed(PanelPosition::Right) {
                            40.0
                        } else {
                            self.ui_state.right_width.max(80.0)
                        }
                    } else {
                        140.0
                    };

                    let bottom_zone_h =
                        if self.document_state.dock.has_panels(PanelPosition::Bottom) {
                            if self.document_state.dock.is_collapsed(PanelPosition::Bottom) {
                                32.0
                            } else {
                                self.ui_state.bottom_height.max(60.0)
                            }
                        } else {
                            120.0
                        };

                    let target = if center_x <= left_zone_w {
                        Some(PanelPosition::Left)
                    } else if center_x >= window_width - right_zone_w {
                        Some(PanelPosition::Right)
                    } else if center_y >= window_height - bottom_zone_h {
                        Some(PanelPosition::Bottom)
                    } else {
                        None
                    };
                    crate::diagnostics::log_debug(format!(
                        "[dock-back] fp=({:.0},{:.0}) center=({center_x:.0},{center_y:.0}) win=({window_width:.0},{window_height:.0}) zones=(L:{left_zone_w:.0},R:{right_zone_w:.0},B:{bottom_zone_h:.0}) target={target:?}",
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
