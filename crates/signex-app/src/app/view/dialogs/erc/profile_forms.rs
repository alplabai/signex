//! The ERC dialog's profile forms: the "Update from profile" diff and the
//! "Save as profile" name and version form. Both open over the rule body,
//! above the footer (#2413 keeps them there).

use iced::widget::{Space, column, container, row};
use iced::{Element, Length, Widget};
use signex_i18n::{ids, t};
use signex_widgets::{raw, text, text_input};

use crate::app::{ErcMsg, Message, Signex};
use crate::styles::{DialogButtonKind, dialog_button};

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// Overwriting a stored profile replaces its rules, so the button is a danger.
const PROFILE_OVERWRITE_KIND: DialogButtonKind = DialogButtonKind::Danger;

impl Signex {
    /// The diff an "Update from profile" would apply, with Apply / Cancel.
    /// Empty space when no update is pending.
    pub(super) fn view_profile_update_panel(
        &self,
        tokens: &signex_types::theme::ThemeTokens,
    ) -> Element<'_, Message> {
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let Some((name, lines)) = self.pending_profile_update_lines() else {
            return Space::new().height(0).boxed();
        };
        let i18n = &*self.ui_state.i18n;
        let heading = if lines.is_empty() {
            t!(i18n, ids::DIALOG_ERC_PROFILE_HASH_ONLY, "name" => name.as_str())
        } else {
            t!(
                i18n,
                ids::DIALOG_ERC_PROFILE_UPDATE_HEADING,
                "name" => name.as_str(),
                "count" => lines.len() as i64
            )
        };
        let mut list = column![text(heading).size(TS.body).color(text_c)].spacing(SP.xs);
        for line in lines {
            // raw: one rule change per line, built by the profile diff (#2061).
            list = list.push(text(raw(line)).size(TS.caption).color(text_muted).boxed());
        }
        let buttons = row![
            dialog_button(
                t!(i18n, ids::DIALOG_ERC_APPLY_UPDATE),
                DialogButtonKind::Primary,
                Some(Message::Erc(ErcMsg::ProfileUpdateConfirmed)),
                tokens,
            ),
            dialog_button(
                t!(i18n, ids::DIALOG_BUTTON_CANCEL),
                DialogButtonKind::Cancel,
                Some(Message::Erc(ErcMsg::ProfileUpdateCancelled)),
                tokens,
            ),
        ]
        .spacing(SP.s);
        container(
            column![
                container(iced::widget::scrollable(list)).height(Length::Fit.max(120)),
                buttons
            ]
            .spacing(6),
        )
        .padding([8, 14])
        .width(Length::Fill)
        .boxed()
    }
}

impl Signex {
    /// The "Save as profile" form: name, version, validation, and the
    /// overwrite question. Empty space when the dialog is closed.
    pub(super) fn view_profile_save_panel(
        &self,
        tokens: &signex_types::theme::ThemeTokens,
    ) -> Element<'_, Message> {
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let Some(dialog) = self.ui_state.save_profile_dialog.as_ref() else {
            return Space::new().height(0).boxed();
        };
        let i18n = &*self.ui_state.i18n;
        let mut panel = column![
            text(t!(i18n, ids::DIALOG_ERC_PROFILE_SAVE_TITLE))
                .size(TS.body)
                .color(text_c)
        ]
        .spacing(6);
        if dialog.confirm_overwrite {
            let name = dialog.name.trim();
            panel = panel
                .push(
                    text(t!(i18n, ids::DIALOG_ERC_PROFILE_OVERWRITE_NOTE, "name" => name))
                        .size(TS.caption)
                        .color(text_muted)
                        .boxed(),
                )
                .push(
                    row![
                        dialog_button(
                            t!(i18n, ids::DIALOG_OVERWRITE),
                            PROFILE_OVERWRITE_KIND,
                            (!dialog.busy)
                                .then_some(Message::Erc(ErcMsg::ProfileSaveOverwriteConfirmed)),
                            tokens,
                        ),
                        dialog_button(
                            t!(i18n, ids::DIALOG_BUTTON_BACK),
                            DialogButtonKind::Secondary,
                            Some(Message::Erc(ErcMsg::ProfileSaveCancelled)),
                            tokens,
                        ),
                    ]
                    .spacing(SP.s)
                    .boxed(),
                );
        } else {
            let submit = || Message::Erc(ErcMsg::ProfileSaveSubmitted);
            let editable = !dialog.busy;
            let mut name_input = text_input(
                t!(i18n, ids::DIALOG_ERC_PROFILE_NAME_PLACEHOLDER),
                &dialog.name,
            )
            .style(crate::panels::form_look::text_input_style)
            .size(TS.body);
            let mut version_input = text_input(
                t!(i18n, ids::DIALOG_ERC_PROFILE_VERSION_PLACEHOLDER),
                &dialog.version,
            )
            .style(crate::panels::form_look::text_input_style)
            .size(TS.body)
            .width(90);
            if editable {
                name_input = name_input
                    .on_input(|v| Message::Erc(ErcMsg::ProfileSaveNameChanged(v)))
                    .on_submit(submit());
                version_input = version_input
                    .on_input(|v| Message::Erc(ErcMsg::ProfileSaveVersionChanged(v)))
                    .on_submit(submit());
            }
            panel = panel.push(
                row![
                    text(t!(i18n, ids::DIALOG_ERC_PROFILE_NAME))
                        .size(TS.caption)
                        .color(text_muted),
                    name_input,
                    text(t!(i18n, ids::DIALOG_ERC_PROFILE_VERSION))
                        .size(TS.caption)
                        .color(text_muted),
                    version_input,
                ]
                .spacing(SP.m)
                .align_y(iced::Alignment::Center)
                .boxed(),
            );
            if let Some(error) = &dialog.error {
                // raw: the save-as-profile error is set in English by its
                // handler (#2061).
                panel = panel.push(
                    text(raw(error.clone()))
                        .size(TS.caption)
                        .color(crate::styles::tokens::danger(tokens))
                        .boxed(),
                );
            }
            panel = panel.push(
                row![
                    dialog_button(
                        t!(i18n, ids::DIALOG_BUTTON_SAVE),
                        DialogButtonKind::Primary,
                        editable.then(submit),
                        tokens,
                    ),
                    dialog_button(
                        t!(i18n, ids::DIALOG_BUTTON_CANCEL),
                        DialogButtonKind::Cancel,
                        Some(Message::Erc(ErcMsg::ProfileSaveCancelled)),
                        tokens,
                    ),
                ]
                .spacing(SP.s)
                .boxed(),
            );
        }
        container(panel)
            .padding([8, 14])
            .width(Length::Fill)
            .boxed()
    }
}

#[cfg(test)]
mod overwrite_tests {
    use super::*;

    #[test]
    fn overwriting_a_profile_is_a_danger_button() {
        assert_eq!(PROFILE_OVERWRITE_KIND, DialogButtonKind::Danger);
    }
}
