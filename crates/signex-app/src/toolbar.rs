//! Toolbar strip — tool buttons for schematic/PCB actions.

use iced::widget::{Row, button, container, row, text};
use iced::{Background, Border, Color, Element, Length, Theme};

use crate::app::Tool;

#[derive(Debug, Clone)]
pub enum ToolMessage {
    SelectTool(Tool),
}

#[allow(dead_code)]
fn tool_btn(label: &'static str, tool: Tool, active: Tool) -> Element<'static, ToolMessage> {
    let is_active = tool == active;
    let text_c = if is_active {
        Color::WHITE
    } else {
        Color::from_rgb(0.75, 0.76, 0.80)
    };
    let btn = button(text(label).size(11).color(text_c))
        .padding([3, 7])
        .on_press(ToolMessage::SelectTool(tool))
        .style(move |_: &Theme, status: button::Status| {
            let bg = match (is_active, status) {
                (true, _) => Some(Background::Color(Color::from_rgb(0.18, 0.19, 0.25))),
                (false, button::Status::Hovered) => Some(Background::Color(Color::from_rgb(0.18, 0.19, 0.25))),
                _ => None,
            };
            button::Style {
                background: bg,
                text_color: text_c,
                border: Border::default(),
                ..button::Style::default()
            }
        });

    btn.into()
}

#[allow(dead_code)]
pub fn view(active: Tool) -> Element<'static, ToolMessage> {
    let sep = || text("|").size(10).color(Color::from_rgb(0.30, 0.31, 0.36));

    let bar: Row<'static, ToolMessage> = row![
        tool_btn("Select", Tool::Select, active),
        sep(),
        tool_btn("Wire (W)", Tool::Wire, active),
        tool_btn("Bus (B)", Tool::Bus, active),
        tool_btn("Label (L)", Tool::Label, active),
        tool_btn("Comp (P)", Tool::Component, active),
        tool_btn("Text (T)", Tool::Text, active),
        sep(),
        tool_btn("Line", Tool::Line, active),
        tool_btn("Rect", Tool::Rectangle, active),
        tool_btn("Circle", Tool::Circle, active),
    ]
    .spacing(2)
    .align_y(iced::Alignment::Center);

    container(bar)
        .width(Length::Fill)
        .padding([1, 6])
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(Color::from_rgb(0.12, 0.13, 0.16))),
            ..container::Style::default()
        })
        .into()
}
