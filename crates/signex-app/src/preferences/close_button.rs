//! Preferences dialog header close button.

use super::PrefMsg;
use crate::app::view::dialogs::{
    MODAL_CLOSE_X_HIT_H, MODAL_CLOSE_X_HIT_W, MODAL_CLOSE_X_HOVER, MODAL_CLOSE_X_ICON,
};
use iced::widget::{Widget, button, container, svg};
use iced::{Background, Border, Color, Element, Theme};
use signex_types::theme::ThemeId;

/// Canonical close-X — same SVG glyph and red hover footprint as the
/// shared `view::dialogs::close_x_button`, generic over the modal's
/// message type so this version can compose into a `PrefMsg`
/// element. Matches the main-window chrome close (white glyph,
/// 46×28 hit-box, top-right rounded hover, Windows-native red).
pub(super) fn close_btn<'a>(theme_id: ThemeId) -> Element<'a, PrefMsg> {
    let handle = crate::icons::icon_chrome_window_close(theme_id);
    button(
        container(
            svg(handle)
                .width(MODAL_CLOSE_X_ICON)
                .height(MODAL_CLOSE_X_ICON)
                // Resolve against the theme so the glyph stays legible on a
                // light header; the red hover footprint contrasts with the
                // near-white/near-black text colour on both palettes.
                .style(move |theme: &Theme, _| svg::Style {
                    color: Some(theme.palette().background.base.text),
                }),
        )
        .width(MODAL_CLOSE_X_HIT_W)
        .height(MODAL_CLOSE_X_HIT_H)
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center),
    )
    .padding(0)
    .on_press(PrefMsg::Close)
    .style(move |_: &Theme, status: button::Status| {
        let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
        button::Style {
            background: if hovered {
                Some(Background::Color(MODAL_CLOSE_X_HOVER))
            } else {
                None
            },
            text_color: Color::WHITE,
            border: Border {
                radius: iced::border::Radius {
                    top_left: 0.0,
                    top_right: 4.0,
                    bottom_left: 0.0,
                    bottom_right: 0.0,
                },
                ..Border::default()
            },
            ..button::Style::default()
        }
    })
    .boxed()
}
