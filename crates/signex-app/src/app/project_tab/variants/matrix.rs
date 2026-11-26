//! The Variant Manager's parts matrix (#1293): Part, Value · Footprint, then
//! a column per variant (Base first, the active one marked). A cell that
//! matches Base stays quiet; a difference is coloured, and a cell with a
//! parameter value of its own shows that value (mockup row 32). A click on
//! a cell selects it and the right pane edits it. The parts filter and Only
//! rows that differ hide rows; every remaining row is listed.

use iced::widget::{Column, Row, Space, column, container, mouse_area, row, scrollable};
use iced::{Alignment, Background, Border, Color, Element, Length, Theme, Widget as _};
use signex_i18n::{Localized, ids, t};
use signex_output::{VariantMatrix, VariantRow};
use signex_types::project::{FOOTPRINT_PARAMETER, MPN_PARAMETER, VALUE_PARAMETER};
use signex_widgets::text;

use super::super::ProjectTabMsg;
use super::super::general::tab_message;
use super::super::look::with_tip;
use super::super::table::{SP, TS};
use super::{
    GLYPH_CHECK, GLYPH_CROSS, Manager, VariantCell, VariantManagerMsg, glyph, medium, mono, pill,
    tint_over_panel,
};
use crate::app::Message;
use crate::styles::tokens::resolve;
use crate::styles::{PillTone, Radii, ti};

/// Width of the Part column, in `body`-size characters.
pub(super) const PART_EMS: f32 = 58.0 / 12.0;
/// Width of the Value · Footprint column, in `body`-size characters.
pub(super) const VALUE_EMS: f32 = 150.0 / 12.0;
/// Height of a matrix row, in `body`-size characters.
pub(super) const ROW_EMS: f32 = 30.0 / 12.0;
/// Height of the heading row, in `body`-size characters.
const HEAD_EMS: f32 = 28.0 / 12.0;
/// Height of a cell pill, in `body`-size characters.
pub(super) const CELL_EMS: f32 = 20.0 / 12.0;
/// Strength of the accent fill of the active column.
pub(super) const ACTIVE_FILL: f32 = 0.22;
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
    // A part on a repeated sheet is one row where its first copy is listed,
    // open into the copies the filters keep (#2662).
    let mut listed: Vec<&signex_types::pcb::SymbolLink> = Vec::new();
    for part in &shown {
        let Some(channel) = part.channel.as_ref() else {
            body = body.push(part_row(m, part));
            continue;
        };
        if listed.contains(&&channel.part) {
            continue;
        }
        listed.push(&channel.part);
        body = body.push(super::channel_rows::part_row(m, matrix, channel, part));
        if m.is_channel_part_open(&channel.part) {
            let is_copy = |row: &&&VariantRow| {
                row.channel
                    .as_ref()
                    .is_some_and(|other| other.part == channel.part)
            };
            for copy in shown.iter().filter(is_copy) {
                body = body.push(part_row(m, copy));
            }
        }
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
    for variant in m.variants() {
        line = line.push(variant_cell(m, part, variant));
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

/// `part` in `variant`'s column: its state, on the active column's fill,
/// outlined in the accent while selected; a click selects it.
fn variant_cell<'a>(
    m: &Manager<'_>,
    part: &VariantRow,
    variant: Option<&str>,
) -> Element<'a, Message> {
    let r = resolve(m.tokens);
    let Some(column) = m.column(variant) else {
        return body_cell(Space::new().boxed(), Length::Fill, None);
    };
    let name = variant.unwrap_or_default();
    let face = match own_value(m, part, variant, column) {
        Some(OwnValue::Alternate(mpn)) => with_tip(
            alternate_pill(m, mpn),
            t!(m.i18n, ids::VARIANT_MANAGER_ALTERNATE_TIP, "variant" => name, "mpn" => mpn),
            m.tokens,
        ),
        Some(OwnValue::Value(value)) => with_tip(
            value_pill(m, value, ids::VARIANT_MANAGER_OVERRIDE_VALUE),
            t!(m.i18n, ids::VARIANT_MANAGER_VALUE_TIP, "variant" => name),
            m.tokens,
        ),
        Some(OwnValue::Other(value, kind)) => value_pill(m, value, kind),
        None => cell_pill(
            m,
            part.differs_at(column),
            !part.fitted.get(column).copied().unwrap_or(true),
            false,
        ),
    };
    let fill = m
        .is_active(variant)
        .then(|| tint_over_panel(r.accent_soft, ACTIVE_FILL, m.tokens));
    let cell = VariantCell {
        variant: variant.map(str::to_string),
        designator: part.designator.clone(),
    };
    let is_selected = m.cell() == Some(&cell);
    let edge = if is_selected {
        r.accent_text
    } else {
        Color::TRANSPARENT
    };
    let boxed = container(body_cell(face, Length::Fill, fill))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_: &Theme| container::Style {
            border: Border {
                width: ACTIVE_RULE,
                color: edge,
                radius: Radii::GRAPHITE.small.into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    let project_id = m.project_id;
    mouse_area(boxed)
        .on_press(tab_message(ProjectTabMsg::VariantManager {
            project_id,
            msg: VariantManagerMsg::CellSelected(cell),
        }))
        .interaction(iced::mouse::Interaction::Pointer)
        .boxed()
}

/// What a cell with a value of its own shows.
enum OwnValue<'s> {
    /// An alternate part (mockup row 208): its part number.
    Alternate(&'s str),
    /// A value of its own (mockup row 209).
    Value(&'s str),
    /// A footprint or a field, with the kind of value it replaces.
    Other(&'s str, signex_i18n::MessageId),
}

/// The value `variant` gives `part` instead of its own, shown in its cell:
/// an alternate part's number first, then the value, then the footprint,
/// then a field. Base, a cell without one and a part not fitted in
/// `column` (it shows Not fitted) have none.
fn own_value<'s>(
    m: &'s Manager<'_>,
    part: &VariantRow,
    variant: Option<&str>,
    column: usize,
) -> Option<OwnValue<'s>> {
    if !part.is_overridden_at(column) {
        return None;
    }
    let set = &m.own(variant, part)?.parameters;
    if let Some(mpn) = set.get(MPN_PARAMETER) {
        return Some(OwnValue::Alternate(mpn));
    }
    if let Some(value) = set.get(VALUE_PARAMETER) {
        return Some(OwnValue::Value(value));
    }
    if let Some(value) = set.get(FOOTPRINT_PARAMETER) {
        return Some(OwnValue::Other(
            value,
            ids::VARIANT_MANAGER_OVERRIDE_FOOTPRINT,
        ));
    }
    let (_, value) = set.iter().next()?;
    Some(OwnValue::Other(value, ids::VARIANT_MANAGER_OVERRIDE_FIELD))
}

/// A cell that uses an alternate part (mockup row 208): its part number in
/// the mono face on the info tint.
fn alternate_pill<'a>(m: &Manager<'_>, mpn: &str) -> Element<'a, Message> {
    let r = resolve(m.tokens);
    let face = text(signex_i18n::UserText::new(mpn.to_string()))
        .size(TS.caption)
        .font(mono())
        .color(r.info)
        .wrapping(iced::widget::text::Wrapping::None);
    let fill = r.info_soft;
    container(face)
        .height(Length::Fixed(TS.body * CELL_EMS))
        .padding([0.0, SP.m * 0.75])
        .align_y(iced::alignment::Vertical::Center)
        .clip(true)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                radius: Radii::GRAPHITE.small.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// A cell with a value of its own: the value in the mono face and, after
/// it, the kind of value it replaces, on the accent tint.
fn value_pill<'a>(
    m: &Manager<'_>,
    value: &str,
    kind: signex_i18n::MessageId,
) -> Element<'a, Message> {
    let r = resolve(m.tokens);
    let face = row![
        text(signex_i18n::UserText::new(value.to_string()))
            .size(TS.caption)
            .font(mono())
            .color(r.accent_soft_ink)
            .wrapping(iced::widget::text::Wrapping::None),
        text(t!(m.i18n, kind)).size(TS.caption).color(r.ink4),
    ]
    .spacing(SP.xs)
    .align_y(Alignment::Center);
    let fill = r.accent_soft;
    container(face)
        .height(Length::Fixed(TS.body * CELL_EMS))
        .padding([0.0, SP.m * 0.75])
        .align_y(iced::alignment::Vertical::Center)
        .clip(true)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                radius: Radii::GRAPHITE.small.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// One body cell, `width` wide, on `fill` when given.
pub(super) fn body_cell<'a>(
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
pub(super) fn cell_pill<'a>(
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
