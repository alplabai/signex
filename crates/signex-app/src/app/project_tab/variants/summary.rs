//! The Variant Manager's summary cards (#1293): per variant the parts it
//! fits, the cost (no price source yet: "—") and its changes against Base;
//! the active variant's card is highlighted. Four cards to a row.

use iced::widget::{Column, Row, Space, column, container, row};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget as _};
use signex_i18n::{Localized, ids, t};
use signex_widgets::text;

use super::super::table::{SP, TS};
use super::{Manager, mono, pill, semibold, tint_over_panel};
use crate::app::Message;
use crate::styles::tokens::resolve;
use crate::styles::{PillTone, Radii, ti};

/// Cards in one row.
const PER_ROW: usize = 4;
/// Strength of the accent fill of the active card.
const ACTIVE_FILL: f32 = 0.35;

/// Every variant's card, in rows of four.
pub(super) fn view<'a>(m: &Manager<'_>) -> Element<'a, Message> {
    let variants: Vec<Option<&str>> = m.variants().collect();
    let mut rows: Column<Element<'a, Message>> = Column::new().spacing(SP.m);
    for chunk in variants.chunks(PER_ROW) {
        let mut line: Row<Element<'a, Message>> = Row::new().spacing(SP.m);
        for &variant in chunk {
            line = line.push(card(m, variant, Length::Fill));
        }
        for _ in chunk.len()..PER_ROW {
            line = line.push(Space::new().width(Length::Fill).boxed());
        }
        rows = rows.push(line.boxed());
    }
    rows.boxed()
}

/// One variant's card, `width` wide.
pub(super) fn card<'a>(
    m: &Manager<'_>,
    variant: Option<&str>,
    width: Length,
) -> Element<'a, Message> {
    let r = resolve(m.tokens);
    let is_active = m.is_active(variant);
    let mut head: Row<Element<'a, Message>> = row![
        container(
            text(signex_i18n::UserText::new(m.name(variant)))
                .size(TS.body)
                .font(semibold())
                .wrapping(iced::widget::text::Wrapping::None)
        )
        .width(Length::Fill)
        .clip(true),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center);
    if is_active {
        head = head.push(pill(
            t!(m.i18n, ids::PROJECT_VARIANTS_ACTIVE),
            PillTone::Accent,
            m.tokens,
        ));
    }
    let column = m.column(variant);
    let fitted = m.matrix().zip(column).map(|(matrix, column)| {
        t!(
            m.i18n,
            ids::PROJECT_VARIANTS_CARD_FITTED,
            "fitted" => matrix.fitted_count(column),
            "total" => matrix.rows.len(),
        )
    });
    let vs_base = match (variant, m.matrix().zip(column)) {
        (None, _) => Some((t!(m.i18n, ids::PROJECT_VARIANTS_REFERENCE), true)),
        (Some(_), Some((matrix, column))) => Some((
            t!(m.i18n, ids::PROJECT_VARIANTS_CHANGES, "changes" => matrix.changes(column)),
            false,
        )),
        (Some(_), None) => None,
    };
    let none = || t!(m.i18n, ids::PROJECT_VARIANTS_NO_VALUE);
    let facts = column![
        fact(
            m,
            t!(m.i18n, ids::PROJECT_VARIANTS_CARD_PARTS),
            fitted.unwrap_or_else(none),
            false
        ),
        fact(m, t!(m.i18n, ids::PROJECT_VARIANTS_CARD_COST), none(), true),
        match vs_base {
            Some((words, quiet)) => fact(
                m,
                t!(m.i18n, ids::PROJECT_VARIANTS_CARD_VS_BASE),
                words,
                quiet
            ),
            None => fact(
                m,
                t!(m.i18n, ids::PROJECT_VARIANTS_CARD_VS_BASE),
                none(),
                true
            ),
        },
    ]
    .spacing(SP.xs / 2.0);
    let (fill, edge, edge_width) = if is_active {
        (
            tint_over_panel(r.accent_soft, ACTIVE_FILL, m.tokens),
            r.accent_text,
            1.5,
        )
    } else {
        (ti(m.tokens.panel_bg), ti(m.tokens.border), 1.0)
    };
    container(column![head, facts].spacing(SP.s))
        .width(width)
        .padding([SP.s * 1.5, SP.m])
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                width: edge_width,
                color: edge,
                radius: Radii::GRAPHITE.button.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// One caption / value line of a card; a `quiet` value is muted, not mono.
fn fact<'a>(
    m: &Manager<'_>,
    caption: Localized,
    value: Localized,
    quiet: bool,
) -> Element<'a, Message> {
    let r = resolve(m.tokens);
    let value = text(value)
        .size(TS.caption)
        .wrapping(iced::widget::text::Wrapping::None);
    let value = if quiet {
        value.color(r.ink4)
    } else {
        value.font(mono()).color(ti(m.tokens.text))
    };
    row![
        text(caption).size(TS.caption).color(r.ink4),
        Space::new().width(Length::Fill),
        value,
    ]
    .spacing(SP.m)
    .boxed()
}
