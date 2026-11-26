//! The matrix row of a part on a repeated sheet and the right pane's
//! every-channel fit (#2662, mockup row 32 "Per-channel variant state"). The
//! row stands for every channel's copy: the part's designator on its sheet
//! with a fold chevron, its value and the number of channels, and per
//! variant Fitted, Not fitted or "mixed" when the channels disagree. A click
//! opens it into one row per copy, each set on its own.

use iced::widget::{Space, column, container, mouse_area, row};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget as _};
use signex_i18n::{UserText, ids, t};
use signex_output::{ChannelFit, RowChannel, VariantMatrix, VariantRow};
use signex_widgets::text;

use super::super::ProjectTabMsg;
use super::super::general::tab_message;
use super::super::look::{ButtonKind, field_label, text_button};
use super::super::table::{SP, TS};
use super::columns::Columns;
use super::matrix::{ACTIVE_FILL, CELL_EMS, ROW_EMS, body_cell, cell_pill};
use super::{Manager, VariantCell, VariantManagerMsg, medium, mono, tint_over_panel};
use crate::app::Message;
use crate::styles::tokens::resolve;
use crate::styles::{Radii, ti};

/// The row of the part `channel` names, which `first` (its first copy
/// listed) gives the value of.
pub(super) fn part_row<'a>(
    m: &Manager<'_>,
    columns: &Columns,
    matrix: &VariantMatrix,
    channel: &RowChannel,
    first: &VariantRow,
) -> Element<'a, Message> {
    let r = resolve(m.tokens);
    let copies = matrix.channel_copies(&channel.part).len();
    let open = m.is_channel_part_open(&channel.part);
    let designator = row![
        signex_widgets::chevron::chevron(open, TS.body, r.ink3),
        text(UserText::new(channel.designator.clone()))
            .size(TS.body)
            .font(super::weighted_mono())
            .color(ti(m.tokens.text))
            .wrapping(iced::widget::text::Wrapping::None),
    ]
    .spacing(SP.xs)
    .align_y(Alignment::Center);
    let value = row![
        text(UserText::new(first.value.clone()))
            .size(TS.body)
            .font(mono())
            .color(ti(m.tokens.text_secondary))
            .wrapping(iced::widget::text::Wrapping::None),
        text(t!(m.i18n, ids::PROJECT_VARIANTS_CHANNEL_COPIES, "count" => copies))
            .size(TS.caption)
            .color(r.ink4)
            .wrapping(iced::widget::text::Wrapping::None),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center);
    let mut line = row![
        body_cell(designator.boxed(), Columns::part(), None),
        body_cell(value.boxed(), Columns::value(), None),
    ];
    let base = matrix.channel_fit(&channel.part, 0);
    for (index, variant) in m.variants().enumerate() {
        let width = columns.variant(index);
        line = line.push(fit_cell(m, matrix, channel, variant, base, width));
    }
    let side = iced::border::Side::default().width(1.0).color(r.line_soft);
    let boxed = container(line.align_y(Alignment::Center))
        .width(Length::Fill)
        .height(Length::Fixed(TS.body * ROW_EMS))
        .style(move |_: &Theme| container::Style {
            border: Border::default().bottom(side),
            ..container::Style::default()
        });
    let project_id = m.project_id;
    mouse_area(boxed)
        .on_press(tab_message(ProjectTabMsg::VariantManager {
            project_id,
            msg: VariantManagerMsg::ChannelPartToggled {
                part: channel.part.clone(),
            },
        }))
        .interaction(iced::mouse::Interaction::Pointer)
        .boxed()
}

/// How every copy fits in `variant`'s column, coloured where it differs
/// from Base, on the active column's fill.
fn fit_cell<'a>(
    m: &Manager<'_>,
    matrix: &VariantMatrix,
    channel: &RowChannel,
    variant: Option<&str>,
    base: ChannelFit,
    width: Length,
) -> Element<'a, Message> {
    let r = resolve(m.tokens);
    let Some(column) = m.column(variant) else {
        return body_cell(Space::new().boxed(), width, None);
    };
    let fit = matrix.channel_fit(&channel.part, column);
    let differs = column > 0 && fit != base;
    let face = match fit {
        ChannelFit::Fitted => cell_pill(m, differs, false, false),
        ChannelFit::NotFitted => cell_pill(m, differs, true, false),
        ChannelFit::Mixed { not_fitted, of } => mixed_pill(m, not_fitted, of),
    };
    let fill = m
        .is_active(variant)
        .then(|| tint_over_panel(r.accent_soft, ACTIVE_FILL, m.tokens));
    body_cell(face, width, fill)
}

/// "mixed · 1 of 3 not fitted" on the warning tint.
fn mixed_pill<'a>(m: &Manager<'_>, not_fitted: usize, of: usize) -> Element<'a, Message> {
    let r = resolve(m.tokens);
    let (fill, ink) = (r.warn_soft, ti(m.tokens.warning));
    container(
        text(t!(
            m.i18n,
            ids::PROJECT_VARIANTS_CHANNEL_MIXED,
            "count" => not_fitted,
            "total" => of,
        ))
        .size(TS.caption)
        .font(medium())
        .color(ink)
        .wrapping(iced::widget::text::Wrapping::None),
    )
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

/// The right pane's every-channel fit for a channel's copy in a variant:
/// fit or leave out every channel's copy at once, or let the sheets decide
/// for each. `None` for a part on a plain sheet, Base's cell, or while the
/// settings are locked.
pub(super) fn every_channel<'a>(
    m: &Manager<'_>,
    cell: &VariantCell,
    matrix: &VariantMatrix,
    part: &VariantRow,
) -> Option<Element<'a, Message>> {
    let channel = part.channel.as_ref()?;
    let variant = cell.variant.clone()?;
    if m.settings_locked {
        return None;
    }
    let copies = matrix.channel_copies(&channel.part).len();
    let project_id = m.project_id;
    let pick = |fitted: Option<bool>| {
        tab_message(ProjectTabMsg::VariantManager {
            project_id,
            msg: VariantManagerMsg::ChannelPartFitPicked {
                variant: variant.clone(),
                part: channel.part.clone(),
                fitted,
            },
        })
    };
    let button = |id: signex_i18n::MessageId, fitted: Option<bool>, kind: ButtonKind| {
        text_button(None, t!(m.i18n, id), Some(pick(fitted)), kind, m.tokens)
    };
    let actions = row![
        button(
            ids::VARIANT_MANAGER_FIT_EVERY_CHANNEL,
            Some(true),
            ButtonKind::Small
        ),
        button(
            ids::VARIANT_MANAGER_UNFIT_EVERY_CHANNEL,
            Some(false),
            ButtonKind::Small
        ),
        button(
            ids::VARIANT_MANAGER_EVERY_CHANNEL_AS_ON_SHEETS,
            None,
            ButtonKind::Ghost
        ),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center)
    .wrap()
    .boxed();
    Some(
        column![
            field_label(
                t!(m.i18n, ids::VARIANT_MANAGER_EVERY_CHANNEL, "count" => copies),
                m.tokens
            ),
            actions,
        ]
        .spacing(SP.m)
        .boxed(),
    )
}
