use super::*;

use iced::widget::{Column, Row, tooltip};
use iced_aw::{DropDown, drop_down};

const PALETTE_WIDTH: f32 = 304.0;
const SWATCH_WIDTH: f32 = 17.0;
const SWATCH_HEIGHT: f32 = 13.0;

pub(super) fn color_dropdown<'a>(
    choices: Vec<GerberLayerColorChoice>,
    selected: Option<GerberLayerColorChoice>,
    target: GerberColorTarget,
    expanded: bool,
    trigger_width: Length,
    on_select: impl Fn(usize) -> GerberViewerMessage + Copy + 'a,
    tokens: &ThemeTokens,
) -> Element<'a, GerberViewerMessage> {
    let panel_background = styles::ti(tokens.panel_bg);
    let trigger_background = styles::ti(tokens.toolbar_bg);
    let border_color = styles::ti(tokens.border);
    let hover_color = styles::ti(tokens.hover);
    let accent_color = styles::ti(tokens.accent);
    let text_color = styles::ti(tokens.text);
    let selected_color = selected
        .as_ref()
        .map_or(Color::TRANSPARENT, |choice| choice.color);
    let selected_palette_index = selected.as_ref().map(|choice| choice.palette_index);

    let selected_swatch = container(Space::new())
        .width(Length::Fill)
        .height(14)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(selected_color)),
            border: Border {
                width: 1.0,
                radius: 2.0.into(),
                color: border_color,
            },
            ..container::Style::default()
        });
    let trigger = button(
        row![selected_swatch, text("▾").size(11).color(text_color)]
            .spacing(6)
            .align_y(iced::Alignment::Center),
    )
    .width(trigger_width)
    .padding([5, 6])
    .on_press(GerberViewerMessage::ToggleColorPicker(target))
    .style(move |_: &Theme, status: button::Status| {
        let background = if status == button::Status::Hovered {
            hover_color
        } else {
            trigger_background
        };
        button::Style {
            background: Some(Background::Color(background)),
            border: Border {
                width: 1.0,
                radius: 3.0.into(),
                color: border_color,
            },
            text_color,
            ..button::Style::default()
        }
    });

    let overlay: Element<'a, GerberViewerMessage> = if expanded {
        let mut palette = Column::new().spacing(2);
        for choices_row in choices.chunk_by(|left, right| left.family_index == right.family_index) {
            let mut swatch_row = Row::new().spacing(2);
            for choice in choices_row {
                let color = choice.color;
                let palette_index = choice.palette_index;
                let selected = selected_palette_index == Some(palette_index);
                let swatch = container(Space::new())
                    .width(SWATCH_WIDTH)
                    .height(SWATCH_HEIGHT)
                    .style(move |_: &Theme| container::Style {
                        background: Some(Background::Color(color)),
                        border: Border {
                            width: if selected { 2.0 } else { 1.0 },
                            radius: 1.0.into(),
                            color: if selected { accent_color } else { border_color },
                        },
                        ..container::Style::default()
                    });
                let option = button(swatch)
                    .padding(1)
                    .on_press(on_select(palette_index))
                    .style(move |_: &Theme, status: button::Status| button::Style {
                        background: (status == button::Status::Hovered)
                            .then_some(Background::Color(hover_color)),
                        border: Border::default(),
                        ..button::Style::default()
                    });
                swatch_row = swatch_row.push(tooltip(
                    option,
                    text(choice.label.clone()).size(11),
                    tooltip::Position::Top,
                ));
            }
            palette = palette.push(swatch_row);
        }

        container(palette)
            .padding(6)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(panel_background)),
                border: Border {
                    width: 1.0,
                    radius: 4.0.into(),
                    color: border_color,
                },
                shadow: iced::Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.45),
                    offset: iced::Vector::new(0.0, 3.0),
                    blur_radius: 10.0,
                },
                ..container::Style::default()
            })
            .into()
    } else {
        Space::new().into()
    };

    DropDown::new(trigger, overlay, expanded)
        .width(PALETTE_WIDTH)
        .alignment(drop_down::Alignment::Bottom)
        .on_dismiss(GerberViewerMessage::CloseColorPicker)
        .into()
}
