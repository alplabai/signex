//! Shared modal-chrome primitives — backdrop wrapper, draggable / detached
//! headers, the close-X button, and the common button / section builders
//! every dialog family reaches for.
//!
//! Extracted verbatim from `view/dialogs.rs` (ADR-0001, issue #164) as pure
//! code motion — no behaviour change.

use super::*;
use iced::widget::{Space, button, container, row, text};
use iced::{Background, Border, Color, Element, Length, Theme};

use super::{
    MODAL_CLOSE_X_HIT_H, MODAL_CLOSE_X_HIT_W, MODAL_CLOSE_X_HOVER, MODAL_CLOSE_X_ICON,
};

const BACKDROP: Color = Color::from_rgba(0.0, 0.0, 0.0, 0.55);

pub(in crate::app::view) fn wrap_modal<'a>(
    inner: Element<'a, Message>,
    offset: (f32, f32),
    window_size: (f32, f32),
    modal_size: (f32, f32),
) -> Element<'a, Message> {
    // Absolute top-left = centre + drag offset. The Translate widget (see
    // view/translate.rs) passes full parent limits to the child and then
    // translates the child's layout node, so the modal keeps its fixed
    // width/height even when positioned partially off-screen. No clamp is
    // applied here — Altium lets modals drag completely off the client
    // area and the OS window edge is the only hard boundary. If the user
    // drags the modal fully outside the window, they can dismiss it with
    // Escape (see bootstrap key handler).
    let (dx, dy) = offset;
    let (ww, wh) = window_size;
    let (mw, mh) = modal_size;
    let centre_x = (ww - mw) * 0.5;
    let centre_y = (wh - mh) * 0.5;
    let left = centre_x + dx;
    let top = centre_y + dy;

    let backdrop: Element<'a, Message> = container(iced::widget::Space::new())
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(BACKDROP)),
            ..container::Style::default()
        })
        .into();

    let positioned: Element<'a, Message> =
        super::super::translate::Translate::new(inner, (left, top)).into();

    iced::widget::stack![backdrop, positioned].into()
}

/// Wrap a header element in a mouse_area so pressing on it begins a modal
/// drag. Uses the last known mouse position as the drag anchor.
pub(in crate::app::view) fn draggable_header<'a>(
    header_content: Element<'a, Message>,
    modal: super::super::super::state::ModalId,
    last_mouse: (f32, f32),
) -> Element<'a, Message> {
    iced::widget::mouse_area(header_content)
        .on_press(Message::Overlay(OverlayMsg::ModalDragStart {
            modal,
            x: last_mouse.0,
            y: last_mouse.1,
        }))
        .into()
}

/// Borderless-window header — pressing anywhere on the header region
/// asks iced to start an OS-level window drag. Replaces the OS title
/// bar for detached modals opened with `decorations: false`.
pub(crate) fn detached_header<'a>(
    header_content: Element<'a, Message>,
    modal: super::super::super::state::ModalId,
) -> Element<'a, Message> {
    iced::widget::mouse_area(header_content)
        .on_press(Message::Window(WindowMsg::StartDetachedWindowDrag(modal)))
        .interaction(iced::mouse::Interaction::Grab)
        .into()
}

/// Compact X close button for borderless modal headers. Matches the
/// main-window chrome close (`view/mod.rs::view_main_window_chrome`):
/// no border, fully transparent at rest, Windows-native red bg + white
/// icon on hover. The `_border` argument is kept for API compatibility
/// with existing call sites — it is intentionally ignored.
pub(crate) fn close_x_button(
    message: Message,
    theme_id: signex_types::theme::ThemeId,
    text_color: Color,
) -> Element<'static, Message> {
    // Use the same SVG and footprint as the main-window chrome close
    // (`view::view_main_window_chrome::chrome_btn`) so the modal X is
    // visually identical to the OS-window X, including stroke weight,
    // hit-box dimensions, and red-on-hover behaviour. Glyph is
    // ALWAYS white per design — `text_color` is ignored for the
    // icon. Some themes' text colour was washing the X out against
    // the toolbar bg.
    let _ = text_color;
    use iced::widget::svg;
    let handle = crate::icons::icon_chrome_window_close(theme_id);
    button(
        container(
            svg(handle)
                .width(MODAL_CLOSE_X_ICON)
                .height(MODAL_CLOSE_X_ICON)
                .style(move |_: &Theme, _| svg::Style {
                    color: Some(Color::WHITE),
                }),
        )
        .width(MODAL_CLOSE_X_HIT_W)
        .height(MODAL_CLOSE_X_HIT_H)
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center),
    )
    .padding(0)
    .on_press(message)
    .style(move |_: &Theme, status: button::Status| {
        let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
        // Top-right radius matches the modal card's outer corner so
        // the red hover background fills the rounded corner cleanly
        // — same trick the OS chrome close uses on Windows 11.
        let radius = iced::border::Radius::default().top_right(crate::styles::MODAL_CORNER_RADIUS);
        button::Style {
            background: if hovered {
                Some(Background::Color(MODAL_CLOSE_X_HOVER))
            } else {
                None
            },
            border: Border {
                radius,
                ..Border::default()
            },
            // Always white — keeps glyph readable on any theme +
            // the destructive red on hover.
            text_color: Color::WHITE,
            ..button::Style::default()
        }
    })
    .into()
}

// `detach_button` was removed once the three big modals started
// opening as separate OS windows by default (see
// `handle_open_annotate_dialog` et al.). Drag-off is no longer needed
// because there is no in-window overlay to drag.

#[allow(dead_code)]
fn close_button(
    label: &str,
    message: Message,
    text_color: Color,
    border: Color,
) -> Element<'_, Message> {
    button(container(text(label.to_string()).size(11).color(text_color)).padding([3, 10]))
        .on_press(message)
        .style(move |_: &Theme, _| button::Style {
            background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.03))),
            border: Border {
                width: 1.0,
                radius: 3.0.into(),
                color: border,
            },
            text_color,
            ..button::Style::default()
        })
        .into()
}

/// Subtle section divider used inside the BOM modal's properties
/// sidebar. Rendered as a 1-line label on a slightly tinted strip
/// so the panel reads as a stack of named sections.
pub(super) fn section_header(title: &str, muted: Color) -> Element<'_, Message> {
    container(
        row![
            text("\u{25BE}").size(10).color(muted),
            Space::new().width(6),
            text(title.to_string()).size(11).color(muted),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([6, 12])
    .width(Length::Fill)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.025))),
        ..container::Style::default()
    })
    .into()
}

pub(super) fn secondary_button(
    label: &str,
    message: Message,
    text_color: Color,
    border: Color,
) -> Element<'_, Message> {
    button(container(text(label.to_string()).size(11).color(text_color)).padding([5, 14]))
        .on_press(message)
        .style(move |_: &Theme, _| button::Style {
            background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.04))),
            border: Border {
                width: 1.0,
                radius: 4.0.into(),
                color: border,
            },
            text_color,
            ..button::Style::default()
        })
        .into()
}

pub(super) fn primary_button(label: &str, message: Option<Message>, border: Color) -> Element<'_, Message> {
    primary_button_themed(label, message, border, None)
}

/// Theme-aware primary button. Pass `Some(accent)` to use the
/// theme's accent colour as the button bg (Altium-amber on Signex,
/// cyan on Alp Lab, etc.). Pass `None` to fall back to the legacy
/// hardcoded blue (existing call sites that haven't been migrated).
pub(super) fn primary_button_themed(
    label: &str,
    message: Option<Message>,
    border: Color,
    accent: Option<Color>,
) -> Element<'_, Message> {
    let enabled = message.is_some();
    let active_bg = accent.unwrap_or(Color::from_rgb(0.00, 0.47, 0.84));
    let bg = if enabled {
        active_bg
    } else {
        Color::from_rgba(1.0, 1.0, 1.0, 0.04)
    };
    let fg = if enabled {
        Color::WHITE
    } else {
        Color::from_rgba(1.0, 1.0, 1.0, 0.4)
    };
    let mut b = button(container(text(label.to_string()).size(11).color(fg)).padding([5, 14]))
        .style(move |_: &Theme, _| button::Style {
            background: Some(Background::Color(bg)),
            border: Border {
                width: 1.0,
                radius: 4.0.into(),
                color: border,
            },
            text_color: fg,
            ..button::Style::default()
        });
    if let Some(msg) = message {
        b = b.on_press(msg);
    }
    b.into()
}
