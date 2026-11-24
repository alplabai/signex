//! The 28 px tab strip under a repeated sheet: Editor, then one tab per
//! channel with its part count. Sizes come from the chip metrics of the
//! count chips; the chosen tab has the accent-soft fill and a 2 px
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
            label = label.push(
                text(count)
                    .size(metrics.caption_size)
                    .color(res.ink3)
                    .boxed(),
            );
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
    let tokens = ctx.tokens;
    let accent = ti(tokens.accent);
    let button = signex_widgets::button(label)
        .padding(Padding::from([0.0, metrics.tab_pad_h]))
        .height(metrics.tab_height)
        .on_press(on_press)
        .style(move |_: &Theme, status: iced::widget::button::Status| {
            let hovered = matches!(status, iced::widget::button::Status::Hovered);
            tab_look(&tokens, is_on, hovered)
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

/// A tab's fill and ink: the chosen tab is accent-soft with its ink, an off
/// tab is unfilled (hover fill under the pointer) in the secondary text
/// token, so its label reads on the strip in either theme (mockup row 151).
fn tab_look(
    tokens: &signex_types::theme::ThemeTokens,
    is_on: bool,
    hovered: bool,
) -> iced::widget::button::Style {
    let res = resolve(tokens);
    let (background, text_color) = match (is_on, hovered) {
        (true, _) => (
            Some(Background::Color(res.accent_soft)),
            res.accent_soft_ink,
        ),
        (false, true) => (
            Some(Background::Color(ti(tokens.hover))),
            ti(tokens.text_secondary),
        ),
        (false, false) => (None, ti(tokens.text_secondary)),
    };
    iced::widget::button::Style {
        background,
        text_color,
        ..iced::widget::button::Style::default()
    }
}

#[cfg(test)]
mod tests {
    use signex_types::theme::{ThemeId, theme_tokens};

    use super::*;

    /// mockup row 151: off tabs use the secondary text token, at rest and
    /// under the pointer; the chosen tab uses the accent-soft ink.
    #[test]
    fn tab_labels_take_their_ink_from_the_theme() {
        for theme in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
            let tokens = theme_tokens(theme);
            let secondary = ti(tokens.text_secondary);
            assert_eq!(
                tab_look(&tokens, false, false).text_color,
                secondary,
                "{theme:?}"
            );
            assert_eq!(
                tab_look(&tokens, false, true).text_color,
                secondary,
                "{theme:?}"
            );
            let on = tab_look(&tokens, true, false);
            assert_eq!(on.text_color, resolve(&tokens).accent_soft_ink, "{theme:?}");
            assert_eq!(
                on.background,
                Some(Background::Color(resolve(&tokens).accent_soft)),
                "{theme:?}"
            );
        }
    }
}
