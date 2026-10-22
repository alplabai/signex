use super::*;

use iced::widget::{button, container, svg, text, tooltip};

const ICON_SIZE: f32 = 18.0;
const BUTTON_SIZE: f32 = 28.0;

const CLEAR_HIGHLIGHT_ICON: &[u8] =
    include_bytes!("../../assets/gerber-viewer/clear_highlight.svg");

pub(super) fn clear_button(
    enabled: bool,
    tokens: &ThemeTokens,
) -> Element<'static, GerberViewerMessage> {
    let icon_color = if enabled {
        styles::ti(tokens.text_secondary)
    } else {
        let muted = styles::ti(tokens.text_secondary);
        Color {
            a: muted.a * 0.45,
            ..muted
        }
    };
    let hover = styles::ti(tokens.hover);
    let icon = svg(svg::Handle::from_memory(CLEAR_HIGHLIGHT_ICON))
        .width(ICON_SIZE)
        .height(ICON_SIZE)
        .style(move |_: &Theme, _| iced::widget::svg::Style {
            color: Some(icon_color),
        });
    let button = button(
        container(icon)
            .width(BUTTON_SIZE)
            .height(BUTTON_SIZE)
            .center_x(BUTTON_SIZE)
            .center_y(BUTTON_SIZE),
    )
    .padding(0)
    .on_press_maybe(enabled.then_some(GerberViewerMessage::ClearHighlight))
    .style(move |_: &Theme, status: button::Status| button::Style {
        background: (enabled && status == button::Status::Hovered)
            .then_some(Background::Color(hover)),
        border: Border {
            width: 0.0,
            radius: 2.0.into(),
            color: Color::TRANSPARENT,
        },
        ..button::Style::default()
    });
    let panel = styles::ti(tokens.panel_bg);
    let border = styles::ti(tokens.border);
    tooltip(
        button,
        container(
            text("Clear Highlight")
                .size(11)
                .color(styles::ti(tokens.text)),
        )
        .padding([4, 7])
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(panel)),
            border: Border {
                width: 1.0,
                radius: 3.0.into(),
                color: border,
            },
            ..container::Style::default()
        }),
        tooltip::Position::Top,
    )
    .gap(5)
    .into()
}

#[cfg(test)]
pub(super) const CLEAR_HIGHLIGHT_ICON_ASSET: &[u8] = CLEAR_HIGHLIGHT_ICON;
