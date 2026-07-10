use iced::widget::{column, container, row, text};
use iced::{Alignment, Background, Border, Element, Length};
use signex_types::theme::ThemeTokens;

use super::color_code::ComponentColorCode;
use super::domain::ComponentKind;

pub fn color_code_representations<'a, Message: 'a>(
    representations: Vec<ComponentColorCode>,
    kind: ComponentKind,
    tokens: &'a ThemeTokens,
) -> Element<'a, Message> {
    let representation_label = if representations.len() > 1 {
        "Color code alternatives"
    } else {
        "Color code"
    };
    let mut color_codes = column![
        text(representation_label)
            .size(11)
            .color(token_color(tokens.text_secondary))
    ]
    .spacing(8);
    if representations.is_empty() {
        color_codes = color_codes.push(
            text(color_code_unavailable_label(kind))
                .size(12)
                .color(token_color(tokens.text_secondary)),
        );
    } else {
        for representation in representations {
            color_codes = color_codes.push(color_code_line(representation, tokens));
        }
    }
    color_codes.into()
}

fn color_code_line<'a, Message: 'a>(
    code: ComponentColorCode,
    tokens: &'a ThemeTokens,
) -> Element<'a, Message> {
    let accessible_label = code.accessible_label();
    let mut bands = row![].spacing(5).align_y(Alignment::Center);
    for band in code.bands {
        bands = bands.push(
            container(text(" "))
                .width(20)
                .height(30)
                .style(move |_theme| container::Style {
                    background: Some(Background::Color(band.color())),
                    border: Border {
                        width: 1.0,
                        radius: 2.0.into(),
                        color: iced::Color::BLACK,
                    },
                    ..container::Style::default()
                }),
        );
    }
    bands
        .push(
            container(
                text(accessible_label)
                    .size(12)
                    .color(token_color(tokens.text)),
            )
            .width(Length::Fill),
        )
        .into()
}

fn color_code_unavailable_label(kind: ComponentKind) -> &'static str {
    match kind {
        ComponentKind::Resistor => "Not representable with IEC 60062 bands",
        ComponentKind::Capacitor => "Not representable with capacitor color bands",
        ComponentKind::Inductor => "Not representable with common µH color bands",
    }
}

fn token_color(color: signex_types::theme::Color) -> iced::Color {
    iced::Color::from_rgba8(color.r, color.g, color.b, f32::from(color.a) / 255.0)
}
