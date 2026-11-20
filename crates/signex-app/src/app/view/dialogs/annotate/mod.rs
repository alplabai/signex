//! Annotate Schematics modal (#75): the options column (order, sheets,
//! fixed options) beside the proposed change list, and the footer. The
//! reset confirm (#76) and the Re-prefix confirm (#77) sit beside it.
//!
//! These are methods of the same `Signex` view impl, split across sibling
//! files by concern.

use super::*;
use iced::widget::{Space, column, container, row};
use iced::{Background, Element, Length, Theme, Widget};
use signex_i18n::{Localized, ids, t};
use signex_widgets::text;

use super::widgets::{draggable_header, fit_modal_size, os_title_bar_slot};
use crate::styles::{DialogButtonKind, dialog_button, dialog_footer_split};

mod change_list;
mod controls;
mod options;
mod reprefix_confirm;
mod reset_confirm;

use super::graphite as gr;

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// Width of the options column: the mock's 372 px at the 12 px body.
const OPTIONS_COL_W: f32 = TS.body * 31.0;
/// Padding inside each column: the mock's 14 px.
const COL_PAD: f32 = TS.body * 14.0 / 12.0;

impl Signex {
    pub(in crate::app::view) fn view_annotate_dialog(&self) -> Element<'_, Message> {
        let dialog = self.view_annotate_dialog_body_inner(true);
        self.wrap_dialog(
            crate::app::state::ModalId::AnnotateDialog,
            dialog,
            self.annotate_modal_size(),
        )
    }

    /// The in-window Annotate modal: 1100 x 760 where the window allows,
    /// capped to the window otherwise (#1227).
    pub(in crate::app::view) fn annotate_modal_size(&self) -> (f32, f32) {
        fit_modal_size(self.ui_state.window_size, (1100.0, 760.0))
    }

    /// Detached-window flavour — just the body, no backdrop, no drag
    /// handler on the header (the OS window chrome owns that).
    pub(in crate::app::view) fn view_annotate_dialog_body(&self) -> Element<'_, Message> {
        self.view_annotate_dialog_body_inner(false)
    }

    /// Whether an annotate run of the active project is one undo step for
    /// every sheet (the project's Annotate undo choice, else Preferences).
    fn annotate_undo_is_one_step(&self) -> bool {
        self.annotate_undo_of(self.document_state.active_path.as_deref())
            == crate::annotate_undo::AnnotateUndo::Project
    }

    fn view_annotate_dialog_body_inner(&self, draggable: bool) -> Element<'_, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let r = crate::styles::tokens::resolve(tokens);
        let border_c = crate::styles::ti(tokens.border);

        // ── Header (draggable when in-window; OS-window-drag when
        // detached) ──
        let header_content: Element<'_, Message> = gr::titled_header(
            self.ui_state
                .i18n
                .message_plain(ids::DIALOG_TITLE_ANNOTATE, &[]),
            self.detached_modal_subtitle(crate::app::state::ModalId::AnnotateDialog),
            Message::Annotate(AnnotateMsg::CloseDialog),
            tokens,
        );
        let header = if draggable {
            draggable_header(
                header_content,
                crate::app::state::ModalId::AnnotateDialog,
                self.interaction_state.last_mouse_pos,
            )
        } else {
            os_title_bar_slot()
        };

        // The options column sits on the chrome fill, ruled off the list.
        let chrome = r.sunken;
        let body_row = row![
            container(self.annotate_options_column())
                .padding(COL_PAD)
                .width(Length::Fixed(OPTIONS_COL_W))
                .height(Length::Fill)
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(chrome)),
                    ..container::Style::default()
                }),
            container(Space::new().width(1).height(Length::Fill)).style(move |_: &Theme| {
                container::Style {
                    background: Some(Background::Color(border_c)),
                    ..container::Style::default()
                }
            }),
            container(self.annotate_change_list_column())
                .padding(COL_PAD)
                .width(Length::Fill)
                .height(Length::Fill),
        ]
        .spacing(0)
        .height(Length::Fill);

        // In-window: the window-capped modal. Detached: fill the OS window.
        // The body fills what the header and footer leave, with the left
        // column and the proposed list scrolling, so the footer buttons
        // stay reachable at any scale (#1227).
        let (modal_w, modal_h): (Length, Length) = if draggable {
            let (w, h) = self.annotate_modal_size();
            (Length::Fixed(w), Length::Fixed(h))
        } else {
            (Length::Fill, Length::Fill)
        };
        let dialog = container(
            column![header, body_row, self.annotate_footer()]
                .width(modal_w)
                .height(modal_h),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true);

        dialog.boxed()
    }

    /// The footer: both resets (each through its confirm) at the left; the
    /// undo caption, Close and Accept Changes with its count at the right.
    fn annotate_footer(&self) -> Element<'_, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = self.ui_state.i18n.as_ref();
        let muted = crate::styles::tokens::resolve(tokens).ink4;
        // A run reads, numbers and writes off the UI thread; the buttons that
        // start another stay off until it ends (#1753).
        let busy = self.ui_state.annotate_runs.is_running();
        let loading = self.ui_state.annotate_preview.loading;
        let secondary = |label: Localized, message: Option<Message>| {
            dialog_button(label, DialogButtonKind::Secondary, message, tokens)
        };
        let reset = |mode| (!busy).then_some(Message::Annotate(AnnotateMsg::ResetPressed(mode)));

        let caption = if busy {
            t!(i18n, ids::DIALOG_ANNOTATE_RUNNING)
        } else {
            let id = if self.annotate_undo_is_one_step() {
                ids::DIALOG_ANNOTATE_UNDO_CAPTION
            } else {
                ids::DIALOG_ANNOTATE_UNDO_CAPTION_PER_SHEET
            };
            t!(i18n, id, "undo" => crate::keymap::shown_in("Mod+Z", i18n))
        };
        let changes =
            crate::app::handlers::erc::change_count(&self.ui_state.annotate_preview.entries);
        let accept = (changes > 0 && !loading && !busy).then_some(Message::Annotate(
            AnnotateMsg::Run(signex_engine::AnnotateMode::Incremental),
        ));
        dialog_footer_split(
            vec![
                secondary(
                    t!(i18n, ids::DIALOG_ANNOTATE_RESET_ALL_ELLIPSIS),
                    reset(signex_engine::AnnotateMode::ResetOnly),
                ),
                secondary(
                    t!(i18n, ids::DIALOG_ANNOTATE_RESET_RENUMBER_ELLIPSIS),
                    reset(signex_engine::AnnotateMode::ResetAndRenumber),
                ),
            ],
            vec![
                text(caption)
                    .size(TS.caption)
                    .color(muted)
                    .wrapping(iced::widget::text::Wrapping::None)
                    .boxed(),
                Space::new().width(SP.m).boxed(),
                dialog_button(
                    t!(i18n, ids::DIALOG_BUTTON_CLOSE),
                    DialogButtonKind::Cancel,
                    Some(Message::Annotate(AnnotateMsg::CloseDialog)),
                    tokens,
                ),
                controls::accept_button(i18n, changes, accept, tokens),
            ],
        )
    }
}
