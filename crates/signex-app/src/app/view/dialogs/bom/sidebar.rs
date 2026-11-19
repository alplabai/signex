//! BOM dialog — the Properties sidebar: the General / Columns / Rules
//! switch over the active pane. General holds the export settings (the
//! not-fitted option and the format); Columns picks and orders the
//! exported columns; Rules is in `rules`.

use super::super::*;
use iced::widget::{Column, Row, Space, column, container, mouse_area, row, scrollable};
use iced::{Background, Border, Color, Element, Length, Theme, Widget};
use signex_i18n::{I18n, Localized, MessageId, UserText, ids, t};
use signex_output::{BomColumn, BomFormat};
use signex_types::theme::ThemeTokens;
use signex_widgets::{Label, SegmentedStyle, button, raw, segmented, text};

use super::super::graphite_annotate::checkbox_pip;
use crate::app::state::{BomPreviewState, BomSidebarTab};
use crate::styles::tokens::resolve;
use crate::styles::{HAIRLINE, Radii, Spacing, TypeScale, ti};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Height of a checkbox row, as drawn.
const CHECK_ROW_H: f32 = 24.0;
/// Height of a Columns-tab row, as drawn.
const COLUMN_ROW_H: f32 = 28.0;
/// Width of the key column of the Export Options form, as drawn.
const FORM_KEY_W: f32 = 56.0;
/// Gap above a section heading, as drawn (the first one sits closer).
const SECTION_GAP: f32 = 20.0;

/// The standard columns the Columns tab offers, in its order.
static COLUMN_OPTIONS: [(BomColumn, MessageId); 14] = [
    (BomColumn::Name, ids::BOM_COLUMN_NAME),
    (BomColumn::Description, ids::BOM_COLUMN_DESCRIPTION),
    (BomColumn::Designator, ids::BOM_COLUMN_DESIGNATOR),
    (BomColumn::Value, ids::BOM_COLUMN_VALUE),
    (BomColumn::Footprint, ids::BOM_COLUMN_FOOTPRINT),
    (BomColumn::LibRef, ids::BOM_COLUMN_LIB_REF),
    (BomColumn::Qty, ids::BOM_COLUMN_QTY),
    // Cached library prices (#1023).
    (BomColumn::UnitPrice, ids::BOM_COLUMN_UNIT_PRICE),
    (BomColumn::BuyQty, ids::BOM_COLUMN_BUY_QTY),
    (BomColumn::ExtendedPrice, ids::BOM_COLUMN_EXT_PRICE),
    (BomColumn::Currency, ids::BOM_COLUMN_CURRENCY),
    (BomColumn::Stock, ids::BOM_COLUMN_STOCK),
    (BomColumn::Distributor, ids::BOM_COLUMN_DISTRIBUTOR),
    (BomColumn::PriceAge, ids::BOM_COLUMN_PRICE_AGE),
];

/// Segmented-control colours of the sidebar.
pub(super) fn segmented_style(tokens: &ThemeTokens) -> SegmentedStyle {
    let r = resolve(tokens);
    SegmentedStyle::new(
        r.sunken,
        r.raised,
        r.shadow,
        r.ink3,
        ti(tokens.text),
        TS.body,
    )
    .fill_width(true)
}

/// An uppercase section heading with a soft rule under it, and an optional
/// trailing count pill. `first` sits closer to what is above it.
pub(super) fn section<'a>(
    i18n: &I18n,
    label: MessageId,
    pill: Option<Element<'a, Message>>,
    first: bool,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let r = resolve(tokens);
    let words = t!(i18n, label).into_string();
    // raw: catalog text already rendered, then upper-cased for the locale.
    let mut line: Row<Element<'a, Message>> = Row::new().align_y(iced::Alignment::Center).push(
        text(raw(i18n.upper_for_display(&words)))
            .size(TS.section)
            .style(crate::styles::section_label(tokens))
            .width(Length::Fill)
            .boxed(),
    );
    if let Some(pill) = pill {
        line = line.push(pill);
    }
    column![
        Space::new().height(if first { SP.m } else { SECTION_GAP }),
        line,
        Space::new().height(SP.s),
        container(Space::new())
            .width(Length::Fill)
            .height(HAIRLINE)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(r.line_soft)),
                ..container::Style::default()
            }),
        Space::new().height(SP.xs),
    ]
    .boxed()
}

/// A checkbox row: the box and its words, the whole row clickable.
pub(super) fn check_row<'a>(
    label: impl Into<Label<'a>>,
    on: bool,
    on_press: Message,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let ink = ti(tokens.text);
    button(
        row![
            checkbox_pip(tokens, on),
            text(label).size(TS.body).color(ink).width(Length::Fill),
        ]
        .spacing(SP.m)
        .align_y(iced::Alignment::Center),
    )
    .on_press(on_press)
    .padding(0)
    .width(Length::Fill)
    .height(CHECK_ROW_H)
    .style(move |_: &Theme, _: button::Status| button::Style {
        text_color: ink,
        ..button::Style::default()
    })
    .boxed()
}

/// A bordered list of rows (the drawn `cardlist`).
pub(super) fn card_list<'a>(
    rows: Vec<Element<'a, Message>>,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let edge = ti(tokens.border);
    container(Column::with_children(rows))
        .width(Length::Fill)
        .clip(true)
        .style(move |_: &Theme| container::Style {
            border: Border {
                width: HAIRLINE,
                color: edge,
                radius: Radii::GRAPHITE.button.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// A muted caption line.
pub(super) fn caption<'a>(words: Localized, tokens: &ThemeTokens) -> Element<'a, Message> {
    text(words)
        .size(TS.caption)
        .color(resolve(tokens).ink4)
        .boxed()
}

impl Signex {
    /// The Properties sidebar: the tab switch over the active pane.
    pub(super) fn bom_sidebar(&self) -> Element<'_, Message> {
        let Some(ref preview) = self.document_state.bom_preview else {
            return container(Space::new()).boxed();
        };
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = self.ui_state.i18n.as_ref();
        let tabs: [(BomSidebarTab, Label<'static>); 3] = [
            (
                BomSidebarTab::General,
                t!(i18n, ids::BOM_TAB_GENERAL).into(),
            ),
            (
                BomSidebarTab::Columns,
                t!(i18n, ids::BOM_TAB_COLUMNS).into(),
            ),
            (BomSidebarTab::Rules, t!(i18n, ids::BOM_TAB_RULES).into()),
        ];
        let words = t!(i18n, ids::BOM_PROPERTIES).into_string();
        // raw: catalog text already rendered, then upper-cased for the locale.
        let heading = text(raw(i18n.upper_for_display(&words)))
            .size(TS.section)
            .style(crate::styles::section_label(tokens));
        let top = column![
            heading,
            segmented(tabs, preview.sidebar_tab, segmented_style(tokens), |tab| {
                Message::BomPreview(BomPreviewMsg::SetSidebarTab(tab))
            }),
        ]
        .spacing(SP.s)
        .padding(iced::Padding {
            top: 10.0,
            right: SP.l,
            bottom: SP.s,
            left: SP.l,
        });
        let pane: Element<'_, Message> = match preview.sidebar_tab {
            BomSidebarTab::General => general_pane(preview, i18n, tokens),
            BomSidebarTab::Columns => columns_pane(preview, i18n, tokens),
            BomSidebarTab::Rules => self.bom_rules_pane(),
        };
        column![
            top,
            scrollable(container(pane).padding(iced::Padding {
                top: 0.0,
                right: SP.l,
                bottom: SP.l,
                left: SP.l,
            }))
            .direction(crate::styles::thin_vertical())
            .style(crate::styles::scrollbar_style(tokens))
            .width(Length::Fill)
            .height(Length::Fill),
        ]
        .spacing(0)
        .boxed()
    }
}

/// General: BOM Items (the one not-fitted option) and Export Options (the
/// format).
fn general_pane<'a>(
    preview: &BomPreviewState,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let include = preview.includes_not_fitted();
    let formats: [(BomFormat, Label<'static>); 3] = [
        (BomFormat::Csv, t!(i18n, ids::BOM_FORMAT_CSV).into()),
        (BomFormat::Xlsx, t!(i18n, ids::BOM_FORMAT_XLSX).into()),
        (BomFormat::Html, t!(i18n, ids::BOM_FORMAT_HTML).into()),
    ];
    let format_row = row![
        text(t!(i18n, ids::BOM_FORMAT))
            .size(TS.body)
            .color(resolve(tokens).ink3)
            .width(FORM_KEY_W),
        segmented(
            formats,
            preview.options.format,
            segmented_style(tokens),
            |f| Message::BomPreview(BomPreviewMsg::SetFormat(f)),
        ),
    ]
    .spacing(SP.m)
    .align_y(iced::Alignment::Center);
    column![
        section(i18n, ids::BOM_SECTION_ITEMS, None, true, tokens),
        check_row(
            t!(i18n, ids::BOM_INCLUDE_NOT_FITTED),
            include,
            Message::BomPreview(BomPreviewMsg::NotFittedIncludedToggled(!include)),
            tokens,
        ),
        section(i18n, ids::BOM_SECTION_EXPORT, None, false, tokens),
        format_row,
    ]
    .spacing(0)
    .boxed()
}

/// Columns: the exported (ticked) columns in their order, then the others;
/// a ticked row's grip drags it to a new place, a click ticks or clears it.
fn columns_pane<'a>(
    preview: &BomPreviewState,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let r = resolve(tokens);
    let ink = ti(tokens.text);
    // Custom-field columns found in the rolled-up rows.
    let mut custom_keys: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    for row in &preview.table.rows {
        custom_keys.extend(row.custom.keys().map(String::as_str));
    }
    let name_of = |col: &BomColumn| -> Label<'static> {
        match COLUMN_OPTIONS.iter().find(|(c, _)| c == col) {
            Some((_, id)) => t!(i18n, *id).into(),
            None => match col {
                BomColumn::Custom(key) => UserText::new(key.clone()).into(),
                // raw: a column name from signex-output (#2060).
                other => raw(other.header().to_string()),
            },
        }
    };
    let mut rows: Vec<Element<'a, Message>> = Vec::new();
    for (idx, col) in preview.options.columns.iter().enumerate() {
        rows.push(column_row(
            preview,
            name_of(col),
            Some(idx),
            col.clone(),
            ink,
            tokens,
        ));
    }
    let unticked = COLUMN_OPTIONS
        .iter()
        .map(|(col, _)| col.clone())
        .chain(
            custom_keys
                .iter()
                .map(|k| BomColumn::Custom((*k).to_owned())),
        )
        .filter(|col| !preview.options.columns.contains(col));
    for col in unticked {
        rows.push(column_row(
            preview,
            name_of(&col),
            None,
            col,
            r.ink4,
            tokens,
        ));
    }
    column![
        Space::new().height(SP.m),
        caption(t!(i18n, ids::BOM_COLUMNS_HINT), tokens),
        Space::new().height(SP.s),
        card_list(rows, tokens),
    ]
    .spacing(0)
    .boxed()
}

/// One Columns-tab row. `slot` is the column's place among the exported
/// ones (`None`: not exported). A press on the grip arms the same drag the
/// header uses; releasing over another exported row drops it there.
fn column_row<'a>(
    preview: &BomPreviewState,
    name: Label<'static>,
    slot: Option<usize>,
    col: BomColumn,
    ink: Color,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let r = resolve(tokens);
    let grip: Element<'a, Message> = match slot {
        Some(idx) => mouse_area(
            // raw: a grip glyph, not language.
            text(raw("\u{283F}")).size(TS.body).color(r.ink4),
        )
        .on_press(Message::BomPreview(BomPreviewMsg::ColumnDragStart(idx)))
        .interaction(iced::mouse::Interaction::Grab)
        .boxed(),
        None => Space::new().width(TS.body).boxed(),
    };
    let tick = button(
        row![
            checkbox_pip(tokens, slot.is_some()),
            text(name).size(TS.body).color(ink).width(Length::Fill),
        ]
        .spacing(SP.m)
        .align_y(iced::Alignment::Center),
    )
    .on_press(Message::BomPreview(BomPreviewMsg::ToggleColumn(col)))
    .padding(0)
    .width(Length::Fill)
    .style(move |_: &Theme, _: button::Status| button::Style {
        text_color: ink,
        ..button::Style::default()
    });
    let line = r.line_soft;
    let body = container(
        row![grip, tick]
            .spacing(SP.m)
            .align_y(iced::Alignment::Center),
    )
    .padding([0.0, SP.m])
    .width(Length::Fill)
    .height(COLUMN_ROW_H)
    .align_y(iced::alignment::Vertical::Center);
    let rule = container(Space::new())
        .width(Length::Fill)
        .height(HAIRLINE)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(line)),
            ..container::Style::default()
        });
    let item = column![body, rule];
    match (slot, preview.column_drag) {
        // A drag is armed: an exported row takes the dragged column.
        (Some(idx), Some(_)) => mouse_area(item)
            .on_release(Message::BomPreview(BomPreviewMsg::ColumnDragDrop(idx)))
            .boxed(),
        _ => item.boxed(),
    }
}
