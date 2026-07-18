//! BOM (Bill of Materials) preview modal — the modal shell (header, toolbar
//! strip, footer) that stitches together the data grid (`table`) and the
//! Properties sidebar (`sidebar`).
//!
//! Extracted from `view/dialogs.rs` (ADR-0001, issue #164). The section
//! builders were split into `bom/table.rs` and `bom/sidebar.rs` as pure code
//! motion — the final `column!`/`row!` child order is preserved byte-for-byte,
//! so the rendered modal is pixel-identical.

use super::*;
use iced::widget::{Space, column, container, row, text};
use iced::{Element, Length};

use super::widgets::{
    close_x_button, detached_header, draggable_header, primary_button_themed, wrap_modal,
};
use super::{MODAL_HEADER_HEIGHT, MODAL_HEADER_PADDING, MODAL_HEADER_TITLE_SIZE};

mod sidebar;
mod table;

impl Signex {
    pub(in crate::app::view) fn view_bom_preview(&self) -> Element<'_, Message> {
        let modal_w = 1000.0_f32;
        let modal_h = 700.0_f32;
        let dialog = self.view_bom_preview_body_inner(true);
        let offset = self
            .ui_state
            .modal_offsets
            .get(&super::super::super::state::ModalId::BomPreview)
            .copied()
            .unwrap_or((0.0, 0.0));
        wrap_modal(
            dialog,
            offset,
            self.ui_state.window_size,
            (modal_w, modal_h),
        )
    }

    pub(in crate::app::view) fn view_bom_preview_body(&self) -> Element<'_, Message> {
        self.view_bom_preview_body_inner(false)
    }
    fn view_bom_preview_body_inner(&self, draggable: bool) -> Element<'_, Message> {
        let Some(ref preview) = self.document_state.bom_preview else {
            return container(Space::new()).into();
        };
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);
        let theme_id = self.ui_state.theme_id;

        let header_content: Element<'_, Message> = container(
            row![
                text("Bill of Materials")
                    .size(MODAL_HEADER_TITLE_SIZE)
                    .color(text_c),
                Space::new().width(Length::Fill),
                close_x_button(
                    Message::BomPreview(BomPreviewMsg::Close),
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
        let _ = border_c;
        let header = if draggable {
            draggable_header(
                header_content,
                super::super::super::state::ModalId::BomPreview,
                self.interaction_state.last_mouse_pos,
            )
        } else {
            detached_header(header_content, super::super::super::state::ModalId::BomPreview)
        };

        let body = self.bom_table();
        let sidebar = self.bom_sidebar();

        let active_variant_label = preview
            .options
            .active_variant
            .clone()
            .unwrap_or_else(|| "Base".to_string());
        let row_count = preview.table.rows.len();
        let total_count = preview.table.rows.len();
        let status_label = format!("{} of {} lines visible", row_count, total_count);
        let variant_label = format!("Current variant: {}", active_variant_label);

        // Variant dropdown for the top toolbar — placeholder in the
        // header strip area, sits to the left. (i) info badge sits
        // on the right.
        let variant_dropdown: Element<'_, Message> = if preview.variants.is_empty() {
            text(format!("Variant: {}", active_variant_label))
                .size(11)
                .color(text_muted)
                .into()
        } else {
            container(
                row![
                    Space::new().width(Length::Fill),
                    text(active_variant_label.clone()).size(11).color(text_c),
                    Space::new().width(Length::Fill),
                    text("\u{25BE}").size(10).color(text_c),
                ]
                .align_y(iced::Alignment::Center),
            )
            .width(160)
            .height(24)
            .padding([0, 10])
            .style(move |_: &iced::Theme| iced::widget::container::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgba(
                    1.0, 1.0, 1.0, 0.04,
                ))),
                border: iced::Border {
                    width: 1.0,
                    radius: 3.0.into(),
                    color: border_c,
                },
                ..iced::widget::container::Style::default()
            })
            .into()
        };
        let info_badge: Element<'_, Message> =
            container(text("i").size(11).color(iced::Color::WHITE))
                .width(20)
                .height(20)
                .align_x(iced::alignment::Horizontal::Center)
                .align_y(iced::alignment::Vertical::Center)
                .style(move |_: &iced::Theme| iced::widget::container::Style {
                    background: Some(iced::Background::Color(iced::Color {
                        a: 0.7,
                        ..crate::styles::ti(tokens.accent)
                    })),
                    border: iced::Border {
                        width: 0.0,
                        radius: 10.0.into(),
                        color: iced::Color::TRANSPARENT,
                    },
                    ..iced::widget::container::Style::default()
                })
                .into();
        let toolbar_strip = container(
            row![
                variant_dropdown,
                Space::new().width(Length::Fill),
                info_badge,
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding([8, 14])
        .width(Length::Fill)
        .style(crate::styles::toolbar_strip(tokens));

        // Main row: scrollable table + 1 px divider + sidebar.
        // Body now uses nested scrollables (vertical inside
        // horizontal) so the horizontal bar lives below the last
        // row instead of clipping it — no bottom-padding hack
        // needed.
        let main_row = row![
            container(body)
                .width(Length::Fill)
                .height(Length::Fill)
                .padding([6, 8]),
            container(Space::new())
                .width(1)
                .height(Length::Fill)
                .style(move |_: &iced::Theme| iced::widget::container::Style {
                    background: Some(iced::Background::Color(border_c)),
                    ..iced::widget::container::Style::default()
                }),
            container(sidebar)
                .width(Length::Fixed(320.0))
                .height(Length::Fill),
        ]
        .height(Length::Fill);

        // Combined footer: status text on the left + Export button
        // on the right, in ONE strip with bottom-rounded radius.
        // Dropping the separate status / button rows removes the
        // empty band between them and tightens the modal's
        // bottom chrome.
        let footer = container(
            row![
                text(status_label).size(10).color(text_muted),
                Space::new().width(12),
                text("|").size(10).color(text_muted),
                Space::new().width(12),
                text(variant_label).size(10).color(text_muted),
                Space::new().width(Length::Fill),
                primary_button_themed(
                    "Export\u{2026}",
                    Some(Message::BomPreview(BomPreviewMsg::Export)),
                    border_c,
                    Some(crate::styles::ti(tokens.accent)),
                ),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding([6, 14])
        .width(Length::Fill)
        .style(crate::styles::modal_footer_strip(tokens));

        // 1 px theme-border line — same style the main app uses
        // between the menu bar and the document tab strip
        // (`crate::styles::chrome_separator`). The inner Space is
        // sized Fill on both axes so iced doesn't shrink the
        // container to zero height in some layout scenarios.
        let title_separator = container(Space::new().width(Length::Fill).height(Length::Fill))
            .width(Length::Fill)
            .height(1)
            .style(crate::styles::chrome_separator(tokens));

        let dialog = container(
            column![header, title_separator, toolbar_strip, main_row, footer]
                .spacing(0)
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true);
        dialog.into()
    }
}
