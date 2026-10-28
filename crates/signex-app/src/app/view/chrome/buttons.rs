//! The window-control buttons of the main window's strip, one small builder
//! per [`WindowControlsStyle`], plus the pure width bookkeeping the search
//! bar geometry reads.

use iced::widget::{Space, button, container, row, svg};
use iced::{Alignment, Background, Border, Color, Element, Length};
use signex_types::theme::ThemeId;

use crate::app::{Message, WindowMsg};
use crate::chrome::controls::{WindowButton, WindowControlsLayout, WindowControlsStyle};
use crate::menu_bar::MENU_BAR_HEIGHT;

/// One Windows-style button.
pub(crate) const WINDOWS_BTN_W: f32 = 46.0;
const WINDOWS_ICON: f32 = 14.0;

/// Linux (libadwaita-like) round button: diameter, icon, gap, edge padding.
const LINUX_BTN: f32 = 24.0;
const LINUX_ICON: f32 = 16.0;
const LINUX_GAP: f32 = 6.0;
const LINUX_EDGE: f32 = 8.0;

/// Drawn traffic light: diameter, gap, edge padding.
const MAC_DOT: f32 = 12.0;
const MAC_GAP: f32 = 8.0;
const MAC_EDGE: f32 = 12.0;
const MAC_CLOSE: Color = Color::from_rgb(1.0, 0.373, 0.341);
const MAC_MINIMIZE: Color = Color::from_rgb(0.996, 0.737, 0.18);
const MAC_MAXIMIZE: Color = Color::from_rgb(0.157, 0.784, 0.251);

/// Room the OS-drawn macOS traffic lights take at the strip's left edge.
const MAC_NATIVE_INSET: f32 = 78.0;

/// Colours the button builders need, already resolved from the theme.
#[derive(Clone, Copy)]
pub(super) struct Palette {
    pub theme_id: ThemeId,
    pub text: Color,
    pub hover: Color,
    pub close_hover: Color,
}

/// Width of a group of `count` buttons in `style`, edge padding included.
fn group_width(style: WindowControlsStyle, count: usize) -> f32 {
    let n = count as f32;
    if count == 0 {
        return 0.0;
    }
    match style {
        WindowControlsStyle::MacNative => 0.0,
        WindowControlsStyle::Windows => n * WINDOWS_BTN_W,
        WindowControlsStyle::Linux => n * LINUX_BTN + (n - 1.0) * LINUX_GAP + 2.0 * LINUX_EDGE,
        WindowControlsStyle::MacDrawn => n * MAC_DOT + (n - 1.0) * MAC_GAP + 2.0 * MAC_EDGE,
    }
}

/// `(leading, trailing)` widths the strip reserves for window controls. The
/// native macOS lights are not ours to draw but still need the room, except
/// in fullscreen where the OS hides them.
pub(crate) fn extents(layout: &WindowControlsLayout, fullscreen: bool) -> (f32, f32) {
    let leading = match layout.style {
        WindowControlsStyle::MacNative if fullscreen => 0.0,
        WindowControlsStyle::MacNative => MAC_NATIVE_INSET,
        style => group_width(style, layout.leading.len()),
    };
    (leading, group_width(layout.style, layout.trailing.len()))
}

/// What sits at the strip's left edge: reserved room for the native lights,
/// or the leading buttons.
pub(super) fn leading(
    layout: &WindowControlsLayout,
    pal: Palette,
    fullscreen: bool,
) -> Element<'static, Message> {
    if layout.is_native() {
        let (inset, _) = extents(layout, fullscreen);
        return Space::new().width(inset).into();
    }
    group(&layout.leading, layout.style, pal)
}

/// What sits at the strip's right edge.
pub(super) fn trailing(layout: &WindowControlsLayout, pal: Palette) -> Element<'static, Message> {
    group(&layout.trailing, layout.style, pal)
}

fn group(
    buttons: &[WindowButton],
    style: WindowControlsStyle,
    pal: Palette,
) -> Element<'static, Message> {
    let build = |kind: &WindowButton| -> Element<'static, Message> {
        match style {
            WindowControlsStyle::MacNative => Space::new().into(),
            WindowControlsStyle::Windows => windows_button(*kind, pal),
            WindowControlsStyle::Linux => linux_button(*kind, pal),
            WindowControlsStyle::MacDrawn => mac_drawn_button(*kind),
        }
    };
    let (spacing, edge) = match style {
        WindowControlsStyle::Linux => (LINUX_GAP, LINUX_EDGE),
        WindowControlsStyle::MacDrawn => (MAC_GAP, MAC_EDGE),
        WindowControlsStyle::Windows | WindowControlsStyle::MacNative => (0.0, 0.0),
    };
    container(
        row(buttons.iter().map(build))
            .spacing(spacing)
            .padding(iced::Padding {
                top: 0.0,
                right: edge,
                bottom: 0.0,
                left: edge,
            })
            .align_y(Alignment::Center),
    )
    .height(MENU_BAR_HEIGHT)
    .align_y(iced::alignment::Vertical::Center)
    .into()
}

/// Close goes through the unsaved-changes guard like every other close route.
fn press_message(kind: WindowButton) -> Message {
    Message::Window(match kind {
        WindowButton::Close => WindowMsg::CloseMainWindow,
        WindowButton::Minimize => WindowMsg::MinimizeMainWindow,
        WindowButton::Maximize => WindowMsg::ToggleMaximizeMainWindow,
    })
}

fn glyph(kind: WindowButton, theme_id: ThemeId) -> svg::Handle {
    match kind {
        WindowButton::Close => crate::icons::icon_chrome_window_close(theme_id),
        WindowButton::Minimize => crate::icons::icon_chrome_window_min(theme_id),
        WindowButton::Maximize => crate::icons::icon_chrome_window_max(theme_id),
    }
}

fn icon(kind: WindowButton, pal: Palette, size: f32) -> Element<'static, Message> {
    let text = pal.text;
    svg(glyph(kind, pal.theme_id))
        .width(size)
        .height(size)
        .style(move |_: &iced::Theme, _| svg::Style { color: Some(text) })
        .into()
}

fn centered(content: Element<'static, Message>) -> Element<'static, Message> {
    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center)
        .into()
}

fn is_hot(status: button::Status) -> bool {
    matches!(status, button::Status::Hovered | button::Status::Pressed)
}

/// Rectangular 46 px button; close turns destructive red on hover.
fn windows_button(kind: WindowButton, pal: Palette) -> Element<'static, Message> {
    let is_close = kind == WindowButton::Close;
    let (hover_bg, hover_icon) = if is_close {
        (pal.close_hover, Color::WHITE)
    } else {
        (pal.hover, pal.text)
    };
    let text = pal.text;
    button(centered(icon(kind, pal, WINDOWS_ICON)))
        .width(WINDOWS_BTN_W)
        .height(MENU_BAR_HEIGHT)
        .padding(0)
        .on_press(press_message(kind))
        .style(move |_: &iced::Theme, status| {
            let hot = is_hot(status);
            button::Style {
                background: hot.then_some(Background::Color(hover_bg)),
                text_color: if hot { hover_icon } else { text },
                border: Border::default(),
                ..Default::default()
            }
        })
        .into()
}

/// Round button with a faint resting fill, as in libadwaita headerbars.
fn linux_button(kind: WindowButton, pal: Palette) -> Element<'static, Message> {
    let text = pal.text;
    let resting = Color { a: 0.10, ..text };
    let hovered = Color { a: 0.18, ..text };
    button(centered(icon(kind, pal, LINUX_ICON)))
        .width(LINUX_BTN)
        .height(LINUX_BTN)
        .padding(0)
        .on_press(press_message(kind))
        .style(move |_: &iced::Theme, status| button::Style {
            background: Some(Background::Color(if is_hot(status) {
                hovered
            } else {
                resting
            })),
            text_color: text,
            border: Border {
                radius: (LINUX_BTN / 2.0).into(),
                ..Border::default()
            },
            ..Default::default()
        })
        .into()
}

/// Traffic-light circle (used when macOS controls are moved to the right).
fn mac_drawn_button(kind: WindowButton) -> Element<'static, Message> {
    let fill = match kind {
        WindowButton::Close => MAC_CLOSE,
        WindowButton::Minimize => MAC_MINIMIZE,
        WindowButton::Maximize => MAC_MAXIMIZE,
    };
    button(Space::new())
        .width(MAC_DOT)
        .height(MAC_DOT)
        .padding(0)
        .on_press(press_message(kind))
        .style(move |_: &iced::Theme, status| {
            let shade = if is_hot(status) { 0.85 } else { 1.0 };
            button::Style {
                background: Some(Background::Color(Color {
                    r: fill.r * shade,
                    g: fill.g * shade,
                    b: fill.b * shade,
                    a: 1.0,
                })),
                border: Border {
                    radius: (MAC_DOT / 2.0).into(),
                    ..Border::default()
                },
                ..Default::default()
            }
        })
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chrome::controls::{ChromePlatform, WindowControlsPlacement, resolve};

    fn extents_of(platform: ChromePlatform, placement: WindowControlsPlacement) -> (f32, f32) {
        extents(&resolve(platform, placement, None), false)
    }

    #[test]
    fn windows_trailing_keeps_the_original_138_px_strip() {
        let (lead, trail) = extents_of(ChromePlatform::Windows, WindowControlsPlacement::Auto);
        assert_eq!((lead, trail), (0.0, 138.0));
    }

    #[test]
    fn leading_placement_moves_the_width_to_the_left() {
        let (lead, trail) = extents_of(ChromePlatform::Windows, WindowControlsPlacement::Leading);
        assert_eq!((lead, trail), (138.0, 0.0));
    }

    #[test]
    fn native_macos_lights_reserve_room_until_fullscreen() {
        let layout = resolve(ChromePlatform::MacOs, WindowControlsPlacement::Auto, None);
        assert_eq!(extents(&layout, false), (MAC_NATIVE_INSET, 0.0));
        assert_eq!(extents(&layout, true), (0.0, 0.0));
    }

    #[test]
    fn an_empty_side_reserves_nothing() {
        for style in [WindowControlsStyle::Linux, WindowControlsStyle::MacDrawn] {
            assert_eq!(group_width(style, 0), 0.0);
        }
    }

    #[test]
    fn round_button_groups_include_gaps_and_edge_padding() {
        assert_eq!(group_width(WindowControlsStyle::Linux, 1), 24.0 + 16.0);
        assert_eq!(
            group_width(WindowControlsStyle::MacDrawn, 3),
            3.0 * 12.0 + 2.0 * 8.0 + 24.0
        );
    }
}
