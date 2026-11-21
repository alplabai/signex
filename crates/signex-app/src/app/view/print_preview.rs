//! Export-error and print / PDF preview modal builders.
//!
//! Extracted verbatim from `view/mod.rs` (ADR-0001, issue #164) as pure
//! code motion — no behaviour change. These are methods of the same
//! `Signex` view impl, split across sibling files.

use super::*;
use crate::styles::{
    DialogButtonKind, dialog_body, dialog_button, dialog_footer, dialog_footer_split,
    dialog_message,
};
use iced::widget::Widget;
use signex_i18n::{I18n, Localized, ids, t};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

impl Signex {
    /// The export-error dialog in its own OS window: the card filling the
    /// window surface.
    pub(super) fn view_error_notice(&self) -> Element<'_, Message> {
        match self.error_notice_card() {
            Some(card) => self.fill_dialog_window(card),
            None => iced::widget::Space::new().boxed(),
        }
    }

    /// Export-error card — plain "something went wrong, here's the message"
    /// dialog with an OK button. The in-window overlay hands it, unpositioned,
    /// to the dismiss helper so only a press outside the card closes it
    /// (#2377).
    pub(super) fn error_notice_card(&self) -> Option<Element<'_, Message>> {
        use iced::widget::{column, container};
        let notice = self.document_state.error_notice.as_ref()?;

        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = self.ui_state.i18n.as_ref();

        let mut buttons = notice_choice_buttons(i18n, notice, tokens);
        // A lone OK is the primary; beside choices it is the leading Cancel.
        if buttons.is_empty() {
            buttons.push(dialog_button(
                t!(i18n, ids::DIALOG_BUTTON_OK),
                DialogButtonKind::Primary,
                Some(Message::Overlay(OverlayMsg::DismissErrorNotice)),
                tokens,
            ));
        } else {
            buttons.insert(
                0,
                dialog_button(
                    t!(i18n, ids::DIALOG_BUTTON_CANCEL),
                    DialogButtonKind::Cancel,
                    Some(Message::Overlay(OverlayMsg::DismissErrorNotice)),
                    tokens,
                ),
            );
        }
        let lead: Element<'_, Message> = crate::styles::dialog_intro(
            crate::styles::DialogIcon::Error,
            notice.title_text(i18n),
            None,
            tokens,
        );
        let body = column![
            dialog_body(
                column![
                    lead,
                    container(iced::widget::scrollable(dialog_message(
                        notice.detail_text(i18n),
                        tokens
                    )))
                    .height(Length::Fit.max(200.0)),
                ]
                .spacing(SP.m)
            ),
            dialog_footer(buttons),
        ];

        let graphite_card = crate::styles::modal_card(tokens);
        let card = container(body)
            .width(Length::Fit.max(
                if self.modal_detached(crate::app::state::ModalId::ErrorNotice) {
                    f32::MAX
                } else {
                    480.0
                },
            ))
            .style(graphite_card);
        Some(card.boxed())
    }

    /// `card` centred in a dialog's own OS window, on the panel surface.
    fn fill_dialog_window<'a>(&self, card: Element<'a, Message>) -> Element<'a, Message> {
        use iced::widget::container;
        let panel_bg = crate::styles::ti(self.document_state.panel_ctx.tokens.panel_bg);
        container(card)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center)
            .style(move |theme: &iced::Theme| {
                // In its own window the dialog fills the window surface.
                if crate::styles::is_dialog_window(theme) {
                    container::Style {
                        background: Some(panel_bg.into()),
                        ..container::Style::default()
                    }
                } else {
                    container::Style::default()
                }
            })
            .boxed()
    }

    /// The Export PDF dialog: one screen with the page list, the page and
    /// the settings column (`pdf_preview/`), Cancel and Export PDF… in the
    /// footer. Opened by File → Print (Ctrl+P) and File → Export → PDF;
    /// closed by the header X or Cancel (Close once an export finished). The
    /// in-window flavour wraps the body in `wrap_dialog` for backdrop +
    /// drag-to-position.
    pub(super) fn view_print_preview(&self) -> Element<'_, Message> {
        use crate::app::state::ModalId;
        // `pdf_modal_size` is already fitted; fitting again is a no-op.
        self.wrap_dialog(
            ModalId::PrintPreview,
            self.view_print_preview_inner(true),
            self.pdf_modal_size(),
        )
    }

    /// The in-window export modal at its design size, capped to the window
    /// (#1227).
    pub(in crate::app::view) fn pdf_modal_size(&self) -> (f32, f32) {
        crate::app::view::dialogs::fit_modal_size(
            self.ui_state.window_size,
            (Self::PDF_MODAL_W, Self::PDF_MODAL_H),
        )
    }

    /// Detached-window flavour — bare body, no backdrop, no in-window
    /// drag handler (the OS window-drag covers the header).
    pub(super) fn view_print_preview_body(&self) -> Element<'_, Message> {
        self.view_print_preview_inner(false)
    }

    fn view_print_preview_inner(&self, draggable: bool) -> Element<'_, Message> {
        use crate::app::state::ModalId;
        use crate::app::view::dialogs::{
            DIALOG_HEADER_H, draggable_header, graphite_header, os_title_bar_slot,
        };
        use iced::widget::{column, container};
        use signex_widgets::text;

        let preview = match &self.document_state.preview {
            Some(p) => p,
            None => return iced::widget::Space::new().boxed(),
        };

        let i18n = self.ui_state.i18n.as_ref();
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_muted = crate::styles::ti(tokens.text_secondary);

        // Header — the modal chrome, with the file and its sheet count.
        let header_content: Element<'_, Message> = graphite_header(
            i18n.message_plain(ids::DIALOG_PRINT_PREVIEW_EXPORT_PDF, &[]),
            Some(self.pdf_dialog_subtitle(preview)),
            None,
            Message::PrintPreview(PrintPreviewMsg::Close),
            DIALOG_HEADER_H,
            tokens,
        );
        let header = if draggable {
            draggable_header(
                header_content,
                ModalId::PrintPreview,
                self.interaction_state.last_mouse_pos,
            )
        } else {
            os_title_bar_slot()
        };

        // One screen: page list, page, settings column.
        let body = self.view_pdf_dialog_body(preview);

        // Footer — the status caption, then Cancel before the action and
        // Close once an export has finished, and Export PDF….
        let dismiss = if preview.export_status.is_some() {
            ids::DIALOG_BUTTON_CLOSE
        } else {
            ids::DIALOG_BUTTON_CANCEL
        };
        let footer = dialog_footer_split(
            vec![
                text(self.pdf_footer_caption(preview))
                    .size(TS.body)
                    .color(text_muted)
                    .boxed(),
            ],
            vec![
                dialog_button(
                    t!(i18n, dismiss),
                    DialogButtonKind::Cancel,
                    Some(Message::PrintPreview(PrintPreviewMsg::Close)),
                    tokens,
                ),
                dialog_button(
                    t!(i18n, ids::PDF_EXPORT_PDF),
                    DialogButtonKind::Primary,
                    Some(Message::PrintPreview(PrintPreviewMsg::Export)),
                    tokens,
                ),
            ],
        );

        // Detached: the OS window owns the size, so fill it.
        let (modal_w, modal_h): (Length, Length) = if draggable {
            let (w, h) = self.pdf_modal_size();
            (Length::Fixed(w), Length::Fixed(h))
        } else {
            (Length::Fill, Length::Fill)
        };
        let dialog = container(column![header, body, footer].width(modal_w).height(modal_h))
            .style(crate::styles::modal_card(tokens))
            .clip(true);
        dialog.boxed()
    }
}

/// The extra buttons a notice offers before its dismiss button (#1826).
fn notice_choice_buttons<'a>(
    i18n: &I18n,
    notice: &crate::app::state::ErrorNotice,
    tokens: &signex_types::theme::ThemeTokens,
) -> Vec<Element<'a, Message>> {
    use crate::app::state::NoticeAction;
    use crate::persist::DiskDrift;
    let Some(NoticeAction::DiskChanged { drift, .. }) = &notice.action else {
        return Vec::new();
    };
    let choice =
        |label: Localized, kind: DialogButtonKind, message: OverlayMsg| -> Element<'a, Message> {
            dialog_button(label, kind, Some(Message::Overlay(message)), tokens)
        };
    match drift {
        DiskDrift::Changed => vec![
            choice(
                t!(i18n, ids::DIALOG_DISK_CONFLICT_RELOAD),
                DialogButtonKind::Danger,
                OverlayMsg::DiskConflictReloadChosen,
            ),
            choice(
                t!(i18n, ids::DIALOG_OVERWRITE),
                DialogButtonKind::Danger,
                OverlayMsg::DiskConflictOverwriteChosen,
            ),
        ],
        DiskDrift::Missing => vec![choice(
            t!(i18n, ids::DIALOG_DISK_CONFLICT_RECREATE),
            DialogButtonKind::Primary,
            OverlayMsg::DiskConflictOverwriteChosen,
        )],
        // Writing would have to re-create the folder, which a save refuses.
        DiskDrift::FolderMissing => Vec::new(),
    }
}
