//! The compiled channel view: a two-column grid of channel cards on the
//! sunken ground. A card names its channel and counts its parts and nets;
//! its body lists the channel's designators. The chosen channel's card is
//! outlined in the accent; a click chooses that channel. The miniature
//! drawing of the sheet at card scale is not built yet (see
//! `docs/MULTI_CHANNEL.md`, "Not built yet").

use iced::widget::{Column, Row, Space, column, container, row, scrollable};
use iced::{Alignment, Background, Border, Element, Length, Padding, Theme, Widget as _};
use signex_i18n::{UserText, ids, t};
use signex_widgets::text;

use crate::app::{CompiledDesignMsg, Message};
use crate::panels::PanelContext;
use crate::panels::compiled_design::{ChannelStripView, ChannelTabView};
use crate::styles::{Radii, TypeScale, ti, tokens::resolve};

const TS: TypeScale = TypeScale::DEFAULT;
/// Gap between cards and padding around the grid: 10 px at the 12 px body.
const GAP: f32 = TS.body * 10.0 / 12.0;
/// Designators listed per card body.
const BODY_ROWS: usize = 9;
/// Gap between the designator lines of a card: 2 px at the 12 px body.
const LINE_GAP: f32 = TS.body * 2.0 / 12.0;

/// Height the floating Active Bar takes from the top of the canvas region:
/// its top margin and its own height, read from the bar's layout.
fn bar_clearance() -> f32 {
    let metrics = signex_widgets::active_bar::Metrics::GRAPHITE;
    crate::active_bar::bar_top_margin() + crate::active_bar::bar_height_for(&metrics)
}

pub(super) fn view<'a>(
    ctx: &'a PanelContext,
    view: &'a ChannelStripView,
    active: u32,
) -> Element<'a, Message> {
    let sunken = resolve(&ctx.tokens).sunken;
    let mut grid: Column<Element<'a, Message>> = Column::new().spacing(GAP);
    for pair in view.tabs.chunks(2) {
        let mut line: Row<Element<'a, Message>> = Row::new().spacing(GAP);
        for tab in pair {
            line = line.push(card(ctx, view, tab, tab.channel.index == active));
        }
        if pair.len() == 1 {
            line = line.push(
                Space::new()
                    .width(Length::Fill)
                    .height(Length::Shrink)
                    .boxed(),
            );
        }
        grid = grid.push(line.boxed());
    }
    // The schematic's floating Active Bar sits over the top of this view;
    // the grid starts under it so no card header is covered (mockup row 151).
    let inset = Padding {
        top: bar_clearance() + GAP,
        ..Padding::new(GAP)
    };
    container(scrollable(
        container(grid).padding(inset).width(Length::Fill),
    ))
    .width(Length::Fill)
    .height(Length::Fill)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(sunken)),
        ..container::Style::default()
    })
    .boxed()
}

fn card<'a>(
    ctx: &'a PanelContext,
    view: &'a ChannelStripView,
    tab: &'a ChannelTabView,
    is_on: bool,
) -> Element<'a, Message> {
    let i18n = &*ctx.i18n;
    let tokens = &ctx.tokens;
    let res = resolve(tokens);
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    let caption = match (tab.parts, tab.nets) {
        (Some(parts), Some(nets)) => t!(
            i18n,
            ids::PANELS_CHANNEL_CARD_CAPTION,
            "parts" => parts,
            "nets" => nets
        ),
        _ => t!(i18n, ids::PANELS_CHANNEL_VIEW_NOT_COMPILED),
    };
    let header = row![
        text(UserText::new(tab.channel.name.as_str()))
            .size(TS.body)
            .font(mono)
            .color(ti(tokens.text)),
        text(caption).size(TS.caption).color(res.ink3),
    ]
    .spacing(GAP)
    .align_y(Alignment::Center);
    let mut body: Column<Element<'a, Message>> = column![].spacing(LINE_GAP);
    for name in tab.designators.iter().take(BODY_ROWS) {
        body = body.push(
            text(UserText::new(name.as_str()))
                .size(TS.caption)
                .font(mono)
                .color(res.ink3)
                .boxed(),
        );
    }
    let (fill, edge, accent) = (res.raised, res.line_soft, ti(tokens.accent));
    let content = container(column![header, body].spacing(GAP))
        .padding(GAP)
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                color: if is_on { accent } else { edge },
                width: if is_on { 2.0 } else { 1.0 },
                radius: Radii::GRAPHITE.card.into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    signex_widgets::button(content)
        .padding(0)
        .width(Length::Fill)
        .on_press(Message::from(CompiledDesignMsg::ChannelPicked {
            sheet: view.sheet.clone(),
            channel: Some(tab.channel.index),
        }))
        .style(|_: &Theme, _| iced::widget::button::Style::default())
        .boxed()
}
