//! Top menu bar — File, Edit, View, Place, Design, Tools, Window, Help.

use iced::widget::{button, container, row, text, Row};
use iced::{Element, Length};
use signex_types::theme::ThemeId;

use crate::styles;

#[derive(Debug, Clone)]
pub enum MenuMessage {
    NewProject,
    OpenProject,
    Save,
    Undo,
    Redo,
    ZoomFit,
    ThemeSelected(ThemeId),
}

pub fn view(current_theme: ThemeId) -> Element<'static, MenuMessage> {
    let menu_btn = |label: &'static str| {
        button(text(label).size(12).color(styles::TEXT_PRIMARY))
            .padding([3, 10])
            .style(button::text)
    };

    let theme_btn = |id: ThemeId, label: &'static str| {
        let btn = button(text(label).size(10).color(if id == current_theme {
            iced::Color::WHITE
        } else {
            styles::TEXT_MUTED
        }))
        .padding([2, 5])
        .on_press(MenuMessage::ThemeSelected(id));
        if id == current_theme {
            btn.style(button::primary)
        } else {
            btn.style(button::text)
        }
    };

    let bar: Row<'static, MenuMessage> = row![
        menu_btn("File").on_press(MenuMessage::OpenProject),
        menu_btn("Edit").on_press(MenuMessage::Undo),
        menu_btn("View").on_press(MenuMessage::ZoomFit),
        menu_btn("Place"),
        menu_btn("Design"),
        menu_btn("Tools"),
        menu_btn("Window"),
        menu_btn("Help"),
        iced::widget::space::horizontal(),
        text("Theme:").size(10).color(styles::TEXT_MUTED),
        theme_btn(ThemeId::CatppuccinMocha, "Mocha"),
        theme_btn(ThemeId::VsCodeDark, "VS Code"),
        theme_btn(ThemeId::GitHubDark, "GitHub"),
        theme_btn(ThemeId::AltiumDark, "Altium"),
        theme_btn(ThemeId::SolarizedLight, "Solarized"),
        theme_btn(ThemeId::Nord, "Nord"),
    ]
    .spacing(1)
    .align_y(iced::Alignment::Center);

    container(bar)
        .width(Length::Fill)
        .padding([1, 6])
        .style(styles::toolbar_strip)
        .into()
}
