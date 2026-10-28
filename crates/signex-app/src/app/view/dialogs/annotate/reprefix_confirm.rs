//! The Re-prefix to standard confirm (#1315): the changes about to be made,
//! the parts that stay, and Confirm / Cancel.

use super::*;
use iced::widget::{Space, column, container, row, scrollable, text};
use iced::{Element, Length};

use super::super::widgets::{
    close_x_button, draggable_header, os_title_bar_slot, primary_button, secondary_button,
    wrap_modal,
};
use super::super::{MODAL_HEADER_HEIGHT, MODAL_HEADER_PADDING, MODAL_HEADER_TITLE_SIZE};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

impl Signex {
    pub(in crate::app::view) fn view_reprefix_confirm(&self) -> Element<'_, Message> {
        let dialog = self.view_reprefix_confirm_body_inner(true);
        let offset = self
            .ui_state
            .modal_offsets
            .get(&crate::app::state::ModalId::ReprefixConfirm)
            .copied()
            .unwrap_or((0.0, 0.0));
        wrap_modal(dialog, offset, self.ui_state.window_size, (560.0, 520.0))
    }

    pub(in crate::app::view) fn view_reprefix_confirm_body(&self) -> Element<'_, Message> {
        self.view_reprefix_confirm_body_inner(false)
    }

    fn view_reprefix_confirm_body_inner(&self, draggable: bool) -> Element<'_, Message> {
        let Some(confirm) = &self.ui_state.reprefix_confirm else {
            return container(Space::new()).into();
        };
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);

        let header_content: Element<'_, Message> = container(
            row![
                text("Re-prefix to Standard")
                    .size(MODAL_HEADER_TITLE_SIZE)
                    .color(text_c),
                Space::new().width(Length::Fill),
                close_x_button(
                    Message::Annotate(AnnotateMsg::ReprefixCancelled),
                    self.ui_state.theme_id,
                    text_muted
                ),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding(MODAL_HEADER_PADDING)
        .height(MODAL_HEADER_HEIGHT)
        .style(crate::styles::modal_header_strip(tokens))
        .into();
        let header = if draggable {
            draggable_header(
                header_content,
                crate::app::state::ModalId::ReprefixConfirm,
                self.interaction_state.last_mouse_pos,
            )
        } else {
            os_title_bar_slot()
        };

        let mut lines = column![].spacing(SP.xs);
        for line in &confirm.change_lines {
            lines = lines.push(text(line.clone()).size(TS.body).color(text_c));
        }
        if confirm.more > 0 {
            lines = lines.push(
                text(format!("+{} more", confirm.more))
                    .size(TS.body)
                    .color(text_muted),
            );
        }
        let mut skips = column![].spacing(SP.xs);
        for line in &confirm.skip_lines {
            skips = skips.push(text(line.clone()).size(TS.body).color(text_muted));
        }

        let dialog = container(
            column![
                header,
                container(
                    text(confirm.headline.clone())
                        .size(TS.body)
                        .color(text_muted)
                )
                .padding([14, 14]),
                container(
                    scrollable(lines)
                        .height(Length::Fixed(220.0))
                        .width(Length::Fill)
                )
                .padding([0, 14]),
                container(skips).padding([10, 14]),
                container(
                    row![
                        Space::new().width(Length::Fill),
                        secondary_button(
                            "Cancel",
                            Message::Annotate(AnnotateMsg::ReprefixCancelled),
                            text_c,
                            border_c
                        ),
                        Space::new().width(8),
                        primary_button(
                            &confirm.button_label,
                            Some(Message::Annotate(AnnotateMsg::ReprefixConfirmed)),
                            border_c,
                        ),
                    ]
                    .align_y(iced::Alignment::Center),
                )
                .padding([10, 14]),
            ]
            .width(self.dialog_width(crate::app::state::ModalId::ReprefixConfirm, 560.0)),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true);
        dialog.into()
    }
}
