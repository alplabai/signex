//! The divider between the component list and the details section.
//!
//! The list height lives in `ComponentsPanelState::list_height`; the details
//! section takes the rest. Sizes derive from the row text size.

use iced::widget::{Space, column, container};
use iced::{Background, Border, Color, Element, Length, Theme, Widget};
use signex_types::theme::ThemeTokens;

use super::HINT_TEXT_SIZE;
use crate::library::messages::LibraryMessage;

/// Smallest list height: a few rows stay visible.
pub const MIN_LIST_HEIGHT: f32 = HINT_TEXT_SIZE * 8.0;
/// Largest list height, so the details section never vanishes.
pub const MAX_LIST_HEIGHT: f32 = HINT_TEXT_SIZE * 60.0;
/// List height before the user drags the divider.
pub const DEFAULT_LIST_HEIGHT: f32 = HINT_TEXT_SIZE * 25.0;
/// Thickness of the draggable divider.
pub const HANDLE_HEIGHT: f32 = HINT_TEXT_SIZE * 0.64;
/// Width of the grip centred on the divider.
const GRIP_WIDTH: f32 = HINT_TEXT_SIZE * 2.5;
/// Height of the grip.
const GRIP_HEIGHT: f32 = HINT_TEXT_SIZE * 0.27;

/// `height` limited to the allowed range.
pub fn clamp_list_height(height: f32) -> f32 {
    height.clamp(MIN_LIST_HEIGHT, MAX_LIST_HEIGHT)
}

/// The divider's colours: theme lines at rest, the accent while hovered or
/// dragged.
#[derive(Debug, Clone, Copy, PartialEq)]
struct HandleLook {
    edge: Color,
    grip: Color,
}

impl HandleLook {
    fn of(is_active: bool, tokens: &ThemeTokens) -> Self {
        if is_active {
            let accent = crate::styles::ti(tokens.accent);
            Self {
                edge: accent,
                grip: accent,
            }
        } else {
            Self {
                edge: crate::styles::ti(tokens.border),
                grip: crate::styles::tokens::resolve(tokens).line_strong,
            }
        }
    }
}

fn filled(color: Color, radius: f32) -> container::Style {
    container::Style {
        background: Some(Background::Color(color)),
        border: Border {
            radius: radius.into(),
            ..Border::default()
        },
        ..container::Style::default()
    }
}

/// The draggable divider: a full-width line with a centred grip.
pub(super) fn handle<'a>(
    is_hovered: bool,
    is_dragging: bool,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let look = HandleLook::of(is_hovered || is_dragging, tokens);
    let grip = container(Space::new())
        .width(GRIP_WIDTH)
        .height(GRIP_HEIGHT)
        .style(move |_: &Theme| filled(look.grip, GRIP_HEIGHT / 2.0));
    let edge = container(Space::new())
        .width(Length::Fill)
        .height(crate::styles::HAIRLINE)
        .style(move |_: &Theme| filled(look.edge, 0.0));
    let body = column![
        edge,
        container(grip)
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    ]
    .width(Length::Fill)
    .height(HANDLE_HEIGHT);
    iced::widget::mouse_area(body)
        .interaction(iced::mouse::Interaction::ResizingVertically)
        .on_press(LibraryMessage::ComponentsPanelSplitDragStarted)
        .on_enter(LibraryMessage::ComponentsPanelSplitHovered(true))
        .on_exit(LibraryMessage::ComponentsPanelSplitHovered(false))
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::theme::{ThemeId, theme_tokens};

    #[test]
    fn the_handle_uses_the_theme_lines_at_rest_and_the_accent_when_active() {
        for id in ThemeId::BUILTINS {
            let tokens = theme_tokens(*id);
            let rest = HandleLook::of(false, &tokens);
            assert_eq!(rest.edge, crate::styles::ti(tokens.border), "{id:?}");
            assert_eq!(
                rest.grip,
                crate::styles::tokens::resolve(&tokens).line_strong,
                "{id:?}"
            );
            let active = HandleLook::of(true, &tokens);
            assert_eq!(active.edge, crate::styles::ti(tokens.accent), "{id:?}");
            assert_eq!(active.grip, active.edge, "{id:?}");
            assert_ne!(active.grip, rest.grip, "{id:?}");
        }
    }

    #[test]
    fn the_handle_paints_a_visible_line_and_grip_that_change_when_active() {
        const WINDOW: iced::Size = iced::Size::new(200.0, 8.0);
        assert!(GRIP_WIDTH > 0.0 && GRIP_HEIGHT > 0.0 && HANDLE_HEIGHT > GRIP_HEIGHT);
        for id in ThemeId::BUILTINS {
            let tokens = theme_tokens(*id);
            let rest = crate::layout_probe::draw_pixels(handle(false, false, &tokens), WINDOW);
            assert!(
                rest.chunks_exact(4).any(|p| p[..3] != [0, 0, 0]),
                "{id:?}: the divider paints something at rest"
            );
            let hovered = crate::layout_probe::draw_pixels(handle(true, false, &tokens), WINDOW);
            let dragged = crate::layout_probe::draw_pixels(handle(false, true, &tokens), WINDOW);
            assert_ne!(rest, hovered, "{id:?}: hover changes the divider");
            assert_eq!(hovered, dragged, "{id:?}: a drag looks like a hover");
        }
    }

    #[test]
    fn heights_clamp_to_the_range() {
        assert_eq!(clamp_list_height(0.0), MIN_LIST_HEIGHT);
        assert_eq!(clamp_list_height(1.0e6), MAX_LIST_HEIGHT);
        assert_eq!(clamp_list_height(DEFAULT_LIST_HEIGHT), DEFAULT_LIST_HEIGHT);
    }
}
