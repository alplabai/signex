//! The Esc rungs of the right-click menus and popovers: Esc closes the
//! menu, nothing behind it (#2349). The dock-tab, Components-row, Start-row
//! and board layer tab menus answer in `dispatch/escape.rs` before the
//! ladder runs. Moved out of `subscription.rs` to keep it under the file
//! size cap.

use super::*;

impl OpenOverlays {
    /// The rung of menu `id`; `None` while it is closed, or for an id that
    /// is not one of these menus.
    pub(super) fn menu_rung(&self, id: OverlayId) -> Option<Message> {
        match id {
            OverlayId::ContextMenu => self
                .canvas_context_menu_open
                .then(|| Message::ContextMenu(ContextMenuMsg::Close)),
            OverlayId::TabContextMenu => self
                .tab_context_menu_open
                .then(|| Message::ContextMenu(ContextMenuMsg::CloseTab)),
            OverlayId::ProjectTreeContextMenu => self
                .project_tree_context_menu_open
                .then(|| Message::ContextMenu(ContextMenuMsg::CloseProjectTree)),
            // The Properties part ⋯ menu (#3063).
            OverlayId::PartMenu => self.part_menu.clone(),
            OverlayId::GridPicker => self
                .grid_picker_open
                .then_some(Message::Ui(UiMsg::GridPickerClose)),
            OverlayId::ZoomPresets => self
                .zoom_presets_open
                .then_some(Message::Ui(UiMsg::ZoomPresetsDismissed)),
            OverlayId::GridSnapPopover => self
                .grid_snap_popover_open
                .then_some(Message::Overlay(OverlayMsg::GridSnapPopoverDismissed)),
            OverlayId::BoardStatusPopover => self.board_status_popover_open.then(|| {
                Message::Ui(UiMsg::StatusBar(StatusBarRequest::BoardPopover(
                    crate::status_bar::BoardPopoverEvent::Dismissed,
                )))
            }),
            _ => None,
        }
    }
}
