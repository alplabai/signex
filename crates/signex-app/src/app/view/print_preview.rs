//! Export-error and print / PDF preview modal builders.
//!
//! Extracted verbatim from `view/mod.rs` (ADR-0001, issue #164) as pure
//! code motion — no behaviour change. These are methods of the same
//! `Signex` view impl, split across sibling files.

use super::*;

impl Signex {
    /// Export-error modal — plain "something went wrong, here's the
    /// message" dialog with an OK button. Sits on top of the print-preview
    /// overlay when both would otherwise render; dismiss_layer handles
    /// click-outside-to-close.
    pub(super) fn view_export_error(&self) -> Element<'_, Message> {
        use iced::widget::{button, column, container, row, text};
        let msg = match &self.document_state.export_error {
            Some(m) => m.clone(),
            None => return iced::widget::Space::new().into(),
        };

        let tokens = &self.document_state.panel_ctx.tokens;
        let panel_bg = crate::styles::ti(tokens.panel_bg);
        let text_c = crate::styles::ti(tokens.text);
        let border_c = crate::styles::ti(tokens.border);
        let err_red = iced::Color::from_rgb(0.85, 0.25, 0.25);

        let ok_btn = button(text("OK").size(12).color(iced::Color::WHITE))
            .padding([6, 20])
            .on_press(Message::Export(ExportMsg::DismissError))
            .style(
                move |_: &iced::Theme, _status| iced::widget::button::Style {
                    background: Some(err_red.into()),
                    text_color: iced::Color::WHITE,
                    border: iced::Border {
                        radius: iced::border::Radius::from(4.0),
                        ..iced::Border::default()
                    },
                    ..iced::widget::button::Style::default()
                },
            );

        let body = column![
            row![
                text("\u{26A0}").size(24).color(err_red),
                iced::widget::Space::new().width(10),
                text("Export Failed").size(14).color(text_c),
            ]
            .align_y(iced::Alignment::Center),
            iced::widget::Space::new().height(8),
            text(msg).size(12).color(text_c),
            iced::widget::Space::new().height(12),
            row![iced::widget::Space::new().width(Length::Fill), ok_btn,],
        ]
        .padding(20);

        let card = container(body)
            .max_width(480)
            .style(move |_: &iced::Theme| container::Style {
                background: Some(panel_bg.into()),
                border: iced::Border {
                    color: border_c,
                    width: 1.0,
                    radius: iced::border::Radius::from(8.0),
                },
                shadow: iced::Shadow {
                    color: iced::Color::from_rgba(0.0, 0.0, 0.0, 0.35),
                    offset: iced::Vector::new(0.0, 4.0),
                    blur_radius: 16.0,
                },
                ..container::Style::default()
            });

        container(card)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center)
            .into()
    }

    /// #431 — netlist-incomplete "Export anyway (incomplete)?" prompt.
    ///
    /// Same card idiom as [`Self::view_export_error`] (theme-token panel /
    /// text / border; the sibling modal's severity glyph), but it leads with
    /// the refusal explanation — unchanged severity, this is a fab deliverable
    /// — and offers TWO actions: write the partial `.net` anyway (with the
    /// omission recorded in its header comment) or cancel and write nothing.
    /// The refusal stays the default; "Export anyway" is an explicit choice.
    pub(super) fn view_netlist_incomplete_prompt(&self) -> Element<'_, Message> {
        use iced::widget::{button, column, container, row, text};
        let Some(prompt) = self.document_state.netlist_incomplete_prompt.as_ref() else {
            return iced::widget::Space::new().into();
        };

        let tokens = &self.document_state.panel_ctx.tokens;
        let panel_bg = crate::styles::ti(tokens.panel_bg);
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);
        // The sibling export-error modal's severity cue — reused verbatim so
        // the netlist-incomplete prompt reads as the same modal family.
        let warn = iced::Color::from_rgb(0.85, 0.25, 0.25);

        // Loud, deliberate action — mirrors the export-error OK button's red.
        let export_anyway = button(
            text("Export anyway (incomplete)")
                .size(12)
                .color(iced::Color::WHITE),
        )
        .padding([6, 16])
        .on_press(Message::Export(ExportMsg::NetlistExportAnyway))
        .style(
            move |_: &iced::Theme, _status| iced::widget::button::Style {
                background: Some(warn.into()),
                text_color: iced::Color::WHITE,
                border: iced::Border {
                    radius: iced::border::Radius::from(4.0),
                    ..iced::Border::default()
                },
                ..iced::widget::button::Style::default()
            },
        );

        // Neutral default — themed panel background + border, no accent.
        let cancel = button(text("Cancel").size(12).color(text_c))
            .padding([6, 20])
            .on_press(Message::Export(ExportMsg::NetlistCancelIncomplete))
            .style(
                move |_: &iced::Theme, _status| iced::widget::button::Style {
                    background: Some(panel_bg.into()),
                    text_color: text_c,
                    border: iced::Border {
                        color: border_c,
                        width: 1.0,
                        radius: iced::border::Radius::from(4.0),
                    },
                    ..iced::widget::button::Style::default()
                },
            );

        // The omitted pages, one line each — the same detail the exported
        // file's INCOMPLETE header will carry.
        let detail = prompt.messages.join("\n");

        let body = column![
            row![
                text("\u{26A0}").size(24).color(warn),
                iced::widget::Space::new().width(10),
                text("Netlist is incomplete").size(14).color(text_c),
            ]
            .align_y(iced::Alignment::Center),
            iced::widget::Space::new().height(8),
            text(
                "The project netlist does not cover the whole project. Exporting it \
                 would produce a file missing components and carrying the wrong net \
                 names wherever nets merge through a page that is not in it — and a \
                 PCB import cannot tell. You can export it anyway: the file will be \
                 marked INCOMPLETE in its header and will list the omitted pages."
            )
            .size(12)
            .color(text_c),
            iced::widget::Space::new().height(8),
            text(detail).size(11).color(text_muted),
            iced::widget::Space::new().height(14),
            row![
                iced::widget::Space::new().width(Length::Fill),
                cancel,
                iced::widget::Space::new().width(8),
                export_anyway,
            ]
            .align_y(iced::Alignment::Center),
        ]
        .padding(20);

        let card = container(body)
            .max_width(520)
            .style(move |_: &iced::Theme| container::Style {
                background: Some(panel_bg.into()),
                border: iced::Border {
                    color: border_c,
                    width: 1.0,
                    radius: iced::border::Radius::from(8.0),
                },
                shadow: iced::Shadow {
                    color: iced::Color::from_rgba(0.0, 0.0, 0.0, 0.35),
                    offset: iced::Vector::new(0.0, 4.0),
                    blur_radius: 16.0,
                },
                ..container::Style::default()
            });

        container(card)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center)
            .into()
    }

    /// Print Preview overlay. Shows thumbnails of every rendered page on
    /// the left, the selected page full-size on the right, with Export PDF
    /// and Close buttons at the bottom. Triggered by File → Print Preview
    /// (Ctrl+P) and File → Export PDF; disappears on Close or when the
    /// export completes. In-window flavour wraps the body in `wrap_modal`
    /// for backdrop + drag-to-position.
    pub(super) fn view_print_preview(&self) -> Element<'_, Message> {
        use crate::app::state::ModalId;
        use crate::app::view::dialogs::wrap_modal;
        let body = self.view_print_preview_inner(true);
        let offset = self
            .ui_state
            .modal_offsets
            .get(&ModalId::PrintPreview)
            .copied()
            .unwrap_or((0.0, 0.0));
        wrap_modal(
            body,
            offset,
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
        use crate::app::view::dialogs::{
            MODAL_HEADER_HEIGHT, MODAL_HEADER_PADDING, MODAL_HEADER_TITLE_SIZE, close_x_button,
            detached_header, draggable_header,
        };
        use iced::widget::{Space, button, column, container, row, text};
        let theme_id = self.ui_state.theme_id;

        let preview = match &self.document_state.preview {
            Some(p) => p,
            None => return iced::widget::Space::new().into(),
        };

        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let accent_c = crate::styles::ti(tokens.accent);

        // Header — same chrome as every other modal.
        let header_content: Element<'_, Message> = container(
            row![
                text("Export PDF")
                    .size(MODAL_HEADER_TITLE_SIZE)
                    .color(text_c),
                Space::new().width(Length::Fill),
                close_x_button(
                    Message::PrintPreview(PrintPreviewMsg::Close),
                    theme_id,
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
                ModalId::PrintPreview,
                self.interaction_state.last_mouse_pos,
            )
        } else {
            detached_header(header_content, ModalId::PrintPreview)
        };

        // Tab strip — Preview | Settings.
        let tab_strip = self.view_pdf_tab_strip(preview.active_tab);

        // Body switches by tab.
        let body: Element<'_, Message> = match preview.active_tab {
            PdfPreviewTab::Preview => self.view_pdf_preview_tab(preview),
            PdfPreviewTab::Settings => self.view_pdf_settings_tab(preview),
        };

        // Footer — page count + Export PDF.
        let export_btn = button(text("Export PDF").size(12).color(iced::Color::WHITE))
            .padding([6, 14])
            .on_press(Message::PrintPreview(PrintPreviewMsg::Export))
            .style(
                move |_: &iced::Theme, _status| iced::widget::button::Style {
                    background: Some(accent_c.into()),
                    text_color: iced::Color::WHITE,
                    border: iced::Border {
                        radius: iced::border::Radius::from(4.0),
                        ..iced::Border::default()
                    },
                    ..iced::widget::button::Style::default()
                },
            );
        let footer_caption = if preview.pages.is_empty() {
            "No files selected for export".to_string()
        } else {
            format!("{} page(s) — preview at 96 DPI", preview.pages.len())
        };
        let footer = container(
            row![
                text(footer_caption).size(11).color(text_muted),
                Space::new().width(Length::Fill),
                export_btn,
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
        )
        .padding([10, 14]);

        let dialog = container(
            column![header, tab_strip, body, footer]
                .width(Self::PDF_MODAL_W)
                .height(Self::PDF_MODAL_H),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true);
        dialog.into()
    }
}
