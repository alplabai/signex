//! Signex AI panel layout on Graphite themes: Assistant / Generator switch, an
//! empty-state card and a disabled ask field. No sample chat or results.

use iced::widget::{button, column, container, row, text, text_input, tooltip};
use iced::{Alignment, Background, Border, Color, Element, Length, Widget as _};
use signex_widgets::segmented::{SegmentedStyle, segmented};

use super::{COMING_SOON, SignalMode, SignalMsg, coming_soon};
use crate::panels::{PanelContext, PanelMsg};
use crate::styles::{self, ChromeMetrics, TypeScale, ti, tokens::resolve};

const TS: TypeScale = TypeScale::DEFAULT;

/// A container style with a 1 px rule on one edge.
fn rule(line: Color, bottom: bool) -> impl Fn(&iced::Theme) -> container::Style {
    move |_| {
        let side = iced::border::Side::default().width(1.0).color(line);
        let mut border = Border::default();
        if bottom {
            border.bottom = side;
        } else {
            border.top = side;
        }
        container::Style {
            border,
            ..container::Style::default()
        }
    }
}

fn mode_switch<'a>(ctx: &PanelContext) -> Element<'a, PanelMsg> {
    let tokens = &ctx.tokens;
    let r = resolve(tokens);
    let style = SegmentedStyle::new(
        r.sunken,
        r.raised,
        ti(tokens.border),
        ti(tokens.text_secondary),
        ti(tokens.text),
    )
    .fill_width(true);
    let switch = segmented(&SignalMode::ALL, ctx.signal_mode, style, |mode| {
        PanelMsg::Signal(SignalMsg::ModeSelected(mode))
    });
    container(switch)
        .padding([8.0, 12.0])
        .width(Length::Fill)
        .style(rule(r.line_soft, true))
        .boxed()
}

fn empty_state<'a>(ctx: &PanelContext) -> Element<'a, PanelMsg> {
    let tokens = &ctx.tokens;
    let (title, body) = match ctx.signal_mode {
        SignalMode::Assistant => (
            "Signex AI is not available yet",
            "Ask about this sheet and review proposed changes before anything is applied.",
        ),
        SignalMode::Generator => (
            "The Generator is not available yet",
            "Describe a board and approve each stage before anything is drawn.",
        ),
    };
    let card = crate::panels::empty_state_card(tokens, title, body, Some(COMING_SOON));
    container(card)
        .padding(12)
        .width(Length::Fill)
        .height(Length::Fill)
        .boxed()
}

fn ask_footer<'a>(ctx: &PanelContext) -> Element<'a, PanelMsg> {
    let tokens = &ctx.tokens;
    let r = resolve(tokens);
    let (ink4, ink) = (r.ink4, ti(tokens.text));
    // No `on_input`: the field is inert.
    let input: iced::widget::TextInput<'a, PanelMsg> =
        text_input("Ask about this sheet, or describe a change", "")
            .padding(iced::Padding::ZERO)
            .size(TS.dense)
            .width(Length::Fill)
            .style(move |_: &iced::Theme, _| text_input::Style {
                background: Background::Color(Color::TRANSPARENT),
                border: Border::default(),
                placeholder: ink4,
                value: ink,
                selection: Color { a: 0.4, ..ink },
            });
    let send: Element<'a, PanelMsg> = button(
        text("\u{2192}")
            .size(TS.dense)
            .width(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center),
    )
    .width(ChromeMetrics::DEFAULT.field)
    .height(ChromeMetrics::DEFAULT.field)
    .style(styles::primary_button(tokens))
    .boxed();
    let line = row![
        container(input).width(Length::Fill),
        coming_soon(send, tokens, tooltip::Position::Top),
    ]
    .spacing(6)
    .align_y(Alignment::Center);
    let frame = container(line)
        .padding(iced::Padding {
            top: 4.0,
            right: 4.0,
            bottom: 4.0,
            left: 10.0,
        })
        .width(Length::Fill)
        .style(styles::field(tokens, false));
    container(coming_soon(frame.boxed(), tokens, tooltip::Position::Top))
        .padding([10.0, 12.0])
        .width(Length::Fill)
        .style(rule(r.line_soft, false))
        .boxed()
}

/// The Signex AI panel on a Graphite theme.
pub fn view<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    column![mode_switch(ctx), empty_state(ctx), ask_footer(ctx)]
        .width(Length::Fill)
        .height(Length::Fill)
        .boxed()
}
