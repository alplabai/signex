//! The Re-prefix to standard confirm (#1315): the changes about to be made,
//! the parts that stay, and Confirm / Cancel.

use super::*;
use iced::widget::{Space, column, container, row, text};
use iced::{Element, Length, Widget};

use super::super::widgets::{close_x_button, draggable_header, os_title_bar_slot, wrap_modal};
use super::super::{MODAL_HEADER_HEIGHT, MODAL_HEADER_PADDING, MODAL_HEADER_TITLE_SIZE};
use crate::styles::{
    DialogButtonKind, dialog_body, dialog_button, dialog_footer, dialog_list, dialog_message,
};

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
            return container(Space::new()).boxed();
        };
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);

        let header_content: Element<'_, Message> = container(
            row![
                text("Re-prefix to Standard")
                    .size(MODAL_HEADER_TITLE_SIZE)
                    .color(text_c),
                Space::new().width(Length::Fill),
                close_x_button(
                    Message::Annotate(AnnotateMsg::ReprefixCancelled),
                    self.ui_state.effective_theme_id(),
                    text_muted
                ),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding(MODAL_HEADER_PADDING)
        .height(MODAL_HEADER_HEIGHT)
        .style(crate::styles::modal_header_strip(tokens))
        .boxed();
        let header = if draggable {
            draggable_header(
                header_content,
                crate::app::state::ModalId::ReprefixConfirm,
                self.interaction_state.last_mouse_pos,
            )
        } else {
            os_title_bar_slot()
        };

        let mut lines: Vec<Element<'_, Message>> = confirm
            .change_lines
            .iter()
            .map(|line| text(line.clone()).size(TS.body).color(text_c).boxed())
            .collect();
        if confirm.more > 0 {
            lines.push(
                text(format!("+{} more", confirm.more))
                    .size(TS.body)
                    .color(text_muted)
                    .boxed(),
            );
        }
        let mut skips = column![].spacing(SP.xs);
        for line in &confirm.skip_lines {
            skips = skips.push(text(line.clone()).size(TS.body).color(text_muted));
        }

        let body = column![
            dialog_message(confirm.headline.clone(), tokens),
            container(dialog_list(lines, tokens)).height(Length::Fixed(220.0)),
            skips,
        ]
        .spacing(SP.m);
        let dialog = container(
            column![
                header,
                dialog_body(body),
                dialog_footer(vec![
                    dialog_button(
                        "Cancel",
                        DialogButtonKind::Secondary,
                        Some(Message::Annotate(AnnotateMsg::ReprefixCancelled)),
                        tokens,
                    ),
                    dialog_button(
                        &confirm.button_label,
                        DialogButtonKind::Primary,
                        Some(Message::Annotate(AnnotateMsg::ReprefixConfirmed)),
                        tokens,
                    ),
                ]),
            ]
            .width(self.dialog_width(crate::app::state::ModalId::ReprefixConfirm, 560.0)),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true);
        dialog.boxed()
    }
}
