//! Right-click menu of a Components panel row: Place or open the component
//! for editing.

use super::*;

use iced::Widget as _;

use super::items::dd_msg;
use crate::library::messages::LibraryMessage;
use crate::library::state::ComponentsRowAction;
use signex_widgets::active_bar_dropdown::DropdownEntry;

/// Menu rows: pure data, so the labels and messages are tested without a
/// renderer.
pub(super) fn components_row_menu_entries() -> Vec<DropdownEntry<Message>> {
    let pick = |action| Message::Library(LibraryMessage::ComponentsPanelRowMenuPicked(action));
    vec![
        dd_msg(None, "Place", "", pick(ComponentsRowAction::Place)),
        dd_msg(None, "Edit Component", "", pick(ComponentsRowAction::Open)),
    ]
}

impl Signex {
    /// The open row menu with its click-outside backdrop, clamped into the
    /// window; empty when no menu is open.
    pub(in crate::app::view) fn components_row_context_menu_overlay(
        &self,
        window_id: iced::window::Id,
    ) -> Vec<Element<'_, Message>> {
        use iced::widget::{column, row};

        let Some(menu_ctx) = self.library.components_panel.row_menu.as_ref() else {
            return Vec::new();
        };
        if !self.overlay_belongs_to(menu_ctx.window, window_id) {
            return Vec::new();
        }
        let entries = components_row_menu_entries();
        let entries_len = entries.len();
        let tokens = &self.document_state.panel_ctx.tokens;
        let menu = signex_widgets::active_bar_dropdown::view(
            entries,
            tokens,
            Some(Self::CONTEXT_MENU_WIDTH),
        );
        let (x, y) = super::clamp_menu_origin(
            (menu_ctx.x, menu_ctx.y),
            (Self::CONTEXT_MENU_WIDTH, super::menu_height(entries_len)),
            self.ui_state.size_of_window(window_id),
        );
        let layers = vec![
            Self::dismiss_layer(Message::Library(
                LibraryMessage::ComponentsPanelRowMenuDismissed,
            )),
            column![
                iced::widget::Space::new().height(y),
                row![
                    iced::widget::Space::new().width(x),
                    menu,
                    iced::widget::Space::new().width(Length::Fill),
                ]
                .width(Length::Fill),
            ]
            .boxed(),
        ];
        match menu_ctx.window {
            None => layers,
            // A pick in an undocked window acts on that window's sheet.
            Some(window) => layers
                .into_iter()
                .map(|layer| {
                    layer
                        .map(move |m| {
                            crate::app::dispatch::window_action::scope_row_menu_message(window, m)
                        })
                        .boxed()
                })
                .collect(),
        }
    }
}

impl Signex {
    /// The part number of the row being dragged, as a small label beside the
    /// pointer; empty until the press has become a drag.
    pub(in crate::app::view) fn components_row_drag_label_overlay(
        &self,
        window_id: iced::window::Id,
    ) -> Vec<Element<'_, Message>> {
        use iced::widget::{column, row};

        const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
        const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;
        let Some(drag) = self
            .library
            .components_panel
            .drag
            .as_ref()
            .filter(|d| d.started)
        else {
            return Vec::new();
        };
        if !self.overlay_belongs_to(drag.window, window_id) {
            return Vec::new();
        }
        // Over the canvas the part itself follows the pointer.
        let (px, py) = self.interaction_state.last_mouse_pos;
        if drag.armed
            && self.interaction_state.placement.is_some()
            && self.canvas_bounds().contains(iced::Point::new(px, py))
        {
            return Vec::new();
        }
        let key = &drag.key;
        let Some(pn) = self
            .library
            .library_at(&key.library_path)
            .and_then(|lib| lib.tables.get(&key.table))
            .and_then(|rows| rows.iter().find(|r| r.row_id == key.row_id.0))
            .map(|r| r.internal_pn.to_string())
        else {
            return Vec::new();
        };
        let tokens = &self.document_state.panel_ctx.tokens;
        let label = container(
            iced::widget::text(pn)
                .size(TS.caption)
                .color(signex_widgets::theme_ext::text_primary(tokens)),
        )
        .padding([SP.xs, SP.s])
        .style(crate::styles::context_menu(tokens));
        // Offset so the label never sits under the pointer.
        let (x, y) = self.interaction_state.last_mouse_pos;
        vec![
            column![
                iced::widget::Space::new().height(y + SP.l),
                row![iced::widget::Space::new().width(x + SP.l), label],
            ]
            .boxed(),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_menu_offers_place_and_edit_component() {
        let labels: Vec<String> = components_row_menu_entries()
            .into_iter()
            .filter_map(|e| match e {
                DropdownEntry::Item(item) => Some(item.label.to_string()),
                _ => None,
            })
            .collect();
        assert_eq!(labels, ["Place", "Edit Component"]);
    }
}
