//! Custom Iced styles matching Altium Designer's dark theme chrome.
//!
//! All styling functions return closures compatible with Iced's style system.
//! Reusable helpers eliminate inline closure duplication across modules.

use iced::widget::{button, container};
use iced::{Background, Border, Color, Theme};

// ─── Colors (Altium-inspired dark chrome) ─────────────────────

/// Panel/dock background — slightly lighter than window bg
pub const PANEL_BG: Color = Color::from_rgb(0.10, 0.10, 0.11);
/// Toolbar/menu background
pub const TOOLBAR_BG: Color = Color::from_rgb(0.12, 0.12, 0.13);
/// Status bar background
pub const STATUSBAR_BG: Color = Color::from_rgb(0.09, 0.09, 0.10);
/// Active Bar background strip
pub const ACTIVE_BAR_BG: Color = Color::from_rgb(0.09, 0.09, 0.10);
/// Border between panels
pub const BORDER_COLOR: Color = Color::from_rgb(0.20, 0.20, 0.22);
/// Subtle border (less prominent)
pub const BORDER_SUBTLE: Color = Color::from_rgb(0.15, 0.15, 0.17);
/// Tab header active background
pub const TAB_ACTIVE_BG: Color = Color::from_rgb(0.18, 0.18, 0.20);
/// Primary text
pub const TEXT_PRIMARY: Color = Color::from_rgb(0.85, 0.85, 0.88);
/// Secondary/muted text
pub const TEXT_MUTED: Color = Color::from_rgb(0.55, 0.55, 0.60);
/// Active tab indicator (white, matching Altium's neutral style)
pub const ACCENT: Color = Color::from_rgb(0.85, 0.85, 0.88);
/// Floating panel title bar / popup background
pub const POPUP_BG: Color = Color::from_rgb(0.14, 0.14, 0.16);
/// Floating panel border
pub const POPUP_BORDER: Color = Color::from_rgb(0.24, 0.25, 0.33);
/// Hover highlight for menu items
pub const HOVER_BG: Color = Color::from_rgb(0.20, 0.22, 0.30);

// ─── Container styles ─────────────────────────────────────────

/// Panel region (left/right/bottom docks)
pub fn panel_region(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(PANEL_BG.into()),
        text_color: Some(TEXT_PRIMARY),
        border: Border {
            width: 1.0,
            radius: 0.0.into(),
            color: BORDER_COLOR,
        },
        ..container::Style::default()
    }
}

/// Toolbar / menu bar strip
pub fn toolbar_strip(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(TOOLBAR_BG.into()),
        text_color: Some(TEXT_PRIMARY),
        border: Border {
            width: 0.0,
            radius: 0.0.into(),
            color: BORDER_SUBTLE,
        },
        ..container::Style::default()
    }
}

/// Active Bar strip (centered toolbar above canvas)
pub fn active_bar_strip(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(ACTIVE_BAR_BG.into()),
        ..container::Style::default()
    }
}

/// Status bar at the bottom
pub fn status_bar(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(STATUSBAR_BG.into()),
        text_color: Some(TEXT_PRIMARY),
        border: Border {
            width: 1.0,
            radius: 0.0.into(),
            color: BORDER_SUBTLE,
        },
        ..container::Style::default()
    }
}

/// Tab bar background (dock region header)
pub fn tab_bar_strip(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(TOOLBAR_BG.into()),
        text_color: Some(TEXT_PRIMARY),
        border: Border {
            width: 1.0,
            radius: 0.0.into(),
            color: BORDER_SUBTLE,
        },
        ..container::Style::default()
    }
}

/// Collapsed rail (vertical/horizontal panel strip)
pub fn collapsed_rail(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(PANEL_BG.into()),
        border: Border {
            width: 1.0,
            radius: 0.0.into(),
            color: BORDER_SUBTLE,
        },
        ..container::Style::default()
    }
}

/// Resize handle between panels (thin draggable border)
pub fn resize_handle(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(BORDER_COLOR.into()),
        ..container::Style::default()
    }
}

/// Panel content area (inside the dock)
#[allow(dead_code)]
pub fn panel_content(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(PANEL_BG.into()),
        text_color: Some(TEXT_PRIMARY),
        border: Border::default(),
        ..container::Style::default()
    }
}

/// Context menu / popup container (right-click menu, panel list)
pub fn context_menu(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(POPUP_BG.into()),
        text_color: Some(TEXT_PRIMARY),
        border: Border {
            width: 1.0,
            radius: 4.0.into(),
            color: BORDER_COLOR,
        },
        shadow: iced::Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.4),
            offset: iced::Vector::new(2.0, 3.0),
            blur_radius: 8.0,
        },
        ..container::Style::default()
    }
}

/// Floating panel title bar
pub fn floating_title_bar(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(POPUP_BG.into()),
        border: Border {
            width: 1.0,
            radius: 6.0.into(),
            color: POPUP_BORDER,
        },
        ..container::Style::default()
    }
}

/// Floating panel body
pub fn floating_panel_body(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(PANEL_BG.into()),
        border: Border {
            width: 1.0,
            radius: 6.0.into(),
            color: POPUP_BORDER,
        },
        ..container::Style::default()
    }
}

/// Floating panel outer wrapper (shadow only)
pub fn floating_panel_shadow(_theme: &Theme) -> container::Style {
    container::Style {
        shadow: iced::Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.5),
            offset: iced::Vector::new(0.0, 4.0),
            blur_radius: 12.0,
        },
        ..container::Style::default()
    }
}

// ─── Button styles ────────────────────────────────────────────

/// Dock tab button — active state gets highlighted bg + border.
pub fn dock_tab(is_active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_: &Theme, status: button::Status| {
        let bg = match (is_active, status) {
            (true, _) => Some(Background::Color(TAB_ACTIVE_BG)),
            (false, button::Status::Hovered) => Some(Background::Color(TAB_ACTIVE_BG)),
            _ => None,
        };
        button::Style {
            background: bg,
            border: Border {
                width: 1.0,
                radius: 0.0.into(),
                color: BORDER_SUBTLE,
            },
            ..button::Style::default()
        }
    }
}

/// Rail tab button (collapsed dock) — rounded corners.
pub fn rail_tab(is_active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_: &Theme, status: button::Status| {
        let bg = match (is_active, status) {
            (true, _) => Some(Background::Color(TAB_ACTIVE_BG)),
            (false, button::Status::Hovered) => Some(Background::Color(TAB_ACTIVE_BG)),
            _ => None,
        };
        button::Style {
            background: bg,
            border: Border {
                width: 1.0,
                radius: 3.0.into(),
                color: BORDER_COLOR,
            },
            ..button::Style::default()
        }
    }
}

/// Menu item / popup list button — full-width hover highlight.
pub fn menu_item(_theme: &Theme, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered => Some(Background::Color(HOVER_BG)),
        _ => None,
    };
    button::Style {
        background: bg,
        border: Border::default(),
        text_color: TEXT_PRIMARY,
        ..button::Style::default()
    }
}

/// Accent-colored underline container (used below active dock tabs).
pub fn tab_underline(color: Color) -> impl Fn(&Theme) -> container::Style {
    move |_: &Theme| container::Style {
        background: Some(Background::Color(color)),
        ..container::Style::default()
    }
}

