//! Signex AI panel layout on Graphite themes: Assistant / Generator switch, an
//! empty-state card and a disabled ask field. No sample chat or results.

use iced::widget::{column, container, row};
use iced::{Alignment, Background, Border, Color, Element, Length, Widget as _};
use signex_i18n::{ids, t};
use signex_widgets::segmented::{SegmentedStyle, segmented};
use signex_widgets::{button, text, text_input, tooltip};

use super::{SignalMode, SignalMsg, coming_soon};
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
            ids::PANELS_SIGNAL_ASSISTANT_TITLE,
            ids::PANELS_SIGNAL_ASSISTANT_BODY,
        ),
        SignalMode::Generator => (
            ids::PANELS_SIGNAL_GENERATOR_TITLE,
            ids::PANELS_SIGNAL_GENERATOR_BODY,
        ),
    };
    let card = crate::panels::empty_state_card(
        tokens,
        t!(ctx.i18n, title),
        t!(ctx.i18n, body),
        Some(t!(ctx.i18n, ids::PANELS_COMING_SOON)),
    );
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
        text_input(t!(ctx.i18n, ids::PANELS_SIGNAL_ASK), "")
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
        text(t!(ctx.i18n, ids::PANELS_SIGNAL_SEND))
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
        coming_soon(send, tokens, &ctx.i18n, tooltip::Position::Top),
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
    container(coming_soon(
        frame.boxed(),
        tokens,
        &ctx.i18n,
        tooltip::Position::Top,
    ))
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
