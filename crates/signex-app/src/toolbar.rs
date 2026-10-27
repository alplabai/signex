//! Toolbar strip — tool buttons for schematic/PCB actions.

use iced::widget::{Row, button, container, row, text};
use iced::{Background, Border, Element, Length, Theme};
use signex_types::theme::ThemeTokens;

use crate::app::Tool;
use crate::styles::ti;
use crate::styles::tokens::{self, Overlay};

#[derive(Debug, Clone)]
pub enum ToolMessage {
    SelectTool(Tool),
    /// On TAB during placement: commit the ghost at the cursor as a
    /// real object (or pause a footprint-pad placement).
    PrePlacementTab,
    /// Resume placement after TAB paused it — clears `pre_placement`
    /// and `placement_paused`. Wired to the on-canvas "Resume" overlay.
    ResumePlacement,
    /// Cycle the freeform draw mode (line / arc / …).
    CycleDrawMode,
    /// Cancel an in-progress wire/bus draw and reset to the select tool.
    CancelDrawing,
    /// Backspace/Delete: drop the pending wire vertex while drawing,
    /// otherwise delete the selection.
    RemoveLastVertex,
}

fn tool_btn(
    label: &'static str,
    tool: Tool,
    active: Tool,
    tokens: &ThemeTokens,
) -> Element<'static, ToolMessage> {
    let is_active = tool == active;
    let text_c = if is_active {
        ti(tokens.text)
    } else {
        tokens::muted_text(tokens)
    };
    let active_bg = tokens::overlay(tokens, Overlay::Pressed);
    let hover_bg = tokens::hover_overlay(tokens);
    let btn = button(text(label).size(11).color(text_c))
        .padding([3, 7])
        .on_press(ToolMessage::SelectTool(tool))
        .style(move |_: &Theme, status: button::Status| {
            let bg = match (is_active, status) {
                (true, _) => Some(Background::Color(active_bg)),
                (false, button::Status::Hovered) => Some(Background::Color(hover_bg)),
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

pub fn view(active: Tool, tokens: &ThemeTokens) -> Element<'static, ToolMessage> {
    let sep_c = ti(tokens.border);
    let sep = move || text("|").size(10).color(sep_c);

    let bar: Row<'static, ToolMessage> = row![
        tool_btn("Select", Tool::Select, active, tokens),
        sep(),
        tool_btn("Wire (W)", Tool::Wire, active, tokens),
        tool_btn("Bus (B)", Tool::Bus, active, tokens),
        tool_btn("Label (L)", Tool::Label, active, tokens),
        tool_btn("Comp (P)", Tool::Component, active, tokens),
        tool_btn("Text (T)", Tool::Text, active, tokens),
        tool_btn("Sheet (S)", Tool::Sheet, active, tokens),
        sep(),
        tool_btn("Line", Tool::Line, active, tokens),
        tool_btn("Rect", Tool::Rectangle, active, tokens),
        tool_btn("Circle", Tool::Circle, active, tokens),
    ]
    .spacing(2)
    .align_y(iced::Alignment::Center);

    container(bar)
        .width(Length::Fill)
        .padding([1, 6])
        .style(crate::styles::toolbar_strip(tokens))
        .into()
}
