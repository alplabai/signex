use iced::widget::{Space, column, container, row, svg, text};
use iced::{Background, Border, Color, Element, Length, Theme};
use signex_types::theme::ThemeTokens;

use crate::catalog::Message;
use crate::icon::x_handle;
use crate::theme;

pub(crate) fn view<'a>(tokens: &ThemeTokens) -> Element<'a, Message> {
    const HEADER_HEIGHT: f32 = 28.0;
    const CLOSE_WIDTH: f32 = 46.0;
    const ICON_SIZE: f32 = 14.0;
    const BODY_PADDING: f32 = 16.0;

    let panel_bg = theme::color(tokens.panel_bg);
    let toolbar_bg = theme::color(tokens.toolbar_bg);
    let text_color = theme::color(tokens.text);
    let border = theme::color(tokens.border);

    let icon = svg(x_handle())
        .width(ICON_SIZE)
        .height(ICON_SIZE)
        .style(move |_: &Theme, _| svg::Style {
            color: Some(text_color),
        });
    let header = container(
        row![
            text("Modal title").size(13).color(text_color),
            Space::new().width(Length::Fill),
            container(icon)
                .width(CLOSE_WIDTH)
                .height(HEADER_HEIGHT)
                .align_x(iced::alignment::Horizontal::Center)
                .align_y(iced::alignment::Vertical::Center)
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(Color::from_rgba(0.78, 0.22, 0.22, 1.0))),
                    border: Border {
                        radius: iced::border::Radius::default().top_right(8.0),
                        ..Border::default()
                    },
                    ..container::Style::default()
                }),
        ]
        .align_y(iced::Alignment::Center),
    )
    .height(HEADER_HEIGHT)
    .padding(iced::Padding {
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
        left: BODY_PADDING,
    })
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(toolbar_bg)),
        border: Border {
            width: 0.0,
            radius: iced::border::Radius::default().top_left(8.0).top_right(8.0),
            color: Color::TRANSPARENT,
        },
        ..container::Style::default()
    });

    let body = container(
        column![
            text("Body content goes here.").size(11).color(text_color),
            Space::new().height(8),
            text("Use this card style for every modal — same 8 px corners.")
                .size(11)
                .color(text_color),
        ]
        .padding(BODY_PADDING),
    );

    container(column![header, body].width(Length::Fixed(420.0)))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(panel_bg)),
            border: Border {
                width: 1.0,
                radius: 8.0.into(),
                color: border,
            },
            shadow: iced::Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.4),
                offset: iced::Vector::new(0.0, 4.0),
                blur_radius: 12.0,
            },
            ..container::Style::default()
        })
        .clip(true)
        .padding(0)
        .into()
}
