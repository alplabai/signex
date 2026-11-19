//! The Variant Manager's parts matrix (#1293): Part, Value · Footprint, then
//! a column per variant (Base first, the active one marked). A cell that
//! matches Base stays quiet; a difference is coloured. Read-only: changing a
//! cell has no surface yet. The parts filter and Only rows that differ hide
//! rows; every remaining row is listed.

use iced::widget::{Column, Row, Space, column, container, row, scrollable};
use iced::{Alignment, Background, Border, Color, Element, Length, Theme, Widget as _};
use signex_i18n::{Localized, ids, t};
use signex_output::{VariantMatrix, VariantRow};
use signex_widgets::text;

use super::super::table::{SP, TS};
use super::{GLYPH_CHECK, GLYPH_CROSS, Manager, glyph, medium, mono, pill, tint_over_panel};
use crate::app::Message;
use crate::styles::tokens::resolve;
use crate::styles::{PillTone, Radii, ti};

/// Width of the Part column, in `body`-size characters.
const PART_EMS: f32 = 58.0 / 12.0;
/// Width of the Value · Footprint column, in `body`-size characters.
const VALUE_EMS: f32 = 150.0 / 12.0;
/// Height of a matrix row, in `body`-size characters.
const ROW_EMS: f32 = 30.0 / 12.0;
/// Height of the heading row, in `body`-size characters.
const HEAD_EMS: f32 = 28.0 / 12.0;
/// Height of a cell pill, in `body`-size characters.
const CELL_EMS: f32 = 20.0 / 12.0;
/// Strength of the accent fill of the active column.
const ACTIVE_FILL: f32 = 0.22;
/// Width of the active column's underline, in px.
const ACTIVE_RULE: f32 = 2.0;

/// The rows that pass the filter and Only rows that differ.
pub(super) fn shown_rows<'m>(
    matrix: &'m VariantMatrix,
    filter: &str,
    only_differ: bool,
) -> Vec<&'m VariantRow> {
    matrix
        .rows
        .iter()
        .filter(|row| row.matches(filter))
        .filter(|row| !only_differ || row.differs())
        .collect()
}

/// The heading row, the rows in a scroller, the hidden-rows caption and the
/// legend.
pub(super) fn view<'a>(m: &Manager<'_>, matrix: &VariantMatrix) -> Element<'a, Message> {
    let r = resolve(m.tokens);
    let filter = m.tab.map_or("", |tab| tab.variant_filter.as_str());
    let only_differ = m.tab.is_some_and(|tab| tab.variants_only_differ);
    let shown = shown_rows(matrix, filter, only_differ);
    let mut body: Column<Element<'a, Message>> = Column::new();
    for part in &shown {
        body = body.push(part_row(m, part));
    }
    if only_differ {
        let differ = matrix.differing_rows();
        body = body.push(caption_row(
            t!(
                m.i18n,
                ids::PROJECT_VARIANTS_HIDDEN_ROWS,
                "differ" => differ,
                "total" => matrix.rows.len(),
                "hidden" => matrix.rows.len() - differ,
            ),
            m,
        ));
    } else if matrix.rows.is_empty() {
        body = body.push(caption_row(t!(m.i18n, ids::PROJECT_VARIANTS_NO_PARTS), m));
    } else if shown.is_empty() {
        body = body.push(caption_row(t!(m.i18n, ids::PROJECT_VARIANTS_NO_MATCH), m));
    }
    let edge = ti(m.tokens.border);
    let table = container(column![
        heading(m),
        scrollable(body)
            .direction(crate::styles::thin_vertical())
            .style(crate::styles::scrollbar_style(m.tokens))
            .height(Length::Fill),
    ])
    .width(Length::Fill)
    .height(Length::Fill)
    .clip(true)
    .style(move |_: &Theme| container::Style {
        border: Border {
            width: 1.0,
            color: edge,
            radius: Radii::GRAPHITE.button.into(),
            ..Border::default()
        },
        ..container::Style::default()
    });
    let legend = row![
        cell_pill(m, false, false, true),
        cell_pill(m, false, true, true),
        text(t!(m.i18n, ids::PROJECT_VARIANTS_LEGEND))
            .size(TS.caption)
            .color(r.ink3),
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center);
    column![table, legend]
        .spacing(SP.m)
        .height(Length::Fill)
        .boxed()
}

/// The columns' captions on the sunken strip; the active variant's column is
/// marked "active" and underlined in the accent.
fn heading<'a>(m: &Manager<'_>) -> Element<'a, Message> {
    let r = resolve(m.tokens);
    let caption = |id: signex_i18n::MessageId, ink: Color| {
        text(signex_i18n::UserText::new(
            m.i18n.upper_for_display(&m.i18n.message_plain(id, &[])),
        ))
        .size(TS.section)
        .font(medium())
        .color(ink)
        .wrapping(iced::widget::text::Wrapping::None)
    };
    let mut line: Row<Element<'a, Message>> = row![
        head_cell(
            caption(ids::PROJECT_VARIANTS_COL_PART, r.ink3).boxed(),
            Length::Fixed(TS.body * PART_EMS),
            None,
        ),
        head_cell(
            caption(ids::PROJECT_VARIANTS_COL_VALUE, r.ink3).boxed(),
            Length::Fixed(TS.body * VALUE_EMS),
            None,
        ),
    ];
    for variant in m.variants() {
        let is_active = m.is_active(variant);
        let ink = if is_active { r.accent_soft_ink } else { r.ink3 };
        let name = text(signex_i18n::UserText::new(
            m.i18n.upper_for_display(&m.name(variant)),
        ))
        .size(TS.section)
        .font(medium())
        .color(ink)
        .wrapping(iced::widget::text::Wrapping::None);
        let mut face = row![name].spacing(SP.s).align_y(Alignment::Center);
        if is_active {
            face = face.push(pill(
                t!(m.i18n, ids::PROJECT_VARIANTS_ACTIVE),
                PillTone::Accent,
                m.tokens,
            ));
        }
        line = line.push(head_cell(
            face.boxed(),
            Length::Fill,
            is_active.then_some(r.accent_text),
        ));
    }
    let sunken = r.sunken;
    let side = iced::border::Side::default().width(1.0).color(r.line_soft);
    container(line)
        .width(Length::Fill)
        .height(Length::Fixed(TS.body * HEAD_EMS))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(sunken)),
            border: Border::default().bottom(side),
            ..container::Style::default()
        })
        .boxed()
}

/// One heading cell, `width` wide; `underline` draws the active rule.
fn head_cell<'a>(
    face: Element<'a, Message>,
    width: Length,
    underline: Option<Color>,
) -> Element<'a, Message> {
    let side = iced::border::Side::default()
        .width(ACTIVE_RULE)
        .color(underline.unwrap_or(Color::TRANSPARENT));
    container(face)
        .width(width)
        .height(Length::Fill)
        .padding([0.0, SP.m])
        .align_y(iced::alignment::Vertical::Center)
        .clip(true)
        .style(move |_: &Theme| container::Style {
            border: if underline.is_some() {
                Border::default().bottom(side)
            } else {
                Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// One part: designator, value · footprint, and its cell in every column.
fn part_row<'a>(m: &Manager<'_>, part: &VariantRow) -> Element<'a, Message> {
    let r = resolve(m.tokens);
    let designator = text(signex_i18n::UserText::new(part.designator.clone()))
        .size(TS.body)
        .font(super::weighted_mono())
        .color(ti(m.tokens.text))
        .wrapping(iced::widget::text::Wrapping::None);
    let mut value: Row<Element<'a, Message>> = row![
        text(signex_i18n::UserText::new(part.value.clone()))
            .size(TS.body)
            .font(mono())
            .color(ti(m.tokens.text_secondary))
            .wrapping(iced::widget::text::Wrapping::None),
    ]
    .spacing(SP.s);
    if !part.footprint.is_empty() {
        value = value.push(
            text(t!(
                m.i18n,
                ids::PROJECT_VARIANTS_FOOTPRINT,
                "footprint" => part.footprint.as_str(),
            ))
            .size(TS.body)
            .font(mono())
            .color(r.ink4)
            .wrapping(iced::widget::text::Wrapping::None)
            .boxed(),
        );
    }
    let mut line: Row<Element<'a, Message>> = row![
        body_cell(designator.boxed(), Length::Fixed(TS.body * PART_EMS), None),
        body_cell(value.boxed(), Length::Fixed(TS.body * VALUE_EMS), None),
    ];
    let active_fill = tint_over_panel(r.accent_soft, ACTIVE_FILL, m.tokens);
    for variant in m.variants() {
        let face = match m.column(variant) {
            Some(column) => cell_pill(
                m,
                part.differs_at(column),
                !part.fitted.get(column).copied().unwrap_or(true),
                false,
            ),
            None => Space::new().boxed(),
        };
        let fill = m.is_active(variant).then_some(active_fill);
        line = line.push(body_cell(face, Length::Fill, fill));
    }
    let side = iced::border::Side::default().width(1.0).color(r.line_soft);
    container(line.align_y(Alignment::Center))
        .width(Length::Fill)
        .height(Length::Fixed(TS.body * ROW_EMS))
        .style(move |_: &Theme| container::Style {
            border: Border::default().bottom(side),
            ..container::Style::default()
        })
        .boxed()
}

/// One body cell, `width` wide, on `fill` when given.
fn body_cell<'a>(
    face: Element<'a, Message>,
    width: Length,
    fill: Option<Color>,
) -> Element<'a, Message> {
    container(face)
        .width(width)
        .height(Length::Fill)
        .padding([0.0, SP.m])
        .align_y(iced::alignment::Vertical::Center)
        .clip(true)
        .style(move |_: &Theme| container::Style {
            background: fill.map(Background::Color),
            ..container::Style::default()
        })
        .boxed()
}

/// A cell's state: Fitted or Not fitted, coloured when it `differs` from
/// Base (or always, in the `legend`).
fn cell_pill<'a>(
    m: &Manager<'_>,
    differs: bool,
    not_fitted: bool,
    legend: bool,
) -> Element<'a, Message> {
    let r = resolve(m.tokens);
    let loud = differs || legend;
    let (words, mark) = if not_fitted {
        (t!(m.i18n, ids::PROJECT_VARIANTS_NOT_FITTED), GLYPH_CROSS)
    } else {
        (t!(m.i18n, ids::PROJECT_VARIANTS_FITTED), GLYPH_CHECK)
    };
    let (fill, ink) = match (loud, not_fitted) {
        (false, _) => (None, r.ink4),
        (true, true) => (Some(r.err_soft), ti(m.tokens.error)),
        (true, false) if legend => (None, r.ink4),
        (true, false) => (Some(r.ok_soft), ti(m.tokens.success)),
    };
    let face = row![
        glyph(mark, TS.caption, ink),
        text(words)
            .size(TS.caption)
            .color(ink)
            .font(if fill.is_some() {
                medium()
            } else {
                super::weighted(iced::font::Weight::Normal)
            }),
    ]
    .spacing(SP.xs)
    .align_y(Alignment::Center);
    container(face)
        .height(Length::Fixed(TS.body * CELL_EMS))
        .padding([0.0, if fill.is_some() { SP.m * 0.75 } else { SP.xs }])
        .align_y(iced::alignment::Vertical::Center)
        .style(move |_: &Theme| container::Style {
            background: fill.map(Background::Color),
            border: Border {
                radius: Radii::GRAPHITE.small.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// A muted caption across the matrix, after its rows.
fn caption_row<'a>(words: Localized, m: &Manager<'_>) -> Element<'a, Message> {
    container(text(words).size(TS.caption).color(resolve(m.tokens).ink4))
        .width(Length::Fill)
        .padding([SP.m, SP.m])
        .boxed()
}
