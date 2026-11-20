//! The reset confirm (#76). It serves both footer buttons: the title and
//! the danger button follow the reset asked for (Reset All / Reset &
//! Renumber). The detail names the scope, and the banner says how undo
//! works, in the same words as the dialog footer.

use super::super::*;
use iced::widget::{column, container};
use iced::{Element, Length, Widget};
use signex_i18n::{ids, t};
use signex_widgets::text;

use super::super::widgets::{draggable_header, modal_title_header, os_title_bar_slot};
use super::controls::banner_style;
use super::{SP, TS};
use crate::styles::{DialogButtonKind, PillTone, dialog_body, dialog_button, dialog_footer};

impl Signex {
    pub(in crate::app::view) fn view_annotate_reset_confirm(&self) -> Element<'_, Message> {
        let dialog = self.view_annotate_reset_confirm_body_inner(true);
        self.wrap_dialog(
            crate::app::state::ModalId::AnnotateResetConfirm,
            dialog,
            super::super::sizes::ANNOTATE_RESET_CONFIRM,
        )
    }

    pub(in crate::app::view) fn view_annotate_reset_confirm_body(&self) -> Element<'_, Message> {
        self.view_annotate_reset_confirm_body_inner(false)
    }

    fn view_annotate_reset_confirm_body_inner(&self, draggable: bool) -> Element<'_, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = self.ui_state.i18n.as_ref();
        let ink = crate::styles::ti(tokens.text);
        let session = &self.ui_state.annotate_session;
        let mode = session.reset_mode;
        let renumber = session.resets_and_renumbers();
        let (title, action) = (session.reset_title(), session.reset_action());

        let header_content: Element<'_, Message> = modal_title_header(
            t!(i18n, title).into_string(),
            Message::Annotate(AnnotateMsg::CloseResetConfirm),
            tokens,
        );
        let header = if draggable {
            draggable_header(
                header_content,
                crate::app::state::ModalId::AnnotateResetConfirm,
                self.interaction_state.last_mouse_pos,
            )
        } else {
            os_title_bar_slot()
        };

        let yes_no = if renumber { "yes" } else { "no" };
        let scope = self.ui_state.annotate_preview.reset_scope(
            &self.ui_state.annotate_session.unticked,
            &self.ui_state.annotate_locked,
        );
        let detail = match scope {
            Some(scope) => t!(
                i18n,
                ids::DIALOG_ANNOTATE_RESET_DETAIL,
                "sheets" => scope.sheets,
                "symbols" => scope.symbols,
                "renumber" => yes_no
            ),
            None => t!(i18n, ids::DIALOG_ANNOTATE_RESET_DETAIL_LOADING, "renumber" => yes_no),
        };
        // AV-05: one statement of undo, the same as the dialog footer's.
        let undo_id = if self.annotate_undo_is_one_step() {
            ids::DIALOG_ANNOTATE_RESET_ONE_STEP
        } else {
            ids::DIALOG_ANNOTATE_RESET_PER_SHEET
        };
        let undo = t!(i18n, undo_id, "undo" => crate::keymap::shown_in("Mod+Z", i18n));
        let banner = container(text(undo).size(TS.caption).color(ink).width(Length::Fill))
            .padding([SP.s, SP.m])
            .width(Length::Fill)
            .style(banner_style(tokens, PillTone::Warn));

        let body = column![
            crate::styles::dialog_intro(
                crate::styles::DialogIcon::Warn,
                t!(i18n, ids::DIALOG_ANNOTATE_RESET_QUESTION).into_string(),
                Some(detail.into_string()),
                tokens,
            ),
            // Under the text, clear of the icon tile, as in the mock.
            container(banner).padding(iced::Padding {
                left: crate::styles::DIALOG_ICON_TILE + SP.m + SP.xs,
                ..iced::Padding::ZERO
            }),
        ]
        .spacing(SP.m);

        let dialog = container(
            column![
                header,
                dialog_body(body),
                dialog_footer(vec![
                    dialog_button(
                        t!(i18n, ids::DIALOG_BUTTON_CANCEL),
                        DialogButtonKind::Cancel,
                        Some(Message::Annotate(AnnotateMsg::CloseResetConfirm)),
                        tokens,
                    ),
                    dialog_button(
                        t!(i18n, action),
                        DialogButtonKind::Danger,
                        (!self.ui_state.annotate_runs.is_running())
                            .then_some(Message::Annotate(AnnotateMsg::Run(mode))),
                        tokens,
                    ),
                ]),
            ]
            .width(self.dialog_width(
                crate::app::state::ModalId::AnnotateResetConfirm,
                super::super::sizes::ANNOTATE_RESET_CONFIRM,
            )),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true);
        dialog.boxed()
    }
}
