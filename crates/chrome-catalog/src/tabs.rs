use iced::widget::{Column, Row, Space, column, container, text};
use iced::{Background, Color, Element, Length, Theme};
use signex_types::theme::ThemeTokens;
use signex_widgets::tab_pill::{AccentPosition, TabPill, TabPillStyle};

use crate::catalog::Message;
use crate::theme;

pub(crate) fn document_strip<'a>(tokens: &ThemeTokens) -> Element<'a, Message> {
    let tabs_row = Row::new()
        .spacing(0)
        .push(tab(
            "MCU_IO",
            false,
            false,
            false,
            false,
            AccentPosition::Bottom,
            tokens,
        ))
        .push(tab(
            "Loratis-SN",
            true,
            false,
            false,
            false,
            AccentPosition::Bottom,
            tokens,
        ))
        .push(tab(
            "Power",
            false,
            false,
            false,
            true,
            AccentPosition::Bottom,
            tokens,
        ));
    strip_with_baseline(tabs_row, AccentPosition::Bottom, tokens)
}

pub(crate) fn state_matrix<'a>(tokens: &ThemeTokens) -> Element<'a, Message> {
    let mut tabs: Row<'a, Message> = Row::new().spacing(0);
    for (label, active, dragging, hovered, last) in [
        ("Inactive", false, false, false, false),
        ("Hovered", false, false, true, false),
        ("Active", true, false, false, false),
        ("Dragging", false, true, false, false),
        ("Last", false, false, false, true),
    ] {
        tabs = tabs.push(tab(
            label,
            active,
            dragging,
            hovered,
            last,
            AccentPosition::Bottom,
            tokens,
        ));
    }
    strip_with_baseline(tabs, AccentPosition::Bottom, tokens)
}

pub(crate) fn panel_strip<'a>(tokens: &ThemeTokens) -> Element<'a, Message> {
    let mut tabs: Row<'a, Message> = Row::new().spacing(0);
    for (index, label) in [
        "Components",
        "Manufacturer Part Search",
        "PCB CoDesign",
        "Messages",
        "Properties",
    ]
    .iter()
    .enumerate()
    {
        tabs = tabs.push(tab(
            label,
            index == 0,
            false,
            false,
            index == 4,
            AccentPosition::Top,
            tokens,
        ));
    }
    strip_with_baseline(tabs, AccentPosition::Top, tokens)
}

fn strip_with_baseline<'a>(
    tabs: Row<'a, Message>,
    accent_position: AccentPosition,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let toolbar_bg = theme::color(tokens.toolbar_bg);
    let baseline_color = theme::color(tokens.border);
    let baseline = container(Space::new())
        .width(Length::Fill)
        .height(1)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(baseline_color)),
            ..container::Style::default()
        });
    let row_padding = match accent_position {
        AccentPosition::Bottom => iced::Padding {
            top: 2.0,
            right: 6.0,
            bottom: 0.0,
            left: 6.0,
        },
        AccentPosition::Top => iced::Padding {
            top: 0.0,
            right: 6.0,
            bottom: 2.0,
            left: 6.0,
        },
    };
    let row_container = container(tabs).width(Length::Fill).padding(row_padding);
    let inner: Column<'a, Message> = match accent_position {
        AccentPosition::Bottom => column![row_container, baseline],
        AccentPosition::Top => column![baseline, row_container],
    };
    container(inner)
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(toolbar_bg)),
            ..container::Style::default()
        })
        .into()
}

fn tab<'a>(
    label: &str,
    is_active: bool,
    is_dragging: bool,
    is_hovered: bool,
    is_last: bool,
    accent_position: AccentPosition,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let text_primary = theme::color(tokens.text);
    let text_muted = theme::color(tokens.text_secondary);
    let tab_active = theme::color(tokens.hover);
    let accent = theme::color(tokens.accent);
    let fill = if is_dragging {
        Color { a: 0.22, ..accent }
    } else if is_active {
        tab_active
    } else if is_hovered {
        Color {
            a: tab_active.a * 0.70,
            ..tab_active
        }
    } else {
        Color {
            a: tab_active.a * 0.35,
            ..tab_active
        }
    };
    let pill_style = TabPillStyle {
        fill,
        border: theme::color(tokens.border),
        accent,
        is_active,
        is_last,
        accent_position,
    };
    let text_color = if is_active { text_primary } else { text_muted };
    let content = container(text(label.to_string()).size(11).color(text_color)).padding([4, 10]);
    TabPill::new(content, pill_style).into()
}
