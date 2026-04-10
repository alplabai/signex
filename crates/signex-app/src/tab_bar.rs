//! Document tab bar — tabs for open schematic sheets and PCB.

use iced::widget::{button, container, row, text, Row};
use iced::{Element, Length};

use crate::app::TabInfo;

#[derive(Debug, Clone)]
pub enum TabMessage {
    Select(usize),
    Close(usize),
}

/// Render the tab bar.
pub fn view<'a>(tabs: &[TabInfo], active: usize) -> Element<'a, TabMessage> {
    let mut bar = Row::new().spacing(1);

    for (i, tab) in tabs.iter().enumerate() {
        let label = if tab.dirty {
            format!("{} *", tab.title)
        } else {
            tab.title.clone()
        };

        let tab_btn = button(
            row![
                text(label).size(12),
                button(text("x").size(10))
                    .padding([1, 4])
                    .style(button::text)
                    .on_press(TabMessage::Close(i)),
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center),
        )
        .padding([4, 10])
        .on_press(TabMessage::Select(i));

        let tab_btn = if i == active {
            tab_btn.style(button::primary)
        } else {
            tab_btn.style(button::secondary)
        };

        bar = bar.push(tab_btn);
    }

    container(bar)
        .width(Length::Fill)
        .padding([1, 8])
        .style(container::bordered_box)
        .into()
}
