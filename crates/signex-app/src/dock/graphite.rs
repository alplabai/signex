//! Graphite dock chrome: title bar, tab strip, icon buttons, rail and the
//! floating card. Only called when `styles::is_graphite(tokens)`; the legacy
//! look stays in `view.rs`.

use iced::widget::{button, container};
use iced::{Background, Border, Color, Font, Shadow, Theme, border, font};
use signex_types::theme::ThemeTokens;

use super::types::PanelPosition;
use crate::styles::tokens::{hover_overlay, resolve};
use crate::styles::{ChromeMetrics, Radii, chrome_px, floating_shadow, ti};

/// Active panel name in the title bar: 13 px at the default base.
pub(super) const TITLE_SIZE: f32 = chrome_px(13.0 * 11.0 / 12.0);
/// Side of a square title-bar icon button.
pub(super) const ICON_BUTTON: f32 = chrome_px(24.0 * 11.0 / 12.0);
/// Height of the title bar and of the tab strip.
pub(super) const TITLE_H: f32 = ChromeMetrics::DEFAULT.dock_title;
pub(super) const TAB_STRIP_H: f32 = ChromeMetrics::DEFAULT.dock_tab_strip;
/// Collapsed rail width.
pub(super) const RAIL_W: f32 = ChromeMetrics::DEFAULT.rail;
/// Width of the accent line on the inner edge of the active rail tab.
const RAIL_ACCENT: f32 = 2.0;

/// Weight 500 of the default UI family.
pub(super) fn title_font() -> Font {
    Font {
        weight: font::Weight::Medium,
        ..Font::DEFAULT
    }
}

fn side(color: Color, width: f32) -> border::Side {
    border::Side::default().color(color).width(width)
}

/// Title bar: `chrome` fill, 1 px line on the bottom edge only.
pub(super) fn title_bar(tokens: &ThemeTokens) -> impl Fn(&Theme) -> container::Style + 'static {
    let (bg, ink, line) = (ti(tokens.toolbar_bg), ti(tokens.text), ti(tokens.border));
    move |_| container::Style {
        background: Some(Background::Color(bg)),
        text_color: Some(ink),
        border: Border {
            bottom: side(line, 1.0),
            ..Border::default()
        },
        ..container::Style::default()
    }
}

/// Tab strip: `panel` fill, 1 px line on the top edge only.
pub(super) fn tab_strip(tokens: &ThemeTokens) -> impl Fn(&Theme) -> container::Style + 'static {
    let (bg, ink, line) = (ti(tokens.panel_bg), ti(tokens.text), ti(tokens.border));
    move |_| container::Style {
        background: Some(Background::Color(bg)),
        text_color: Some(ink),
        border: Border {
            top: side(line, 1.0),
            ..Border::default()
        },
        ..container::Style::default()
    }
}

/// 24x24 transparent icon button, radius 6, hover overlay.
pub(super) fn icon_button(
    tokens: &ThemeTokens,
) -> impl Fn(&Theme, button::Status) -> button::Style + 'static {
    let hover = hover_overlay(tokens);
    let ink = ti(tokens.text_secondary);
    move |_, status| button::Style {
        background: matches!(status, button::Status::Hovered | button::Status::Pressed)
            .then_some(Background::Color(hover)),
        text_color: ink,
        border: Border {
            radius: Radii::GRAPHITE.small.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

/// Collapsed rail strip: `panel` fill and a hairline on the inner edge.
pub(super) fn rail(
    tokens: &ThemeTokens,
    position: PanelPosition,
) -> impl Fn(&Theme) -> container::Style + 'static {
    let (bg, line) = (ti(tokens.panel_bg), ti(tokens.border));
    move |_| {
        let mut border = Border::default();
        match position {
            PanelPosition::Left => border.right = side(line, 1.0),
            PanelPosition::Right => border.left = side(line, 1.0),
            PanelPosition::Bottom => border.top = side(line, 1.0),
        }
        container::Style {
            background: Some(Background::Color(bg)),
            border,
            ..container::Style::default()
        }
    }
}

/// Rail tab: the active one shows the accent on the edge facing the canvas.
pub(super) fn rail_tab(
    tokens: &ThemeTokens,
    position: PanelPosition,
    is_active: bool,
) -> impl Fn(&Theme, button::Status) -> button::Style + 'static {
    let hover = hover_overlay(tokens);
    let accent = ti(tokens.accent);
    let soft = resolve(tokens).accent_soft;
    move |_, status| {
        let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
        let background = if is_active {
            Some(Background::Color(soft))
        } else if hovered {
            Some(Background::Color(hover))
        } else {
            None
        };
        let mut border = Border::default();
        if is_active {
            match position {
                PanelPosition::Left => border.right = side(accent, RAIL_ACCENT),
                PanelPosition::Right => border.left = side(accent, RAIL_ACCENT),
                PanelPosition::Bottom => border.top = side(accent, RAIL_ACCENT),
            }
        }
        button::Style {
            background,
            border,
            ..button::Style::default()
        }
    }
}

/// Floating panel: one card (radius 10, hairline, themed shadow).
pub(super) fn floating_card(tokens: &ThemeTokens) -> impl Fn(&Theme) -> container::Style + 'static {
    let (bg, ink, line) = (ti(tokens.panel_bg), ti(tokens.text), ti(tokens.border));
    let shadow: Shadow = floating_shadow(tokens, Shadow::default());
    move |_| container::Style {
        background: Some(Background::Color(bg)),
        text_color: Some(ink),
        border: Border {
            width: 1.0,
            color: line,
            radius: Radii::GRAPHITE.card.into(),
            ..Border::default()
        },
        shadow,
        ..container::Style::default()
    }
}

/// Side padding of a dock tab label; a strip that is too wide scrolls, the label
/// size never shrinks.
pub(super) const TAB_PAD: f32 = 10.0;
