//! Toolbar strip — tool buttons for schematic/PCB actions.

use iced::widget::{button, container, row, text, Row};
use iced::{Background, Border, Color, Element, Length, Theme};

use crate::app::Tool;
use crate::styles;

#[derive(Debug, Clone)]
pub enum ToolMessage {
    SelectTool(Tool),
}

fn tool_btn(label: &'static str, tool: Tool, active: Tool) -> Element<'static, ToolMessage> {
    let is_active = tool == active;
    let text_c = if is_active {
        Color::WHITE
    } else {
        styles::TEXT_PRIMARY
    };
    let btn = button(text(label).size(11).color(text_c))
        .padding([3, 7])
        .on_press(ToolMessage::SelectTool(tool))
        .style(move |_: &Theme, status: button::Status| {
            let bg = match (is_active, status) {
                (true, _) => Some(Background::Color(styles::TAB_ACTIVE_BG)),
                (false, button::Status::Hovered) => {
                    Some(Background::Color(styles::TAB_ACTIVE_BG))
                }
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

pub fn view(active: Tool) -> Element<'static, ToolMessage> {
    let sep = || text("|").size(10).color(styles::BORDER_COLOR);

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
        .style(styles::toolbar_strip)
        .into()
}
