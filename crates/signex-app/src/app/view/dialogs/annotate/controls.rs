//! Small read-only form controls used by the Annotate modal — the inline
//! checkbox pip, the order-of-processing radio pill, the 2×2 order-preview
//! legend, and the shared bordered-container style. On Graphite each one
//! delegates to `graphite_annotate`; other themes keep the original look.

use super::super::*;
use iced::widget::{Space, button, column, container, row, text};
use iced::{Background, Border, Color, Element, Theme};
use signex_types::theme::ThemeTokens;

use super::super::graphite_annotate as ga;
use crate::app::state::AnnotateOrder;
use crate::styles::is_graphite;

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// Tiny inline checkbox pip — read-only indicator used inside the Annotate
/// dialog's parameter list and sheet table.
pub(super) fn check_pip(
    on: bool,
    border: Color,
    theme_tokens: &ThemeTokens,
) -> Element<'static, Message> {
    if is_graphite(theme_tokens) {
        return ga::checkbox_pip(theme_tokens, on);
    }
    let inner = if on {
        text("✓").size(TS.caption).color(Color::WHITE)
    } else {
        text(" ").size(TS.caption).color(Color::WHITE)
    };
    let bg = if on {
        Color::from_rgb(0.00, 0.47, 0.84)
    } else {
        Color::from_rgba(1.0, 1.0, 1.0, 0.04)
    };
    container(inner)
        .width(12)
        .height(12)
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(bg)),
            border: Border {
                width: 1.0,
                radius: 2.0.into(),
                color: border,
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// Compact visual *legend* of the annotate order — this is intentionally a
/// static R1..R4 diagram that illustrates how four parts arranged in a 2×2
/// grid would be numbered under the selected traversal. It does NOT reflect
/// the user's actual components; it's the same convention Altium uses.
pub(super) fn order_preview(
    order: AnnotateOrder,
    text_c: Color,
    text_muted: Color,
    border: Color,
    theme_tokens: &ThemeTokens,
) -> Element<'static, Message> {
    let graphite = is_graphite(theme_tokens);
    // Pick labels for each of the four slots (top-left, top-right, bottom-left,
    // bottom-right) matching the selected order.
    // Slot layout:
    //   (0,0) tl   (0,1) tr
    //   (1,0) bl   (1,1) br
    let slots = match order {
        // Column-major, bottom to top within the column (y decreasing):
        //  2 4
        //  1 3
        AnnotateOrder::UpThenAcross => ("R2", "R4", "R1", "R3"),
        // Column-major, top to bottom within the column (y increasing):
        //  1 3
        //  2 4
        AnnotateOrder::DownThenAcross => ("R1", "R3", "R2", "R4"),
        // Row-major, top row first (y increasing):
        //  1 2
        //  3 4
        AnnotateOrder::AcrossThenDown => ("R1", "R2", "R3", "R4"),
        // Row-major, bottom row first (y decreasing):
        //  3 4
        //  1 2
        AnnotateOrder::AcrossThenUp => ("R3", "R4", "R1", "R2"),
    };
    let cell = |label: &'static str| -> Element<'static, Message> {
        let label_text = text(label.to_string()).size(TS.caption).color(text_c);
        let label_text = if graphite {
            label_text.font(ga::mono())
        } else {
            label_text
        };
        let framed = container(label_text)
            .width(34)
            .height(20)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center);
        if graphite {
            return framed.style(ga::preview_cell(theme_tokens)).boxed();
        }
        framed
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.04))),
                border: Border {
                    width: 1.0,
                    radius: 2.0.into(),
                    color: border,
                    ..Border::default()
                },
                ..container::Style::default()
            })
            .boxed()
    };
    let arrow = match order {
        AnnotateOrder::UpThenAcross => "↑→",
        AnnotateOrder::DownThenAcross => "↓→",
        AnnotateOrder::AcrossThenDown => "→↓",
        AnnotateOrder::AcrossThenUp => "→↑",
    };
    container(
        column![
            text("Preview").size(TS.caption).color(text_muted),
            Space::new().height(2),
            row![cell(slots.0), Space::new().width(4), cell(slots.1),].spacing(0),
            Space::new().height(4),
            row![cell(slots.2), Space::new().width(4), cell(slots.3),].spacing(0),
            Space::new().height(2),
            text(arrow).size(TS.body).color(text_muted),
        ]
        .spacing(0)
        .align_x(iced::Alignment::Center),
    )
    .padding(6)
    .style(bordered_style(border, theme_tokens))
    .boxed()
}

pub(super) fn bordered_style(
    border: Color,
    theme_tokens: &ThemeTokens,
) -> Box<dyn Fn(&Theme) -> container::Style + 'static> {
    if is_graphite(theme_tokens) {
        return Box::new(ga::field_box(theme_tokens));
    }
    Box::new(move |_: &Theme| container::Style {
        border: Border {
            width: 1.0,
            radius: 3.0.into(),
            color: border,
            ..Border::default()
        },
        background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.02))),
        ..container::Style::default()
    })
}

/// Style of the per-row lock toggle.
pub(super) fn lock_style(
    theme_tokens: &ThemeTokens,
    is_locked: bool,
    border: Color,
    text_c: Color,
) -> Box<dyn Fn(&Theme, button::Status) -> button::Style + 'static> {
    if is_graphite(theme_tokens) {
        return Box::new(ga::toggle_style(theme_tokens, is_locked));
    }
    Box::new(move |_: &Theme, _| button::Style {
        background: Some(Background::Color(if is_locked {
            Color::from_rgba(1.0, 1.0, 1.0, 0.12)
        } else {
            Color::from_rgba(1.0, 1.0, 1.0, 0.03)
        })),
        border: Border {
            width: 1.0,
            radius: 2.0.into(),
            color: border,
            ..Border::default()
        },
        text_color: text_c,
        ..button::Style::default()
    })
}

pub(super) fn order_radio<'a>(
    label: &'a str,
    value: AnnotateOrder,
    current: AnnotateOrder,
    text_c: Color,
    border: Color,
    theme_tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let selected = value == current;
    if is_graphite(theme_tokens) {
        return button(container(text(label.to_string()).size(TS.body)).padding([SP.s, 10.0]))
            .on_press(Message::Annotate(AnnotateMsg::OrderChanged(value)))
            .style(ga::toggle_style(theme_tokens, selected))
            .boxed();
    }
    let bg = if selected {
        Color::from_rgb(0.00, 0.47, 0.84)
    } else {
        Color::from_rgba(1.0, 1.0, 1.0, 0.04)
    };
    let fg = if selected { Color::WHITE } else { text_c };
    button(container(text(label.to_string()).size(TS.body).color(fg)).padding([SP.s, 10.0]))
        .on_press(Message::Annotate(AnnotateMsg::OrderChanged(value)))
        .style(move |_: &Theme, _| button::Style {
            background: Some(Background::Color(bg)),
            border: Border {
                width: 1.0,
                radius: 3.0.into(),
                color: border,
                ..Border::default()
            },
            text_color: fg,
            ..button::Style::default()
        })
        .boxed()
}

/// The line under the Annotation Summary that says what "Re-prefix to
/// standard" will do, or why it is unavailable (#1315).
pub(super) fn reprefix_note(
    standard: Option<signex_types::designator::DesignatorStandard>,
) -> String {
    match standard {
        None => "Re-prefix to standard is unavailable: this project follows no designator \
                 standard. Pick one in Project Options."
            .to_string(),
        Some(standard) => format!(
            "Re-prefix to standard re-issues the prefix of every placed part under {standard} \
             and renumbers them, across the whole project, as one undo step, after you confirm a \
             list of the changes. Sheets that are not open are loaded unsaved; nothing is \
             written until you save. Locked parts, hand-edited prefixes and parts without a \
             library link are left alone and listed in Messages. PCB references are not \
             updated."
        ),
    }
}
