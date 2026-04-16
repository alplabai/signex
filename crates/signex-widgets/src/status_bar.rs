//! Status bar widget — themed, composable bottom bar with sections.
//!
//! Supports text, key-value labels, and on/off indicators. Sections can be
//! clickable. All colors from `ThemeTokens`.

use iced::widget::{Row, button, container, row, space, text};
use iced::{Border, Element, Length};
use signex_types::theme::ThemeTokens;

use crate::theme_ext;

// ---------------------------------------------------------------------------
// Data model
// ---------------------------------------------------------------------------

/// Content variants for a status bar section.
#[derive(Debug, Clone)]
pub enum StatusContent {
    /// Plain text.
    Text(String),
    /// Key-value pair displayed as "key: value".
    Label { key: String, value: String },
    /// Boolean indicator displayed as a colored dot.
    Indicator { label: String, on: bool },
}

/// A single section in the status bar.
#[derive(Debug, Clone)]
pub struct StatusSection {
    /// What to display.
    pub content: StatusContent,
    /// Whether clicking this section emits a message.
    pub clickable: bool,
}

// ---------------------------------------------------------------------------
// Messages
// ---------------------------------------------------------------------------

/// Messages emitted by the status bar.
#[derive(Debug, Clone)]
pub enum StatusBarMsg {
    /// A clickable section was clicked. Payload is the section index.
    SectionClicked(usize),
}

// ---------------------------------------------------------------------------
// View
// ---------------------------------------------------------------------------

/// Render a themed status bar.
///
/// * `left`   — sections aligned to the left.
/// * `right`  — sections aligned to the right.
/// * `tokens` — theme tokens for all colors.
pub fn status_bar<'a>(
    left: &[StatusSection],
    right: &[StatusSection],
    tokens: &ThemeTokens,
) -> Element<'a, StatusBarMsg> {
    let text_color = theme_ext::text_primary(tokens);
    let muted_color = theme_ext::text_secondary(tokens);
    let border_c = theme_ext::border_color(tokens);
    let success_c = theme_ext::success_color(tokens);

    let sep = || text("|").size(10).color(border_c);

    // Build left sections
    let mut left_row: Row<'_, StatusBarMsg> =
        Row::new().spacing(4).align_y(iced::Alignment::Center);
    for (i, section) in left.iter().enumerate() {
        if i > 0 {
            left_row = left_row.push(sep());
        }
        left_row = left_row.push(render_section(
            section,
            i,
            text_color,
            muted_color,
            success_c,
        ));
    }

    // Build right sections
    let right_offset = left.len();
    let mut right_row: Row<'_, StatusBarMsg> =
        Row::new().spacing(4).align_y(iced::Alignment::Center);
    for (i, section) in right.iter().enumerate() {
        if i > 0 {
            right_row = right_row.push(sep());
        }
        right_row = right_row.push(render_section(
            section,
            right_offset + i,
            text_color,
            muted_color,
            success_c,
        ));
    }

    // Compose: left — spacer — right
    let bar = row![left_row, space::horizontal(), right_row]
        .spacing(4)
        .align_y(iced::Alignment::Center);

    let statusbar_bg = theme_ext::to_color(&tokens.statusbar_bg);
    let bar_text_color = text_color;
    let bar_border_color = border_c;

    container(bar)
        .width(Length::Fill)
        .padding([2, 8])
        .style(move |_theme: &iced::Theme| container::Style {
            background: Some(statusbar_bg.into()),
            text_color: Some(bar_text_color),
            border: Border {
                width: 1.0,
                radius: 0.0.into(),
                color: bar_border_color,
            },
            ..container::Style::default()
        })
        .into()
}

// ---------------------------------------------------------------------------
// Section rendering
// ---------------------------------------------------------------------------

fn render_section<'a>(
    section: &StatusSection,
    index: usize,
    text_color: iced::Color,
    muted_color: iced::Color,
    success_color: iced::Color,
) -> Element<'a, StatusBarMsg> {
    let content: Element<'a, StatusBarMsg> = match &section.content {
        StatusContent::Text(s) => text(s.clone()).size(11).color(text_color).into(),

        StatusContent::Label { key, value } => row![
            text(key.clone()).size(11).color(muted_color),
            text(": ").size(11).color(muted_color),
            text(value.clone()).size(11).color(text_color),
        ]
        .spacing(0)
        .align_y(iced::Alignment::Center)
        .into(),

        StatusContent::Indicator { label, on } => {
            let (dot, dot_color) = if *on {
                ("●", success_color)
            } else {
                ("○", muted_color)
            };
            row![
                text(dot).size(11).color(dot_color),
                text(" ").size(11),
                text(label.clone()).size(11).color(text_color),
            ]
            .spacing(0)
            .align_y(iced::Alignment::Center)
            .into()
        }
    };

    if section.clickable {
        button(content)
            .padding([1, 4])
            .style(button::text)
            .on_press(StatusBarMsg::SectionClicked(index))
            .into()
    } else {
        content
    }
}
