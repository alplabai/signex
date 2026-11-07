//! Graphite canvas zoom pill: `- 100% + Fit`. One widget for every canvas: the
//! schematic overlay and the symbol editor give it their own messages.

use iced::widget::{Row, Widget, button, container, text};
use iced::{Background, Border, Element, Length, Theme};
use signex_types::theme::ThemeTokens;

use crate::styles::tokens::resolve;
use crate::styles::{Radii, ti};

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;

/// Inner padding of the pill.
const PAD: f32 = 3.0;
/// Gap between the pill's controls.
const GAP: f32 = 2.0;
/// Width and height of a `-` / `+` button.
const MINI: f32 = 24.0;
/// Width of the percentage readout.
const READOUT_W: f32 = 44.0;
/// Width of the Fit button.
const FIT_W: f32 = 34.0;
/// Radius of the controls inside the pill (the mock's 6 px).
const CONTROL_RADIUS: f32 = 6.0;

/// Outer size of the pill: padding and the 1 px border on every side.
pub fn pill_size() -> (f32, f32) {
    let inner_w = MINI + READOUT_W + MINI + FIT_W + 3.0 * GAP;
    (inner_w + 2.0 * (PAD + 1.0), MINI + 2.0 * (PAD + 1.0))
}

fn control_style(
    tokens: &ThemeTokens,
) -> impl Fn(&Theme, button::Status) -> button::Style + 'static {
    let ink = ti(tokens.text_secondary);
    let hover = ti(tokens.hover);
    move |_: &Theme, status| button::Style {
        background: match status {
            button::Status::Hovered | button::Status::Pressed => Some(Background::Color(hover)),
            _ => None,
        },
        text_color: ink,
        border: Border {
            radius: CONTROL_RADIUS.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

/// The pill element at its fixed [`pill_size`]; the three messages are what its
/// buttons send.
pub fn view<'a, M: Clone + 'a>(
    tokens: &ThemeTokens,
    zoom_percent: f64,
    zoom_out: M,
    zoom_in: M,
    fit_all: M,
) -> Element<'a, M> {
    let r = resolve(tokens);
    let line = ti(tokens.border);
    let ink = ti(tokens.text_secondary);
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);

    let mini = |label: &'static str, on_press: M| -> Element<'a, M> {
        button(
            container(text(label).size(TS.title).color(ink))
                .center_x(Length::Fill)
                .center_y(Length::Fill),
        )
        .width(MINI)
        .height(MINI)
        .padding(0)
        .on_press(on_press)
        .style(control_style(tokens))
        .boxed()
    };
    let readout: Element<'a, M> = container(
        text(format!("{zoom_percent:.0}%"))
            .size(TS.caption)
            .font(mono)
            .color(ink),
    )
    .width(READOUT_W)
    .height(MINI)
    .center_x(READOUT_W)
    .center_y(MINI)
    .boxed();
    let fit: Element<'a, M> = button(
        container(text("Fit").size(TS.caption).color(ink))
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .width(FIT_W)
    .height(MINI)
    .padding(0)
    .on_press(fit_all)
    .style(control_style(tokens))
    .boxed();

    let row = Row::<Element<'a, M>>::new()
        .spacing(GAP)
        .align_y(iced::Alignment::Center)
        .push(mini("\u{2212}", zoom_out))
        .push(readout)
        .push(mini("+", zoom_in))
        .push(fit);
    let (w, h) = pill_size();
    container(row)
        .padding(PAD)
        .width(w)
        .height(h)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(r.raised)),
            border: Border {
                width: 1.0,
                radius: Radii::GRAPHITE.card.into(),
                color: line,
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}
