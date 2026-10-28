//! The recovery prompt (#1164): a schematic was opened and a recovery copy
//! with edits its file lacks exists. Restore / Discard / Decide later; Esc is
//! Decide later (`OpenOverlays::rung`), so a stray key never deletes or
//! loads anything.

use super::*;
use iced::widget::{Space, column, container, row, text};
use iced::{Element, Length};

use super::widgets::{close_x_button, primary_button, secondary_button, wrap_modal};
use super::{MODAL_HEADER_HEIGHT, MODAL_HEADER_PADDING, MODAL_HEADER_TITLE_SIZE};
use crate::autosave::{AutosaveMsg, RecoveryChoice, RecoveryOffer, describe_age};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

impl Signex {
    pub(in crate::app::view) fn view_autosave_recovery(
        &self,
        offer: &RecoveryOffer,
    ) -> Element<'_, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);
        let later = Message::Autosave(AutosaveMsg::Recovery(RecoveryChoice::Later));

        let header = container(
            row![
                text("Recover Unsaved Edits")
                    .size(MODAL_HEADER_TITLE_SIZE)
                    .color(text_c),
                Space::new().width(Length::Fill),
                close_x_button(later.clone(), self.ui_state.theme_id, text_muted),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding(MODAL_HEADER_PADDING)
        .height(MODAL_HEADER_HEIGHT)
        .style(crate::styles::modal_header_strip(tokens));

        let name = offer
            .path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| offer.path.display().to_string());
        let mut body = column![
            text(format!(
                "Signex found a recovery copy of {name} written {} that differs from the \
                 saved file.",
                describe_age(offer.age_secs)
            ))
            .size(TS.label)
            .color(text_c),
        ]
        .spacing(SP.m);
        if offer.older_than_file {
            body = body.push(
                text(format!(
                    "{name} was saved after this copy was written. Restore replaces that \
                     newer saved work in the editor with the older copy."
                ))
                .size(TS.label)
                .color(crate::styles::ti(tokens.warning)),
            );
        }
        body = body.push(
            text(
                "Restore loads the copy as unsaved edits; nothing is written to the file until \
                 you save. Restoring replaces the open document and cannot be undone with \
                 Ctrl+Z. Discard deletes the copy. Decide later keeps it for the next time \
                 this file is opened, and pauses autosave for this document for the rest of the \
                 session.",
            )
            .size(TS.body)
            .color(text_muted),
        );

        let dialog = container(
            column![
                header,
                container(body).padding([14, 14]),
                container(
                    row![
                        Space::new().width(Length::Fill),
                        secondary_button("Decide later", later, text_c, border_c),
                        Space::new().width(8),
                        secondary_button(
                            "Discard",
                            Message::Autosave(AutosaveMsg::Recovery(RecoveryChoice::Discard)),
                            text_c,
                            border_c,
                        ),
                        Space::new().width(8),
                        primary_button(
                            "Restore",
                            Some(Message::Autosave(AutosaveMsg::Recovery(
                                RecoveryChoice::Restore,
                            ))),
                            border_c,
                        ),
                    ]
                    .align_y(iced::Alignment::Center),
                )
                .padding([14, 14]),
            ]
            .width(520),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true);
        wrap_modal(
            dialog.into(),
            (0.0, 0.0),
            self.ui_state.window_size,
            (520.0, 250.0),
        )
    }
}
