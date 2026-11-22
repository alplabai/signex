//! BOM dialog footer: the status line (lines shown, parts, prices), then
//! Cancel and the primary button named by the export format.

use super::super::*;
use iced::widget::{Space, container, row};
use iced::{Element, Length, Widget};
use signex_i18n::{I18n, Localized, MessageId, ids, t};
use signex_output::{BomColumn, BomFormat, BomTable};
use signex_widgets::text;

use crate::styles::{DialogButtonKind, Spacing, TypeScale, dialog_button};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

impl Signex {
    pub(super) fn bom_footer(&self) -> Element<'_, Message> {
        let Some(ref preview) = self.document_state.bom_preview else {
            return container(Space::new()).boxed();
        };
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = self.ui_state.i18n.as_ref();
        let ink3 = crate::styles::tokens::resolve(tokens).ink3;
        let status = status_line(
            i18n,
            &preview.table,
            &preview.options.columns,
            preview.visible_lines.len(),
        );
        // The dialog closes when an export completes, so it never shows a
        // completed one: the dismiss button is always Cancel.
        container(
            row![
                text(status)
                    .size(TS.body)
                    .color(ink3)
                    .wrapping(iced::widget::text::Wrapping::None),
                Space::new().width(Length::Fill),
                dialog_button(
                    t!(i18n, ids::DIALOG_BUTTON_CANCEL),
                    DialogButtonKind::Cancel,
                    Some(Message::BomPreview(BomPreviewMsg::Close)),
                    tokens,
                ),
                dialog_button(
                    t!(i18n, export_label(preview.options.format)),
                    DialogButtonKind::Primary,
                    Some(Message::BomPreview(BomPreviewMsg::Export)),
                    tokens,
                ),
            ]
            .spacing(SP.m)
            .align_y(iced::Alignment::Center),
        )
        .padding([SP.m, SP.l])
        .width(Length::Fill)
        .style(crate::styles::modal_footer_strip(tokens))
        .boxed()
    }
}

/// The primary button's words: Export and the format it writes.
fn export_label(format: BomFormat) -> MessageId {
    match format {
        BomFormat::Csv => ids::BOM_EXPORT_CSV,
        BomFormat::Xlsx => ids::BOM_EXPORT_XLSX,
        BomFormat::Html => ids::BOM_EXPORT_HTML,
    }
}

/// The footer status: `visible` of all lines, the parts the BOM lists and,
/// while a price column is shown, the price note ([`price_status`]).
fn status_line(i18n: &I18n, table: &BomTable, columns: &[BomColumn], visible: usize) -> Localized {
    let parts: u64 = table.rows.iter().map(|row| u64::from(row.qty)).sum();
    let counts = [
        ("visible", signex_i18n::Arg::from(visible)),
        ("total", table.rows.len().into()),
        ("parts", parts.into()),
    ];
    match price_status(i18n, table, columns) {
        Some(prices) => {
            let mut args = counts.to_vec();
            args.push(("prices", prices.into()));
            i18n.message(ids::BOM_STATUS_VISIBLE_PRICES, &args)
        }
        None => i18n.message(ids::BOM_STATUS_VISIBLE, &counts),
    }
}

/// The footer's price summary while a price column is shown (#1023): the
/// total per currency, and how many lines have no cached price (no price
/// for their library row or MPN). `None` while no price column is shown.
fn price_status(i18n: &I18n, table: &BomTable, columns: &[BomColumn]) -> Option<String> {
    if !columns
        .iter()
        .any(|column| BomColumn::PRICES.contains(column))
    {
        return None;
    }
    let totals = signex_output::bom::price_totals(table);
    if totals.is_empty() {
        return Some(i18n.message_plain(ids::BOM_PRICE_NONE, &[]));
    }
    let unpriced = table.rows.iter().filter(|row| row.price.is_none()).count();
    // Amounts and currency codes are written the same in every language.
    let sums: Vec<String> = totals
        .iter()
        .map(|(currency, total)| format!("{} {currency}", signex_output::bom::money(*total)))
        .collect();
    let sums = signex_i18n::Arg::from(sums.join(" + "));
    Some(if unpriced > 0 {
        i18n.message_plain(
            ids::BOM_PRICE_TOTAL_UNPRICED,
            &[("sums", sums), ("count", unpriced.into())],
        )
    } else {
        i18n.message_plain(ids::BOM_PRICE_TOTAL, &[("sums", sums)])
    })
}

#[cfg(test)]
#[path = "footer_tests.rs"]
mod tests;
