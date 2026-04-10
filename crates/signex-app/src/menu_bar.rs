//! Top menu bar — File, Edit, View, Place, Design, Tools, Window, Help.

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
/// Full dropdown menus will be implemented in a later phase.
pub fn view(_current_theme: ThemeId) -> Element<'static, MenuMessage> {
    let menu_btn = |label: &'static str| {
        button(text(label).size(13))
            .padding([4, 10])
            .style(button::text)
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
    ]
    .spacing(2);

    container(bar)
        .width(Length::Fill)
        .padding([2, 8])
        .style(container::bordered_box)
        .into()
}
