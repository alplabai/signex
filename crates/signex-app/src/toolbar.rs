//! Toolbar strip — icon buttons for common schematic/PCB actions.

use iced::widget::{button, container, row, text, Row};
use iced::{Element, Length};

use crate::app::Tool;

#[derive(Debug, Clone)]
pub enum ToolMessage {
    SelectTool(Tool),
}

fn tool_btn(label: &'static str, tool: Tool, active: Tool) -> Element<'static, ToolMessage> {
    let btn = button(text(label).size(12))
        .padding([4, 8])
        .on_press(ToolMessage::SelectTool(tool));
    if tool == active {
        btn.style(button::primary).into()
    } else {
        btn.style(button::secondary).into()
    }
}

/// Render the toolbar as a row of tool buttons.
pub fn view(active: Tool) -> Element<'static, ToolMessage> {
    let bar: Row<'static, ToolMessage> = row![
        tool_btn("Select", Tool::Select, active),
        tool_btn("Wire (W)", Tool::Wire, active),
        tool_btn("Bus (B)", Tool::Bus, active),
        tool_btn("Label (L)", Tool::Label, active),
        tool_btn("Comp (P)", Tool::Component, active),
        tool_btn("Text (T)", Tool::Text, active),
        text(" | ").size(12),
        tool_btn("Line", Tool::Line, active),
        tool_btn("Rect", Tool::Rectangle, active),
        tool_btn("Circle", Tool::Circle, active),
    ]
    .spacing(2)
    .align_y(iced::Alignment::Center);

    container(bar)
        .width(Length::Fill)
        .padding([2, 8])
        .into()
}
