//! PDF preview / settings tab and section builders for the print-
//! preview modal shell in `print_preview.rs`.
//!
//! Extracted verbatim from `view/print_preview.rs` (ADR-0001, issue
//! #164) as pure code motion — no behaviour change. These are methods
//! of the same `Signex` view impl, split across sibling files.

use super::*;

mod preview;
mod settings;

impl Signex {
    /// Two-tab strip — Preview | Settings — sitting just under the
    /// modal header. Uses the same `TabPill` widget the document tab
    /// bar paints with: 3-sided border (top + L/R), accent stripe on
    /// the active tab, fill that fades for inactive. `is_last=true`
    /// on the rightmost so the trailing border doesn't double up
    /// against an adjacent tab's left edge.
    pub(super) fn view_pdf_tab_strip(&self, active: crate::app::state::PdfPreviewTab) -> Element<'_, Message> {
        use crate::app::state::PdfPreviewTab;
        use iced::widget::{Space, container, mouse_area, row, text};
        use signex_widgets::tab_pill::{AccentPosition, TabPill, TabPillStyle};
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);
        let accent_c = crate::styles::ti(tokens.accent);
        let hover_c = crate::styles::ti(tokens.hover);

        let pill_fill = |is_active: bool| -> iced::Color {
            if is_active {
                hover_c
            } else {
                iced::Color {
                    a: hover_c.a * 0.35,
                    ..hover_c
                }
            }
        };

        let tab = |label: &'static str, this: PdfPreviewTab, is_last: bool| {
            let is_active = this == active;
            let label_color = if is_active { text_c } else { text_muted };
            let style = TabPillStyle {
                fill: pill_fill(is_active),
                border: border_c,
                accent: accent_c,
                is_active,
                is_last,
                accent_position: AccentPosition::Bottom,
            };
            let inner = container(text(label).size(12).color(label_color)).padding([6, 18]);
            mouse_area(TabPill::new(inner, style))
                .on_press(Message::PrintPreview(PrintPreviewMsg::SetTab(this)))
                .interaction(iced::mouse::Interaction::Pointer)
        };

        container(
            row![
                tab("Preview", PdfPreviewTab::Preview, false),
                tab("Settings", PdfPreviewTab::Settings, true),
                Space::new().width(Length::Fill),
            ]
            .spacing(0)
            .align_y(iced::Alignment::Center),
        )
        .padding([0, 14])
        .into()
    }
}
