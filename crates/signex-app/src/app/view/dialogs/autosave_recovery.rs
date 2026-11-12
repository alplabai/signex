//! The recovery prompt (#1164): a schematic was opened and a recovery copy
//! with edits its file lacks exists. Restore / Discard / Decide later; Esc is
//! Decide later (`OpenOverlays::rung`), so a stray key never deletes or
//! loads anything.

use super::*;
use iced::widget::{Space, column, container, row};
use iced::{Element, Length, Widget};
use signex_widgets::{raw, text};

use super::widgets::modal_title_header;
use crate::autosave::{AutosaveMsg, RecoveryChoice, RecoveryOffer, describe_age};
use crate::styles::{
    DialogButtonKind, dialog_body, dialog_button, dialog_footer_split, dialog_hint, dialog_message,
};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

impl Signex {
    pub(in crate::app::view) fn view_autosave_recovery(
        &self,
        offer: &RecoveryOffer,
    ) -> Element<'_, Message> {
        self.wrap_centred_dialog(
            self.autosave_recovery_card(offer, false),
            super::sizes::AUTOSAVE_RECOVERY,
        )
    }

    /// The prompt as the content of its own OS window (no header, fills it).
    pub(in crate::app::view) fn view_autosave_recovery_window(
        &self,
        offer: &RecoveryOffer,
    ) -> Element<'_, Message> {
        self.autosave_recovery_card(offer, true)
    }

    fn autosave_recovery_card(
        &self,
        offer: &RecoveryOffer,
        windowed: bool,
    ) -> Element<'_, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let later = Message::Autosave(AutosaveMsg::Recovery(RecoveryChoice::Later));

        let header_content = modal_title_header("Recover Unsaved Edits", later.clone(), tokens);
        let header: Element<'_, Message> = if windowed {
            super::widgets::os_title_bar_slot()
        } else {
            header_content.boxed()
        };

        // A Component Preview row has no file: the prompt names the part.
        let row_name = self
            .library
            .editors
            .iter()
            .find(|(address, _)| address.synthetic_tab_path() == offer.path)
            .map(|(_, editor)| editor.row.internal_pn.as_str().to_string());
        let name = row_name.unwrap_or_else(|| {
            offer
                .path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| offer.path.display().to_string())
        });
        let mut body = column![dialog_message(
            format!(
                "Signex found a recovery copy of {name} written {} that differs from the \
                 saved file.",
                describe_age(offer.age_secs)
            ),
            tokens,
        )]
        .spacing(SP.m);
        if offer.older_than_file {
            body = body.push(
                text(raw(format!(
                    "{name} was saved after this copy was written. Restore replaces that \
                     newer saved work in the editor with the older copy."
                )))
                .size(TS.body)
                .color(crate::styles::ti(tokens.warning))
                .boxed(),
            );
        }
        body = body.push(dialog_hint(
            format!(
                "Restore loads the copy as unsaved edits; nothing is written to the file until \
                 you save. Restoring replaces the open document and cannot be undone with \
                 {}. Discard deletes the copy. Decide later keeps it for the next time \
                 this file is opened, and pauses autosave for this document for the rest of the \
                 session.",
                crate::keymap::shown("Mod+Z")
            ),
            tokens,
        ));

        let restore = dialog_button(
            "Restore",
            DialogButtonKind::Primary,
            Some(Message::Autosave(AutosaveMsg::Recovery(
                RecoveryChoice::Restore,
            ))),
            tokens,
        );
        let discard = dialog_button(
            "Discard",
            DialogButtonKind::Danger,
            Some(Message::Autosave(AutosaveMsg::Recovery(
                RecoveryChoice::Discard,
            ))),
            tokens,
        );
        // Restore and Discard at the left, a quiet Decide later at the
        // right; Esc keeps meaning Decide later.
        let footer = dialog_footer_split(
            vec![restore, discard],
            vec![dialog_button(
                "Decide later",
                DialogButtonKind::Ghost,
                Some(later),
                tokens,
            )],
        );
        let body: Element<'_, Message> = {
            let r = crate::styles::tokens::resolve(tokens);
            let card = container(
                row![
                    text(signex_i18n::UserText::new(name.clone()))
                        .size(TS.dense)
                        .font(crate::fonts::iced_font_for_family(
                            crate::fonts::CHROME_MONO_FONT,
                        ))
                        .color(crate::styles::ti(tokens.text)),
                    Space::new().width(Length::Fill),
                    text(signex_i18n::t!(
                        self.ui_state.i18n,
                        signex_i18n::ids::DIALOG_AUTOSAVE_RECOVERY_AUTOSAVED_AGE,
                        "age" => describe_age(offer.age_secs)
                    ))
                    .size(TS.label)
                    .color(r.ink3),
                ]
                .align_y(iced::Alignment::Center),
            )
            .padding([SP.m, SP.l])
            .width(Length::Fill)
            .style(crate::styles::card(tokens));
            row![
                super::graphite::recovery_icon_tile(tokens),
                column![body.boxed(), card.boxed()]
                    .spacing(SP.l)
                    .width(Length::Fill)
                    .boxed()
            ]
            .spacing(SP.l)
            .boxed()
        };
        let dialog = container(
            column![header, dialog_body(body), footer].width(if windowed {
                Length::Fill
            } else {
                Length::Fixed(super::widgets::modal_card_width(
                    self.ui_state.window_size,
                    super::sizes::AUTOSAVE_RECOVERY.0,
                ))
            }),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true);
        if windowed {
            dialog.width(Length::Fill).height(Length::Fill).boxed()
        } else {
            dialog.boxed()
        }
    }
}
