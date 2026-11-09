//! BOM (Bill of Materials) preview modal — the modal shell (header, toolbar
//! strip, footer) that stitches together the data grid (`table`) and the
//! Properties sidebar (`sidebar`).
//!
//! Extracted from `view/dialogs.rs` (ADR-0001, issue #164). The section
//! builders were split into `bom/table.rs` and `bom/sidebar.rs` as pure code
//! motion — the final `column!`/`row!` child order is preserved byte-for-byte,
//! so the rendered modal is pixel-identical.

use super::*;
use iced::widget::{Space, column, container, row};
use iced::{Element, Length, Widget};
use signex_widgets::{raw, text};

use super::widgets::{draggable_header, os_title_bar_slot};
use crate::styles::dialog_button;

mod rules;
mod sidebar;
mod table;

pub(in crate::app) use table::{default_column_width, sorted_row_order};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

impl Signex {
    pub(in crate::app::view) fn view_bom_preview(&self) -> Element<'_, Message> {
        let dialog = self.view_bom_preview_body_inner(true);
        self.wrap_dialog(
            crate::app::state::ModalId::BomPreview,
            dialog,
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
            return container(Space::new()).boxed();
        };
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);

        let header_content: Element<'_, Message> = super::graphite::titled_header(
            "Bill of Materials",
            self.detached_modal_subtitle(crate::app::state::ModalId::BomPreview),
            Message::BomPreview(BomPreviewMsg::Close),
            tokens,
        );
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
            text(raw(format!("Variant: {}", active_variant_label)))
                .size(TS.body)
                .color(text_muted)
                .boxed()
        } else {
            container(
                row![
                    Space::new().width(Length::Fill),
                    text(raw(active_variant_label.clone()))
                        .size(TS.body)
                        .color(text_c),
                    Space::new().width(Length::Fill),
                    text(raw("\u{25BE}")).size(TS.caption).color(text_c),
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
                    ..iced::Border::default()
                },
                ..iced::widget::container::Style::default()
            })
            .boxed()
        };
        let info_badge: Element<'_, Message> =
            container(text(raw("i")).size(TS.body).color(iced::Color::WHITE))
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
                        ..iced::Border::default()
                    },
                    ..iced::widget::container::Style::default()
                })
                .boxed();
        let toolbar_strip = container(
            row![
                variant_dropdown,
                Space::new().width(Length::Fill),
                info_badge,
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding([SP.m, 14.0])
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
                .padding([6.0, SP.m]),
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
                    text(raw(label))
                        .size(TS.caption)
                        .color(rules::severity_color(severity, text_muted, tokens))
                        .boxed()
                }
                None => text(raw("No BOM issues"))
                    .size(TS.caption)
                    .color(text_muted)
                    .boxed(),
            };
        let footer = container(
            row![
                text(raw(status_label)).size(TS.caption).color(text_muted),
                Space::new().width(12),
                text(raw("|")).size(TS.caption).color(text_muted),
                Space::new().width(12),
                text(raw(variant_label)).size(TS.caption).color(text_muted),
                Space::new().width(Length::Fill),
                issue_badge,
                Space::new().width(12),
                dialog_button(
                    "Export\u{2026}",
                    super::graphite::call_to_action(tokens),
                    Some(Message::BomPreview(BomPreviewMsg::Export)),
                    tokens,
                ),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding([SP.s, SP.l])
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
        dialog.boxed()
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
