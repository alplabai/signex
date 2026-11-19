//! The "Not stored yet" group (#2523): the rows of settings the board does
//! not store yet, gathered in one dashed box that starts closed, instead of
//! greyed rows spread through the sections. Each row keeps its tooltip with
//! the reason.

use iced::widget::{Column, column, container, row};
use iced::{Border, Element, Length, Theme, Widget as _};
use signex_i18n::{ids, t};
use signex_widgets::{button, text};

use super::rows::{Look, PAD_X};
use super::{PcbPrimitiveMsg, PrimitiveCommon, SP, TS};
use crate::panels::PanelMsg;
use crate::styles::Radii;
use crate::styles::tokens::resolve;

/// The dashed box holding `rows` under a header that counts them; closed
/// unless the panel opened it. `None` when there is no row.
pub(super) fn group<'a>(
    look: Look<'a>,
    common: &'a PrimitiveCommon,
    rows: Vec<Element<'a, PanelMsg>>,
) -> Option<Element<'a, PanelMsg>> {
    if rows.is_empty() {
        return None;
    }
    let r = resolve(look.tokens());
    let (faint, line) = (r.ink4, r.line_strong);
    let is_open = common.unstored_open;
    let label = t!(look.ctx.i18n, ids::PCB_PRIM_UNSTORED, "count" => rows.len());
    let header = button(
        container(
            row![
                signex_widgets::chevron::chevron(is_open, TS.caption, faint),
                text(label).size(TS.caption).color(faint),
            ]
            .spacing(SP.s)
            .align_y(iced::Alignment::Center),
        )
        .padding([SP.s, SP.m])
        .width(Length::Fill),
    )
    .padding(0)
    .width(Length::Fill)
    .on_press(PanelMsg::PcbPrimitive(PcbPrimitiveMsg::UnstoredToggled))
    .style(|_: &Theme, _| iced::widget::button::Style::default());
    let mut body: Column<Element<'a, PanelMsg>> = column![header.boxed()].spacing(SP.xs);
    if is_open {
        for r in rows {
            body = body.push(r);
        }
    }
    let boxed = container(body)
        .padding(iced::Padding {
            bottom: if is_open { SP.xs } else { 0.0 },
            ..iced::Padding::ZERO
        })
        .width(Length::Fill)
        .style(move |_: &Theme| iced::widget::container::Style {
            border: Border {
                width: crate::styles::HAIRLINE,
                color: line,
                radius: Radii::GRAPHITE.button.into(),
                ..Border::default()
            },
            ..iced::widget::container::Style::default()
        });
    Some(
        container(boxed)
            .padding([SP.xs, PAD_X])
            .width(Length::Fill)
            .boxed(),
    )
}
