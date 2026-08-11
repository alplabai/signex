use super::*;
use iced_aw::menu::{DrawPath, Item, Menu, MenuBar};
use iced_aw::style::menu_bar as menu_style;

mod file;
mod tools;
mod view_menu;

#[cfg(test)]
mod inventory;

#[cfg(test)]
pub(super) use inventory::{FILE_MENU_LABELS, TOOLS_MENU_LABELS, VIEW_MENU_LABELS};

const DROPDOWN_WIDTH: f32 = 300.0;
const MENU_LABEL_SIZE: f32 = 12.0;
const MENU_SHORTCUT_SIZE: f32 = 11.0;
const MENU_CHEVRON_SIZE: f32 = 18.0;

#[derive(Clone, Copy)]
struct MenuColors {
    text: Color,
    text_muted: Color,
    text_disabled: Color,
    toolbar_bg: Color,
    panel_bg: Color,
    border: Color,
    hover: Color,
}

impl MenuColors {
    fn from_tokens(tokens: &ThemeTokens) -> Self {
        let text_muted = styles::ti(tokens.text_secondary);
        Self {
            text: styles::ti(tokens.text),
            text_muted,
            text_disabled: Color {
                a: text_muted.a * 0.6,
                ..text_muted
            },
            toolbar_bg: styles::ti(tokens.toolbar_bg),
            panel_bg: styles::ti(tokens.paper),
            border: styles::ti(tokens.border),
            hover: styles::ti(tokens.hover),
        }
    }
}

pub(super) fn view(
    state: &GerberViewerState,
    tokens: &ThemeTokens,
) -> Element<'static, GerberViewerMessage> {
    let colors = MenuColors::from_tokens(tokens);
    let file_menu = file::view(state, colors);
    let view_menu = view_menu::view(state, colors);
    let tools_menu = tools::view(state, colors);

    let menu_bar = MenuBar::new(vec![file_menu, view_menu, tools_menu])
        .spacing(1.0)
        .padding([1, 4])
        .close_on_item_click_global(true)
        .close_on_background_click_global(true)
        .draw_path(DrawPath::Backdrop)
        .style(move |_theme: &Theme, _status| menu_style::Style {
            bar_background: Background::Color(colors.toolbar_bg),
            bar_border: Border::default(),
            bar_shadow: iced::Shadow::default(),
            menu_background: Background::Color(colors.panel_bg),
            menu_border: Border {
                width: 1.0,
                radius: 4.0.into(),
                color: colors.border,
            },
            menu_shadow: iced::Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.5),
                offset: iced::Vector::new(2.0, 4.0),
                blur_radius: 8.0,
            },
            path: Background::Color(colors.hover),
            path_border: Border {
                width: 1.0,
                radius: 2.0.into(),
                color: colors.border,
            },
        });

    container(menu_bar)
        .padding([0, 8])
        .width(Length::Fill)
        .style(styles::toolbar_strip(tokens))
        .into()
}

fn recent_files_menu(
    label: &str,
    colors: MenuColors,
) -> Item<'static, GerberViewerMessage, Theme, Renderer> {
    Item::with_menu(
        submenu_button(label, colors),
        dropdown(vec![leaf_stub("(No recent files)", None, colors)]),
    )
}

fn dropdown(
    items: Vec<Item<'static, GerberViewerMessage, Theme, Renderer>>,
) -> Menu<'static, GerberViewerMessage, Theme, Renderer> {
    Menu::new(items)
        .max_width(DROPDOWN_WIDTH)
        .offset(2.0)
        .spacing(2.0)
        .padding(iced::Padding {
            top: 5.0,
            right: 5.0,
            bottom: 5.0,
            left: 0.0,
        })
}

fn root_button(label: &str, colors: MenuColors) -> Element<'static, GerberViewerMessage> {
    let label = label.to_owned();
    button(text(label).size(MENU_LABEL_SIZE).color(colors.text))
        .padding([7, 6])
        .on_press(GerberViewerMessage::NoOp)
        .style(move |_: &Theme, status: button::Status| {
            let active = matches!(status, button::Status::Hovered | button::Status::Pressed,);
            button::Style {
                background: active.then_some(Background::Color(colors.hover)),
                text_color: colors.text,
                border: Border {
                    width: if active { 1.0 } else { 0.0 },
                    radius: 2.0.into(),
                    color: colors.border,
                },
                ..button::Style::default()
            }
        })
        .into()
}

fn submenu_button(label: &str, colors: MenuColors) -> Element<'static, GerberViewerMessage> {
    let content = row![
        text(label.to_owned())
            .size(MENU_LABEL_SIZE)
            .color(colors.text),
        Space::new().width(Length::Fill),
        text("›").size(MENU_CHEVRON_SIZE).color(colors.text_muted),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    button(content)
        .padding([4, 12])
        .width(Length::Fill)
        .on_press(GerberViewerMessage::NoOp)
        .style(move |_: &Theme, status: button::Status| {
            let background = matches!(status, button::Status::Hovered | button::Status::Pressed,)
                .then_some(Background::Color(colors.hover));
            button::Style {
                background,
                text_color: colors.text,
                border: Border::default(),
                ..button::Style::default()
            }
        })
        .into()
}

fn leaf(
    label: &str,
    shortcut: Option<&str>,
    message: GerberViewerMessage,
    colors: MenuColors,
) -> Item<'static, GerberViewerMessage, Theme, Renderer> {
    Item::new(menu_item_button(
        label,
        shortcut,
        Some(message),
        None,
        colors,
    ))
}

fn checked_leaf(
    label: &str,
    checked: bool,
    message: GerberViewerMessage,
    colors: MenuColors,
) -> Item<'static, GerberViewerMessage, Theme, Renderer> {
    Item::new(menu_item_button(
        label,
        None,
        Some(message),
        Some(checked),
        colors,
    ))
}

fn leaf_if(
    label: &str,
    shortcut: Option<&str>,
    message: GerberViewerMessage,
    enabled: bool,
    colors: MenuColors,
) -> Item<'static, GerberViewerMessage, Theme, Renderer> {
    if enabled {
        leaf(label, shortcut, message, colors)
    } else {
        leaf_stub(label, shortcut, colors)
    }
}

fn leaf_stub(
    label: &str,
    shortcut: Option<&str>,
    colors: MenuColors,
) -> Item<'static, GerberViewerMessage, Theme, Renderer> {
    Item::new(menu_item_button(label, shortcut, None, None, colors))
}

fn separator(colors: MenuColors) -> Item<'static, GerberViewerMessage, Theme, Renderer> {
    Item::new(
        container(Space::new())
            .height(1)
            .width(Length::Fill)
            .padding([2, 8])
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(colors.border)),
                ..container::Style::default()
            }),
    )
}

fn menu_item_button(
    label: &str,
    shortcut: Option<&str>,
    message: Option<GerberViewerMessage>,
    checked: Option<bool>,
    colors: MenuColors,
) -> Element<'static, GerberViewerMessage> {
    let enabled = message.is_some();
    let text_color = if enabled {
        colors.text
    } else {
        colors.text_disabled
    };
    let mut content = row![].spacing(8).align_y(iced::Alignment::Center);
    if let Some(checked) = checked {
        content = content.push(
            container(
                text(if checked { "✓" } else { "" })
                    .size(MENU_LABEL_SIZE)
                    .color(text_color),
            )
            .width(14),
        );
    }
    content = content.push(
        text(label.to_owned())
            .size(MENU_LABEL_SIZE)
            .color(text_color)
            .wrapping(iced::widget::text::Wrapping::None),
    );
    if let Some(shortcut) = shortcut {
        content = content.push(Space::new().width(Length::Fill)).push(
            text(shortcut.to_owned())
                .size(MENU_SHORTCUT_SIZE)
                .color(colors.text_muted)
                .wrapping(iced::widget::text::Wrapping::None),
        );
    }

    let widget = button(content.width(Length::Fill))
        .padding([4, 12])
        .width(Length::Fill)
        .style(move |_: &Theme, status: button::Status| {
            let background = if enabled {
                matches!(status, button::Status::Hovered | button::Status::Pressed,)
                    .then_some(Background::Color(colors.hover))
            } else {
                None
            };
            button::Style {
                background,
                text_color,
                border: Border {
                    radius: 2.0.into(),
                    ..Border::default()
                },
                ..button::Style::default()
            }
        });

    match message {
        Some(message) => widget.on_press(message).into(),
        None => widget.into(),
    }
}
