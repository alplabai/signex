//! Top menu bar — File, Edit, View, Place, Design, Tools, Window, Help.
//! Theme selector is temporarily inline until proper dropdown menus are built.

use iced::widget::{button, container, row, text, Row};
use iced::{Element, Length};
use signex_types::theme::ThemeId;

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

/// Render the menu bar as a row of text buttons.
pub fn view(current_theme: ThemeId) -> Element<'static, MenuMessage> {
    let menu_btn = |label: &'static str| {
        button(text(label).size(13))
            .padding([4, 10])
            .style(button::text)
    };

    // Theme buttons — highlight the active one
    let theme_btn = |id: ThemeId, label: &'static str| {
        let btn = button(text(label).size(11))
            .padding([2, 6])
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
        // Spacer
        iced::widget::space::horizontal(),
        // Theme selector (temporary — will move to View > Theme submenu)
        text("Theme:").size(11),
        theme_btn(ThemeId::CatppuccinMocha, "Mocha"),
        theme_btn(ThemeId::VsCodeDark, "VS Code"),
        theme_btn(ThemeId::GitHubDark, "GitHub"),
        theme_btn(ThemeId::AltiumDark, "Altium"),
        theme_btn(ThemeId::SolarizedLight, "Solarized"),
        theme_btn(ThemeId::Nord, "Nord"),
    ]
    .spacing(2)
    .align_y(iced::Alignment::Center);

    container(bar)
        .width(Length::Fill)
        .padding([2, 8])
        .into()
}
