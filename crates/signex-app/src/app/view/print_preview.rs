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
use signex_i18n::{I18n, Localized, ids, shortcut_ids, t};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

impl Signex {
    /// Export-error modal — plain "something went wrong, here's the
    /// message" dialog with an OK button. Sits on top of the print-preview
    /// overlay when both would otherwise render; dismiss_layer handles
    /// click-outside-to-close.
    pub(super) fn view_error_notice(&self) -> Element<'_, Message> {
        use iced::widget::{column, container};
        let Some(notice) = self.document_state.error_notice.as_ref() else {
            return iced::widget::Space::new().boxed();
        };

        let tokens = &self.document_state.panel_ctx.tokens;
        let panel_bg = crate::styles::ti(tokens.panel_bg);
        let i18n = self.ui_state.i18n.as_ref();

        let mut buttons = notice_choice_buttons(i18n, notice, tokens);
        // A lone OK is the primary; beside choices it is the leading Cancel.
        if buttons.is_empty() {
            buttons.push(dialog_button(
                t!(i18n, shortcut_ids::PREF_KEYMAP_OK),
                DialogButtonKind::Primary,
                Some(Message::Overlay(OverlayMsg::DismissErrorNotice)),
                tokens,
            ));
        } else {
            buttons.insert(
                0,
                dialog_button(
                    t!(i18n, ids::DIALOG_ASSIGN_FOOTPRINTS_CANCEL),
                    DialogButtonKind::Cancel,
                    Some(Message::Overlay(OverlayMsg::DismissErrorNotice)),
                    tokens,
                ),
            );
        }
        let lead: Element<'_, Message> = crate::styles::dialog_intro(
            crate::styles::DialogIcon::Error,
            notice.title.clone(),
            None,
            tokens,
        );
        let body = column![
            dialog_body(
                column![
                    lead,
                    container(iced::widget::scrollable(dialog_message(
                        notice.detail.clone(),
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

    /// #431 — netlist-incomplete "Export anyway (incomplete)?" prompt.
    ///
    /// Same card idiom as [`Self::view_error_notice`] (theme-token panel /
    /// text / border; the sibling modal's severity glyph), but it leads with
    /// the refusal explanation — unchanged severity, this is a fab deliverable
    /// — and offers TWO actions: write the partial `.net` anyway (with the
    /// omission recorded in its header comment) or cancel and write nothing.
    /// The refusal stays the default; "Export anyway" is an explicit choice.
    pub(super) fn view_netlist_incomplete_prompt(&self) -> Element<'_, Message> {
        use iced::widget::{column, container};
        use signex_widgets::{raw, text};
        let Some(prompt) = self.document_state.netlist_incomplete_prompt.as_ref() else {
            return iced::widget::Space::new().boxed();
        };

        let tokens = &self.document_state.panel_ctx.tokens;
        let panel_bg = crate::styles::ti(tokens.panel_bg);
        let i18n = self.ui_state.i18n.as_ref();
        let text_muted = crate::styles::ti(tokens.text_secondary);
        // The sibling export-error modal's severity cue — reused verbatim so
        // the netlist-incomplete prompt reads as the same modal family.

        // The omitted pages, one line each — the same detail the
        // warning logged on export will carry.
        let detail = prompt.messages.join("\n");

        let lead: Element<'_, Message> = crate::styles::dialog_intro(
            crate::styles::DialogIcon::Warn,
            "Netlist is incomplete",
            None,
            tokens,
        );
        let body = column![
            dialog_body(
                column![
                    lead,
                    dialog_message(
                        "The project netlist does not cover the whole project. Exporting it \
                         would produce a file missing components and carrying the wrong net \
                         names wherever nets merge through a page that is not in it — and a \
                         PCB import cannot tell. You can export it anyway: the file stays \
                         a valid netlist, and the omitted pages are reported in the Messages panel.",
                        tokens,
                    ),
                    container(iced::widget::scrollable(
                        text(raw(detail)).size(TS.body).color(text_muted)
                    ))
                    .height(Length::Fit.max(120.0)),
                ]
                .spacing(SP.m)
            ),
            // Cancel is the default; exporting an incomplete netlist is a
            // deliberate, risky choice, so it takes the danger style.
            dialog_footer(vec![
                dialog_button(
                    t!(i18n, ids::DIALOG_ASSIGN_FOOTPRINTS_CANCEL),
                    DialogButtonKind::Cancel,
                    Some(Message::Export(ExportMsg::NetlistCancelIncomplete)),
                    tokens,
                ),
                dialog_button(
                    t!(i18n, ids::DIALOG_NETLIST_EXPORT_ANYWAY),
                    DialogButtonKind::Danger,
                    Some(Message::Export(ExportMsg::NetlistExportAnyway)),
                    tokens,
                ),
            ]),
        ];

        let graphite_card = crate::styles::modal_card(tokens);
        let card = container(body)
            .width(Length::Fit.max(
                if self.modal_detached(crate::app::state::ModalId::NetlistIncompletePrompt) {
                    f32::MAX
                } else {
                    520.0
                },
            ))
            .style(graphite_card);

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

    /// Print Preview overlay. Shows thumbnails of every rendered page on
    /// the left, the selected page full-size on the right, with Export PDF
    /// and Close buttons at the bottom. Triggered by File → Print Preview
    /// (Ctrl+P) and File → Export PDF; disappears on Close or when the
    /// export completes. In-window flavour wraps the body in `wrap_dialog`
    /// for backdrop + drag-to-position.
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
        use crate::app::state::{ModalId, PdfPreviewTab};
        use crate::app::view::dialogs::{draggable_header, modal_title_header, os_title_bar_slot};
        use iced::widget::{column, container};
        use signex_widgets::{raw, text};

        let preview = match &self.document_state.preview {
            Some(p) => p,
            None => return iced::widget::Space::new().boxed(),
        };

        let i18n = self.ui_state.i18n.as_ref();
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_muted = crate::styles::ti(tokens.text_secondary);

        // Header — same chrome as every other modal.
        let header_content: Element<'_, Message> = modal_title_header(
            "Export PDF",
            Message::PrintPreview(PrintPreviewMsg::Close),
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

        // Tab strip — Preview | Settings.
        let tab_strip = self.view_pdf_tab_strip(preview.active_tab);

        // Body switches by tab.
        let body: Element<'_, Message> = match preview.active_tab {
            PdfPreviewTab::Preview => self.view_pdf_preview_tab(preview),
            PdfPreviewTab::Settings => self.view_pdf_settings_tab(preview),
        };

        // Footer — page count + Export PDF.
        let footer_caption = match (&preview.export_status, preview.pages.is_empty()) {
            (Some(status), _) => status.clone(),
            (None, true) => "No files selected for export".to_string(),
            (None, false) => format!("{} page(s)", preview.pages.len()),
        };
        let footer = dialog_footer_split(
            vec![
                text(raw(footer_caption))
                    .size(TS.body)
                    .color(text_muted)
                    .boxed(),
            ],
            vec![dialog_button(
                t!(i18n, ids::DIALOG_PRINT_PREVIEW_EXPORT_PDF),
                DialogButtonKind::Primary,
                Some(Message::PrintPreview(PrintPreviewMsg::Export)),
                tokens,
            )],
        );

        // Detached: the OS window owns the size, so fill it.
        let (modal_w, modal_h): (Length, Length) = if draggable {
            let (w, h) = self.pdf_modal_size();
            (Length::Fixed(w), Length::Fixed(h))
        } else {
            (Length::Fill, Length::Fill)
        };
        let dialog = container(
            column![header, tab_strip, body, footer]
                .width(modal_w)
                .height(modal_h),
        )
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
