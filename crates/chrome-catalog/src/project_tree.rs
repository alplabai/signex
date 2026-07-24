use iced::widget::{Column, Row, Space, container, text};
use iced::{Background, Border, Color, Element, Length, Theme};
use signex_types::theme::ThemeTokens;

use crate::catalog::Message;
use crate::theme;

pub(crate) fn view<'a>(tokens: &ThemeTokens) -> Element<'a, Message> {
    let mut rows: Column<'a, Message> = Column::new().spacing(2);
    for (label, open, dirty, active) in [
        ("clean.standard_sch", false, false, false),
        ("open.standard_sch", true, false, false),
        ("dirty.standard_sch", true, true, false),
        ("active.standard_sch", true, false, true),
        ("active+dirty.standard_sch", true, true, true),
    ] {
        rows = rows.push(tree_row(label, open, dirty, active, tokens));
    }
    rows.into()
}

fn tree_row<'a>(
    label: &str,
    is_open: bool,
    is_dirty: bool,
    is_active: bool,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let text_color = theme::color(tokens.text);
    let active_bg = Color {
        a: 0.45,
        ..theme::color(tokens.selection)
    };
    let dirty_red = Color::from_rgba(0.85, 0.30, 0.30, 1.0);
    let dot_size = 6.0;
    let mut content: Row<'a, Message> = Row::new().spacing(8).align_y(iced::Alignment::Center);
    content = content.push(text(label.to_string()).size(11).color(text_color));
    content = content.push(Space::new().width(Length::Fill));
    if is_open || is_dirty {
        let dot_color = if is_dirty { dirty_red } else { Color::WHITE };
        content = content.push(
            container(Space::new().width(dot_size).height(dot_size))
                .width(dot_size)
                .height(dot_size)
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(dot_color)),
                    border: Border {
                        radius: (dot_size / 2.0).into(),
                        ..Border::default()
                    },
                    ..container::Style::default()
                }),
        );
    }
    let background = is_active.then_some(Background::Color(active_bg));
    container(content)
        .width(Length::Fixed(360.0))
        .padding([4, 8])
        .style(move |_: &Theme| container::Style {
            background,
            border: Border {
                radius: 2.0.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .into()
}
