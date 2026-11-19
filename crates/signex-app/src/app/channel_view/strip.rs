//! The 28 px tab strip under a repeated sheet: Editor, then one tab per
//! channel with its part count. Sizes come from the chip metrics of the
//! count tabs (BB4); the chosen tab has the accent-soft fill and a 2 px
//! accent underline.

use iced::widget::{Row, Space, column, container, row, svg};
use iced::{Alignment, Background, Element, Length, Padding, Theme, Widget as _};
use signex_i18n::{UserText, ids, t};
use signex_widgets::chips::ChipMetrics;
use signex_widgets::text;

use crate::app::{CompiledDesignMsg, Message};
use crate::panels::PanelContext;
use crate::panels::compiled_design::ChannelStripView;
use crate::styles::{Spacing, TypeScale, ti, tokens::resolve};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;
/// The chosen tab's underline.
const UNDERLINE: f32 = 2.0;

pub(super) fn view<'a>(ctx: &'a PanelContext, view: &'a ChannelStripView) -> Element<'a, Message> {
    let i18n = &*ctx.i18n;
    let tokens = &ctx.tokens;
    let res = resolve(tokens);
    let metrics = ChipMetrics::from_text_size(TS.body);
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    let pick = |channel: Option<u32>| {
        Message::from(CompiledDesignMsg::ChannelPicked {
            sheet: view.sheet.clone(),
            channel,
        })
    };
    let editor = row![
        svg(crate::icons::icon_dd_sheet_symbol(ctx.theme_id))
            .width(TS.body)
            .height(TS.body),
        text(t!(i18n, ids::PANELS_CHANNEL_STRIP_EDITOR)).size(TS.body),
    ]
    .spacing(metrics.tab_gap)
    .align_y(Alignment::Center);
    let mut tabs: Row<Element<'a, Message>> = Row::new().spacing(metrics.tab_spacing);
    tabs = tabs.push(tab(
        ctx,
        metrics,
        editor.boxed(),
        view.active.is_none(),
        pick(None),
    ));
    for channel in &view.tabs {
        let index = channel.channel.index;
        let is_on = view.active == Some(index);
        let mut label = row![
            text(UserText::new(channel.channel.name.as_str()))
                .size(TS.body)
                .font(mono)
        ]
        .spacing(metrics.tab_gap)
        .align_y(Alignment::Center);
        if let Some(parts) = channel.parts {
            let count = if is_on {
                t!(i18n, ids::PANELS_CHANNEL_STRIP_PARTS, "count" => parts)
            } else {
                t!(i18n, ids::PANELS_CHANNEL_STRIP_COUNT, "count" => parts)
            };
            label = label.push(text(count).size(metrics.caption_size).color(res.ink3));
        }
        tabs = tabs.push(tab(ctx, metrics, label.boxed(), is_on, pick(Some(index))));
    }
    let chrome = ti(tokens.toolbar_bg);
    let edge = res.line_soft;
    column![
        container(Space::new())
            .width(Length::Fill)
            .height(1)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(edge)),
                ..container::Style::default()
            }),
        container(tabs)
            .padding(Padding::from([0.0, SP.s]))
            .width(Length::Fill)
            .height(metrics.tab_height)
            .align_y(iced::alignment::Vertical::Center)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(chrome)),
                ..container::Style::default()
            }),
    ]
    .width(Length::Fill)
    .boxed()
}

/// One tab: the label in a button, and the accent underline when chosen.
fn tab<'a>(
    ctx: &'a PanelContext,
    metrics: ChipMetrics,
    label: Element<'a, Message>,
    is_on: bool,
    on_press: Message,
) -> Element<'a, Message> {
    let tokens = &ctx.tokens;
    let res = resolve(tokens);
    let (soft, hover, accent) = (res.accent_soft, ti(tokens.hover), ti(tokens.accent));
    let button = signex_widgets::button(label)
        .padding(Padding::from([0.0, metrics.tab_pad_h]))
        .height(metrics.tab_height)
        .on_press(on_press)
        .style(move |_: &Theme, status: iced::widget::button::Status| {
            let hovered = matches!(status, iced::widget::button::Status::Hovered);
            let fill = match (is_on, hovered) {
                (true, _) => Some(Background::Color(soft)),
                (false, true) => Some(Background::Color(hover)),
                (false, false) => None,
            };
            iced::widget::button::Style {
                background: fill,
                ..iced::widget::button::Style::default()
            }
        });
    if !is_on {
        return button.boxed();
    }
    // The underline is a layer over the tab's bottom edge; the button sizes
    // the stack.
    let underline = container(
        container(Space::new())
            .width(Length::Fill)
            .height(UNDERLINE)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(accent)),
                ..container::Style::default()
            }),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_y(iced::alignment::Vertical::Bottom);
    iced::widget::stack![button, underline].boxed()
}
