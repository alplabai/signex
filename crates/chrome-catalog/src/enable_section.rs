//! Enable-header subsection sample (epic #2523, BB3): the header is the
//! enable checkbox, and the fields under a switched-off header are greyed
//! under a panel-coloured veil and take no input.
//!
//! The app widget is `signex-app` `panels::enable_section`; this crate
//! cannot depend on the app, so the sample repeats its look from the
//! theme tokens. Keep the two in step when either changes.

use iced::widget::{Space, column, container, row, stack};
use iced::{Background, Border, Color, Element, Length, Padding, Theme, Widget};
use signex_types::theme::ThemeTokens;
use signex_widgets::{CheckboxLabel, Label, checkbox, text};

use crate::catalog::Message;
use crate::theme;
use signex_i18n::UserText;

/// Sample text for this page: fixture content, not UI copy.
fn sample_text(text: impl Into<String>) -> Label<'static> {
    UserText::new(text).into()
}

/// The catalog's body text size; every other size derives from it.
const BASE: f32 = 11.0;
/// Alpha of the veil over switched-off fields (the app's
/// `DEPENDENT_VEIL_ALPHA`).
const VEIL_ALPHA: f32 = 0.58;

/// Sample text: the catalog has no message catalog to draw from.
fn label(s: &'static str) -> Label<'static> {
    sample_text(s)
}

/// An on and an off subsection side by side.
pub(crate) fn view<'a>(tokens: &ThemeTokens) -> Element<'a, Message> {
    row![
        subsection(
            tokens,
            "Paste",
            true,
            &[("Expansion", "0.05 mm"), ("Mode", "Rule")]
        ),
        subsection(
            tokens,
            "Thermal Relief",
            false,
            &[("Spokes", "4"), ("Gap", "0.25 mm")]
        ),
    ]
    .spacing(BASE * 2.0)
    .boxed()
}

fn subsection<'a>(
    tokens: &ThemeTokens,
    name: &'static str,
    is_enabled: bool,
    fields: &[(&'static str, &'static str)],
) -> Element<'a, Message> {
    let mut body = column![].spacing(BASE * 4.0 / 11.0);
    for &(field, value) in fields {
        body = body.push(field_row(tokens, field, value));
    }
    let body: Element<'a, Message> = if is_enabled {
        body.boxed()
    } else {
        let veil = Color {
            a: VEIL_ALPHA,
            ..theme::color(tokens.panel_bg)
        };
        stack![
            body,
            container(Space::new())
                .width(Length::Fill)
                .height(Length::Fill)
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(veil)),
                    ..container::Style::default()
                }),
        ]
        .boxed()
    };
    container(column![header(tokens, name, is_enabled), body])
        .width(Length::Fixed(BASE * 22.0))
        .boxed()
}

/// The header: a checkbox in the secondary ink, medium weight.
fn header<'a>(tokens: &ThemeTokens, name: &'static str, is_enabled: bool) -> Element<'a, Message> {
    let ink2 = theme::color(tokens.text_secondary);
    let fill = theme::color(tokens.extended.primary.unwrap_or(tokens.accent));
    let field = theme::color(tokens.extended.field.unwrap_or(tokens.panel_bg));
    let line = theme::color(tokens.extended.line_strong.unwrap_or(tokens.border));
    let check = checkbox::<Message, Theme, iced::Renderer>(is_enabled)
        .labelled(label(name))
        .size(BASE)
        .text_size(BASE * 10.0 / 11.0)
        .spacing(BASE * 6.0 / 11.0)
        .font(iced::Font {
            weight: iced::font::Weight::Medium,
            ..iced::Font::DEFAULT
        })
        .style(move |_: &Theme, _| checkbox::Style {
            background: Background::Color(if is_enabled { fill } else { field }),
            icon_color: Color::WHITE,
            border: Border {
                width: 1.0,
                radius: (BASE * 4.0 / 11.0).into(),
                color: if is_enabled { fill } else { line },
                ..Border::default()
            },
            text_color: Some(ink2),
        });
    container(check)
        .padding(Padding {
            top: BASE * 8.0 / 11.0,
            right: BASE,
            bottom: BASE * 2.0 / 11.0,
            left: BASE,
        })
        .boxed()
}

/// One dependent field: label and a value box.
fn field_row<'a>(
    tokens: &ThemeTokens,
    name: &'static str,
    value: &'static str,
) -> Element<'a, Message> {
    let ink = theme::color(tokens.text);
    let ink3 = theme::color(tokens.extended.ink3.unwrap_or(tokens.text_secondary));
    let field = theme::color(tokens.extended.field.unwrap_or(tokens.panel_bg));
    let line = theme::color(tokens.border);
    row![
        container(text(label(name)).size(BASE).color(ink3)).width(Length::FillPortion(2)),
        container(text(label(value)).size(BASE).color(ink))
            .padding([BASE * 2.0 / 11.0, BASE * 6.0 / 11.0])
            .width(Length::FillPortion(3))
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(field)),
                border: Border {
                    width: 1.0,
                    radius: (BASE * 7.0 / 11.0).into(),
                    color: line,
                    ..Border::default()
                },
                ..container::Style::default()
            }),
    ]
    .align_y(iced::Alignment::Center)
    .padding([0.0, BASE])
    .boxed()
}
