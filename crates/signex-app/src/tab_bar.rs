//! Document tab bar — tabs for open schematic sheets and PCB.

use iced::widget::{button, container, row, text, Row};
use iced::{Element, Length};

use crate::app::TabInfo;
use crate::styles;

#[derive(Debug, Clone)]
pub enum TabMessage {
    Select(usize),
    Close(usize),
}

pub fn view<'a>(tabs: &[TabInfo], active: usize) -> Element<'a, TabMessage> {
    let mut bar = Row::new().spacing(0);

    for (i, tab) in tabs.iter().enumerate() {
        let label = if tab.dirty {
            format!("{} *", tab.title)
        } else {
            tab.title.clone()
        };

        let is_active = i == active;
        let tab_btn = button(
            row![
                text(label).size(11).color(if is_active {
                    iced::Color::WHITE
                } else {
                    styles::TEXT_MUTED
                }),
                button(text("x").size(9).color(styles::TEXT_MUTED))
                    .padding([0, 3])
                    .style(button::text)
                    .on_press(TabMessage::Close(i)),
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center),
        )
        .padding([3, 10])
        .on_press(TabMessage::Select(i));

        let tab_btn = if is_active {
            tab_btn.style(button::primary)
        } else {
            tab_btn.style(button::text)
        };

        bar = bar.push(tab_btn);
    }

    container(bar)
        .width(Length::Fill)
        .padding([0, 6])
        .style(styles::tab_bar)
        .into()
}
