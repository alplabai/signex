use super::*;

use iced::widget::{button, column, container, row, svg, text, tooltip};

const ICON_SIZE: f32 = 18.0;
const BUTTON_SIZE: f32 = 28.0;

const PREVIOUS_LAYER_ICON: &[u8] = include_bytes!("../../assets/gerber-viewer/previous_layer.svg");
const NEXT_LAYER_ICON: &[u8] = include_bytes!("../../assets/gerber-viewer/next_layer.svg");
const MOVE_LAYER_UP_ICON: &[u8] = include_bytes!("../../assets/gerber-viewer/move_layer_up.svg");
const MOVE_LAYER_DOWN_ICON: &[u8] =
    include_bytes!("../../assets/gerber-viewer/move_layer_down.svg");

pub(super) fn view<'a>(
    state: &GerberViewerState,
    tokens: &ThemeTokens,
) -> Element<'a, GerberViewerMessage> {
    let previous = previous_layer_index(state.active_layer, state.layers.len())
        .map(|_| GerberViewerMessage::PreviousLayer);
    let next = next_layer_index(state.active_layer, state.layers.len())
        .map(|_| GerberViewerMessage::NextLayer);
    let move_up = state
        .active_layer
        .filter(|index| *index + 1 < state.layers.len())
        .map(|_| GerberViewerMessage::MoveLayerUp);
    let move_down = state
        .active_layer
        .filter(|index| *index > 0)
        .map(|_| GerberViewerMessage::MoveLayerDown);

    column![
        text("Layer controls")
            .size(12)
            .color(styles::ti(tokens.text)),
        row![
            layer_control_button(
                PREVIOUS_LAYER_ICON,
                "Previous Layer (PgUp)",
                previous,
                tokens,
            ),
            layer_control_button(NEXT_LAYER_ICON, "Next Layer (PgDn)", next, tokens,),
            layer_control_button(MOVE_LAYER_UP_ICON, "Move Layer Up (+)", move_up, tokens,),
            layer_control_button(
                MOVE_LAYER_DOWN_ICON,
                "Move Layer Down (-)",
                move_down,
                tokens,
            ),
        ]
        .spacing(4),
    ]
    .spacing(6)
    .into()
}

fn layer_control_button<'a>(
    icon: &'static [u8],
    hint: &'static str,
    on_press: Option<GerberViewerMessage>,
    tokens: &ThemeTokens,
) -> Element<'a, GerberViewerMessage> {
    let enabled = on_press.is_some();
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
    let icon = svg(svg::Handle::from_memory(icon))
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
    .on_press_maybe(on_press)
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
        container(text(hint).size(11).color(styles::ti(tokens.text)))
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
pub(super) const LAYER_CONTROL_ICON_ASSETS: [&[u8]; 4] = [
    PREVIOUS_LAYER_ICON,
    NEXT_LAYER_ICON,
    MOVE_LAYER_UP_ICON,
    MOVE_LAYER_DOWN_ICON,
];
