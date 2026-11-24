//! The reset confirm (#76). It serves both footer buttons: the title and
//! the danger button follow the reset asked for (Reset All / Reset &
//! Renumber). The detail names the scope, and the warn banner says how undo
//! works, in the same words as the dialog footer, with the shortcut as a
//! key cap.

use super::super::*;
use iced::widget::text::Span;
use iced::widget::{column, container};
use iced::{Element, Length, Widget};
use signex_i18n::{Localized, MessageId, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{rich_text, span};

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
        let shortcut = crate::keymap::shown_in("Mod+Z", i18n);
        let undo = t!(i18n, undo_id, "undo" => shortcut.clone());
        let banner = container(undo_caveat(undo, &shortcut, ink, tokens))
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

        let run = (!self.ui_state.annotate_runs.is_running())
            .then_some(Message::Annotate(AnnotateMsg::Run(mode)));
        let dialog = container(
            column![
                header,
                dialog_body(body),
                dialog_footer(
                    footer_buttons(action)
                        .into_iter()
                        .map(|(label, kind, does)| {
                            let on_press = match does {
                                FooterAction::Reset => run.clone(),
                                FooterAction::Close => {
                                    Some(Message::Annotate(AnnotateMsg::CloseResetConfirm))
                                }
                            };
                            dialog_button(t!(i18n, label), kind, on_press, tokens)
                        })
                        .collect()
                ),
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

/// The confirm's footer: Cancel, then the reset asked for. There is no
/// "Save all first": a reset writes no file and one undo restores every
/// sheet, so saving first protects nothing (mockup row 76, Caner
/// 2026-11-24).
fn footer_buttons(action: MessageId) -> [(MessageId, DialogButtonKind, FooterAction); 2] {
    [
        (
            ids::DIALOG_BUTTON_CANCEL,
            DialogButtonKind::Cancel,
            FooterAction::Close,
        ),
        (action, DialogButtonKind::Danger, FooterAction::Reset),
    ]
}

/// What a footer button of the confirm does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FooterAction {
    /// Closes the confirm and resets nothing.
    Close,
    /// Runs the reset asked for.
    Reset,
}

/// The undo caveat as one wrapping paragraph, its shortcut drawn as a key
/// cap where the sentence names it (mockup row 76). A translation that
/// leaves the shortcut out shows as plain words.
fn undo_caveat<'a>(
    undo: Localized,
    shortcut: &str,
    ink: iced::Color,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let spans = match undo.split_around(shortcut) {
        Some((before, cap, after)) => vec![
            span::<()>(before).color(ink),
            key_cap_span(cap, tokens),
            span::<()>(after).color(ink),
        ],
        None => vec![span::<()>(undo).color(ink)],
    };
    rich_text::<(), Message, iced::Theme>(spans)
        .size(TS.caption)
        .wrapping(iced::widget::text::Wrapping::Word)
        .width(Length::Fill)
        .boxed()
}

/// A shortcut inside a sentence on the key-cap face (`styles::key_cap`, the
/// face of every key cap) in the mono font. The padding only grows the
/// face, so it stays inside the spaces around the shortcut.
fn key_cap_span<'a>(cap: Localized, tokens: &ThemeTokens) -> Span<'a, ()> {
    // The key-cap style reads only the tokens, not the theme.
    let face = crate::styles::key_cap(tokens)(&iced::Theme::Light);
    span::<()>(cap)
        .font(crate::fonts::iced_font_for_family(
            crate::fonts::CHROME_MONO_FONT,
        ))
        .color_maybe(face.text_color)
        .background_maybe(face.background)
        .border(face.border)
        .padding([TS.caption * 0.1, TS.caption * 0.25])
}

#[cfg(test)]
mod tests {
    use signex_i18n::{I18n, ids, t};

    use crate::app::Signex;

    /// Mockup row 76: both undo caveats name the shortcut where the key cap
    /// goes, so neither falls back to plain words.
    #[test]
    fn both_undo_caveats_carry_the_shortcut_as_a_key_cap() {
        let i18n = I18n::en_us();
        let shortcut = crate::keymap::shown_in("Mod+Z", &i18n);
        for id in [
            ids::DIALOG_ANNOTATE_RESET_ONE_STEP,
            ids::DIALOG_ANNOTATE_RESET_PER_SHEET,
        ] {
            let undo = t!(i18n, id, "undo" => shortcut.clone());
            let (_, cap, _) = undo.split_around(&shortcut).expect("the shortcut");
            assert_eq!(cap.into_string(), shortcut, "{id:?}");
        }
    }

    /// Mockup row 76 (Caner 2026-11-24): the confirm offers Cancel and the
    /// reset only, with no "Save all first", for both resets.
    #[test]
    fn the_reset_confirm_offers_cancel_and_the_reset_only() {
        use super::{FooterAction, footer_buttons};
        use crate::styles::DialogButtonKind;
        for action in [
            ids::DIALOG_ANNOTATE_RESET_ALL,
            ids::DIALOG_ANNOTATE_RESET_RENUMBER,
        ] {
            assert_eq!(
                footer_buttons(action),
                [
                    (
                        ids::DIALOG_BUTTON_CANCEL,
                        DialogButtonKind::Cancel,
                        FooterAction::Close
                    ),
                    (action, DialogButtonKind::Danger, FooterAction::Reset),
                ]
            );
        }
    }

    /// The confirm with its key-cap banner builds.
    #[test]
    fn the_reset_confirm_builds() {
        let (app, _task) = Signex::new();
        let _ = app.view_annotate_reset_confirm_body();
    }
}
