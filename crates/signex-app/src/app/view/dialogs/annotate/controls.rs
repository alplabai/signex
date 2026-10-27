//! Small read-only form controls used by the Annotate modal — the inline
//! checkbox pip, the order-of-processing radio pill, the 2×2 order-preview
//! legend, and the shared bordered-container style.
//!
//! Moved verbatim out of `dialogs/annotate.rs` (ADR-0001, issue #164) as
//! pure code motion — no behaviour change. These are free helper fns local
//! to the annotate concern; kept `pub(super)` so only `annotate/mod.rs`
//! reaches them.

use super::super::*;
use iced::widget::{Space, button, column, container, row, text};
use iced::{Background, Border, Color, Element, Theme};

use crate::app::state::AnnotateOrder;

/// Tiny inline checkbox pip — read-only indicator used inside the Annotate
/// dialog's parameter list and sheet table.
pub(super) fn check_pip(on: bool, border: Color) -> Element<'static, Message> {
    let inner = if on {
        text("✓").size(9).color(Color::WHITE)
    } else {
        text(" ").size(9).color(Color::WHITE)
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
            },
            ..container::Style::default()
        })
        .into()
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
) -> Element<'static, Message> {
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
        container(text(label.to_string()).size(10).color(text_c))
            .width(34)
            .height(20)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.04))),
                border: Border {
                    width: 1.0,
                    radius: 2.0.into(),
                    color: border,
                },
                ..container::Style::default()
            })
            .into()
    };
    let arrow = match order {
        AnnotateOrder::UpThenAcross => "↑→",
        AnnotateOrder::DownThenAcross => "↓→",
        AnnotateOrder::AcrossThenDown => "→↓",
        AnnotateOrder::AcrossThenUp => "→↑",
    };
    container(
        column![
            text("Preview").size(9).color(text_muted),
            Space::new().height(2),
            row![cell(slots.0), Space::new().width(4), cell(slots.1),].spacing(0),
            Space::new().height(4),
            row![cell(slots.2), Space::new().width(4), cell(slots.3),].spacing(0),
            Space::new().height(2),
            text(arrow).size(11).color(text_muted),
        ]
        .spacing(0)
        .align_x(iced::Alignment::Center),
    )
    .padding(6)
    .style(bordered_style(border))
    .into()
}

pub(super) fn bordered_style(border: Color) -> impl Fn(&Theme) -> container::Style + 'static {
    move |_: &Theme| container::Style {
        border: Border {
            width: 1.0,
            radius: 3.0.into(),
            color: border,
        },
        background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.02))),
        ..container::Style::default()
    }
}

pub(super) fn order_radio(
    label: &str,
    value: AnnotateOrder,
    current: AnnotateOrder,
    text_c: Color,
    border: Color,
) -> Element<'_, Message> {
    let selected = value == current;
    let bg = if selected {
        Color::from_rgb(0.00, 0.47, 0.84)
    } else {
        Color::from_rgba(1.0, 1.0, 1.0, 0.04)
    };
    let fg = if selected { Color::WHITE } else { text_c };
    button(container(text(label.to_string()).size(11).color(fg)).padding([4, 10]))
        .on_press(Message::Annotate(AnnotateMsg::OrderChanged(value)))
        .style(move |_: &Theme, _| button::Style {
            background: Some(Background::Color(bg)),
            border: Border {
                width: 1.0,
                radius: 3.0.into(),
                color: border,
            },
            text_color: fg,
            ..button::Style::default()
        })
        .into()
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
             and renumbers them, across the whole project, as one undo step. Sheets that are \
             not open are rewritten on disk and cannot be undone. Locked parts, hand-edited \
             prefixes and parts without a library link are left alone and listed in Messages. \
             PCB references are not updated."
        ),
    }
}
