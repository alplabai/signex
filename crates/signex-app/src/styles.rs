//! Custom Iced styles matching Altium Designer's dark theme chrome.
//!
//! All styling functions return closures compatible with Iced's style system.

use iced::widget::container;
use iced::{Border, Color, Theme};

// ─── Colors (Altium-inspired dark chrome) ─────────────────────

/// Panel/dock background — slightly lighter than window bg
pub const PANEL_BG: Color = Color::from_rgb(0.10, 0.10, 0.11);
/// Toolbar/menu background
pub const TOOLBAR_BG: Color = Color::from_rgb(0.12, 0.12, 0.13);
/// Status bar background
pub const STATUSBAR_BG: Color = Color::from_rgb(0.09, 0.09, 0.10);
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

/// Tab bar background
#[allow(dead_code)]
pub fn tab_bar(_theme: &Theme) -> container::Style {
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

/// Context menu popup (right-click menu)
pub fn context_menu(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb(0.14, 0.14, 0.16).into()),
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

