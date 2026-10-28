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
    close_x_button, draggable_header, os_title_bar_slot, primary_button_themed, wrap_modal,
};
use super::{MODAL_HEADER_HEIGHT, MODAL_HEADER_PADDING, MODAL_HEADER_TITLE_SIZE};

mod rules;
mod sidebar;
mod table;

pub(in crate::app) use table::{default_column_width, sorted_row_order};

impl Signex {
    pub(in crate::app::view) fn view_bom_preview(&self) -> Element<'_, Message> {
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
            self.bom_modal_size(),
        )
    }

    /// The in-window BOM modal: 1000 x 700 where the window allows, capped
    /// to the window otherwise (#1227).
    pub(in crate::app::view) fn bom_modal_size(&self) -> (f32, f32) {
        super::widgets::fit_modal_size(self.ui_state.window_size, (1000.0, 700.0))
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
        let header = if draggable {
            draggable_header(
                header_content,
                super::super::super::state::ModalId::BomPreview,
                self.interaction_state.last_mouse_pos,
            )
        } else {
            os_title_bar_slot()
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
        let mut status_label = format!("{} of {} lines visible", row_count, total_count);
        if let Some(prices) = price_status(&preview.table, &preview.options.columns) {
            status_label.push_str(" · ");
            status_label.push_str(&prices);
        }
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
        let issue_badge: Element<'_, Message> =
            match rules::issue_count_label(&preview.validation_report) {
                Some(label) => {
                    let severity = if preview.validation_report.has_errors() {
                        signex_output::BomIssueSeverity::Error
                    } else {
                        signex_output::BomIssueSeverity::Warning
                    };
                    text(label)
                        .size(10)
                        .color(rules::severity_color(severity, text_muted))
                        .into()
                }
                None => text("No BOM issues").size(10).color(text_muted).into(),
            };
        let footer = container(
            row![
                text(status_label).size(10).color(text_muted),
                Space::new().width(12),
                text("|").size(10).color(text_muted),
                Space::new().width(12),
                text(variant_label).size(10).color(text_muted),
                Space::new().width(Length::Fill),
                issue_badge,
                Space::new().width(12),
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

        let (modal_w, modal_h): (Length, Length) = if draggable {
            let (w, h) = self.bom_modal_size();
            (Length::Fixed(w), Length::Fixed(h))
        } else {
            (Length::Fill, Length::Fill)
        };
        let dialog = container(
            column![header, title_separator, toolbar_strip, main_row, footer]
                .spacing(0)
                .width(modal_w)
                .height(modal_h),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true);
        dialog.into()
    }
}

/// The footer's price summary while a price column is shown (#1023): the
/// total per currency, and how many lines have no cached price (no price
/// for their library row or MPN). `None` while no price column is shown.
pub(in crate::app) fn price_status(
    table: &signex_output::BomTable,
    columns: &[signex_output::BomColumn],
) -> Option<String> {
    if !columns
        .iter()
        .any(|column| signex_output::BomColumn::PRICES.contains(column))
    {
        return None;
    }
    let totals = signex_output::bom::price_totals(table);
    let unpriced = table.rows.iter().filter(|row| row.price.is_none()).count();
    let mut parts = Vec::new();
    if totals.is_empty() {
        parts.push("no cached prices".to_string());
    } else {
        let sums: Vec<String> = totals
            .iter()
            .map(|(currency, total)| format!("{} {currency}", signex_output::bom::money(*total)))
            .collect();
        parts.push(format!("Total {}", sums.join(" + ")));
    }
    if unpriced > 0 && !totals.is_empty() {
        parts.push(format!(
            "{unpriced} line{} without a cached price",
            if unpriced == 1 { "" } else { "s" }
        ));
    }
    Some(parts.join(" · "))
}

#[cfg(test)]
mod price_status_tests {
    use signex_output::bom::BomPrice;
    use signex_output::{BomColumn, BomRow, BomTable};

    use super::price_status;

    fn row(price: Option<(&str, f64)>) -> BomRow {
        BomRow {
            price: price.map(|(currency, extended_price)| BomPrice {
                distributor: "D".into(),
                currency: currency.into(),
                unit_price: extended_price,
                extended_price,
                buy_qty: 1,
                stock: None,
                age: String::new(),
            }),
            ..Default::default()
        }
    }

    #[test]
    fn the_footer_shows_totals_and_unpriced_lines_only_with_a_price_column() {
        let table = BomTable {
            rows: vec![row(Some(("USD", 1.5))), row(Some(("EUR", 2.0))), row(None)],
            ..Default::default()
        };
        assert_eq!(price_status(&table, &[BomColumn::Name]), None);
        assert_eq!(
            price_status(&table, &[BomColumn::UnitPrice]).as_deref(),
            Some("Total 2.00 EUR + 1.50 USD · 1 line without a cached price")
        );
        let none = BomTable {
            rows: vec![row(None)],
            ..Default::default()
        };
        assert_eq!(
            price_status(&none, &[BomColumn::ExtendedPrice]).as_deref(),
            Some("no cached prices")
        );
    }
}
