//! Custom Iced styles matching Altium Designer's dark theme chrome.
//!
//! All style functions are token-aware factories that accept `&ThemeTokens`
//! and return closures. This ensures every UI component picks up theme
//! changes in real time.

use iced::widget::{button, container};
use iced::{Background, Border, Color, Theme};
use signex_types::theme::ThemeTokens;

// ─── Color conversion ─────────────────────────────────────────

/// Convert a signex-types Color to an iced Color.
#[inline]
pub fn ti(c: signex_types::theme::Color) -> Color {
    Color::from_rgba8(c.r, c.g, c.b, c.a as f32 / 255.0)
}

// ─── Container styles ─────────────────────────────────────────

/// Panel region (left/right/bottom docks)
pub fn panel_region(tokens: &ThemeTokens) -> impl Fn(&Theme) -> container::Style + 'static {
    let bg = ti(tokens.panel_bg);
    let text = ti(tokens.text);
    let border = ti(tokens.border);
    move |_| container::Style {
        background: Some(bg.into()),
        text_color: Some(text),
        border: Border {
            width: 1.0,
            radius: 0.0.into(),
            color: border,
        },
        ..container::Style::default()
    }
}

/// Toolbar / menu bar strip
pub fn toolbar_strip(tokens: &ThemeTokens) -> impl Fn(&Theme) -> container::Style + 'static {
    let bg = ti(tokens.toolbar_bg);
    let text = ti(tokens.text);
    let border = ti(tokens.border);
    move |_| container::Style {
        background: Some(bg.into()),
        text_color: Some(text),
        border: Border {
            width: 0.0,
            radius: 0.0.into(),
            color: border,
        },
        ..container::Style::default()
    }
}

/// Active Bar strip (centered toolbar above canvas)
#[allow(dead_code)]
pub fn active_bar_strip(tokens: &ThemeTokens) -> impl Fn(&Theme) -> container::Style + 'static {
    let bg = ti(tokens.statusbar_bg);
    move |_| container::Style {
        background: Some(bg.into()),
        ..container::Style::default()
    }
}

/// Status bar at the bottom
pub fn status_bar(tokens: &ThemeTokens) -> impl Fn(&Theme) -> container::Style + 'static {
    let bg = ti(tokens.statusbar_bg);
    let text = ti(tokens.text);
    let border = ti(tokens.border);
    move |_| container::Style {
        background: Some(bg.into()),
        text_color: Some(text),
        border: Border {
            width: 1.0,
            radius: 0.0.into(),
            color: border,
        },
        ..container::Style::default()
    }
}

/// Tab bar background (dock region header)
pub fn tab_bar_strip(tokens: &ThemeTokens) -> impl Fn(&Theme) -> container::Style + 'static {
    let bg = ti(tokens.toolbar_bg);
    let text = ti(tokens.text);
    let border = ti(tokens.border);
    move |_| container::Style {
        background: Some(bg.into()),
        text_color: Some(text),
        border: Border {
            width: 1.0,
            radius: 0.0.into(),
            color: border,
        },
        ..container::Style::default()
    }
}

/// Collapsed rail (vertical/horizontal panel strip)
pub fn collapsed_rail(tokens: &ThemeTokens) -> impl Fn(&Theme) -> container::Style + 'static {
    let bg = ti(tokens.panel_bg);
    let border = ti(tokens.border);
    move |_| container::Style {
        background: Some(bg.into()),
        border: Border {
            width: 1.0,
            radius: 0.0.into(),
            color: border,
        },
        ..container::Style::default()
    }
}

/// Resize handle between panels (thin draggable border)
pub fn resize_handle(tokens: &ThemeTokens) -> impl Fn(&Theme) -> container::Style + 'static {
    let bg = ti(tokens.border);
    move |_| container::Style {
        background: Some(bg.into()),
        ..container::Style::default()
    }
}

/// Panel content area (inside the dock)
#[allow(dead_code)]
pub fn panel_content(tokens: &ThemeTokens) -> impl Fn(&Theme) -> container::Style + 'static {
    let bg = ti(tokens.panel_bg);
    let text = ti(tokens.text);
    move |_| container::Style {
        background: Some(bg.into()),
        text_color: Some(text),
        border: Border::default(),
        ..container::Style::default()
    }
}

/// Context menu / popup container (right-click menu, panel list)
pub fn context_menu(tokens: &ThemeTokens) -> impl Fn(&Theme) -> container::Style + 'static {
    let bg = ti(tokens.paper);
    let text = ti(tokens.text);
    let border = ti(tokens.border);
    move |_| container::Style {
        background: Some(bg.into()),
        text_color: Some(text),
        border: Border {
            width: 1.0,
            radius: 4.0.into(),
            color: border,
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
pub fn floating_title_bar(tokens: &ThemeTokens) -> impl Fn(&Theme) -> container::Style + 'static {
    let bg = ti(tokens.paper);
    let border = ti(tokens.border);
    move |_| container::Style {
        background: Some(bg.into()),
        border: Border {
            width: 1.0,
            radius: 6.0.into(),
            color: border,
        },
        ..container::Style::default()
    }
}

/// Floating panel body
pub fn floating_panel_body(tokens: &ThemeTokens) -> impl Fn(&Theme) -> container::Style + 'static {
    let bg = ti(tokens.panel_bg);
    let border = ti(tokens.border);
    move |_| container::Style {
        background: Some(bg.into()),
        border: Border {
            width: 1.0,
            radius: 6.0.into(),
            color: border,
        },
        ..container::Style::default()
    }
}

/// Floating panel outer wrapper (shadow only)
pub fn floating_panel_shadow(
    _tokens: &ThemeTokens,
) -> impl Fn(&Theme) -> container::Style + 'static {
    move |_| container::Style {
        shadow: iced::Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.5),
            offset: iced::Vector::new(0.0, 4.0),
            blur_radius: 12.0,
        },
        ..container::Style::default()
    }
}

/// Translucent highlight overlay shown on dock regions when a floating panel
/// is dragged near a window edge.
pub fn dock_zone_highlight(tokens: &ThemeTokens) -> impl Fn(&Theme) -> container::Style + 'static {
    let accent = ti(tokens.accent);
    move |_| container::Style {
        background: Some(Background::Color(Color::from_rgba(
            accent.r, accent.g, accent.b, 0.15,
        ))),
        border: Border {
            width: 2.0,
            radius: 4.0.into(),
            color: Color::from_rgba(accent.r, accent.g, accent.b, 0.4),
        },
        ..container::Style::default()
    }
}

// ─── Button styles ────────────────────────────────────────────

/// Container-based dock tab (used with mouse_area for drag-to-undock).
///
/// Altium-style flat tabs: active tab gets a hover background + accent
/// underline (added by the caller via `tab_underline`). Inactive tabs are
/// plain text — no border box. Iced 0.14 `Border` is uniform on all four
/// sides, so any border here would draw the unwanted bottom edge too.
/// Drag-aware wrapper — flags the currently-dragged tab with a thick
/// accent border + tinted background so the user gets visual
/// feedback on the tab they grabbed.
pub fn dock_tab_container_dragging(
    tokens: &ThemeTokens,
    is_active: bool,
    is_dragging: bool,
) -> impl Fn(&Theme) -> container::Style + 'static {
    let tab_active = ti(tokens.hover);
    let border_c = ti(tokens.border);
    let accent = ti(tokens.accent);
    let inactive_fill = iced::Color {
        a: tab_active.a * 0.35,
        ..tab_active
    };
    let drag_fill = iced::Color { a: 0.22, ..accent };
    move |_: &Theme| container::Style {
        background: Some(Background::Color(if is_dragging {
            drag_fill
        } else if is_active {
            tab_active
        } else {
            inactive_fill
        })),
        border: Border {
            width: if is_dragging { 2.0 } else { 1.0 },
            radius: 0.0.into(),
            color: if is_dragging { accent } else { border_c },
        },
        ..container::Style::default()
    }
}

#[allow(dead_code)]
pub fn dock_tab_container(
    tokens: &ThemeTokens,
    is_active: bool,
) -> impl Fn(&Theme) -> container::Style + 'static {
    let tab_active = ti(tokens.hover);
    let border_c = ti(tokens.border);
    // Inactive tabs get a subtle fill derived from the hover color
    // (about half its alpha) so they still read as clickable tabs
    // rather than bare text on the header strip.
    let inactive_fill = iced::Color {
        a: tab_active.a * 0.35,
        ..tab_active
    };
    move |_: &Theme| container::Style {
        background: Some(Background::Color(if is_active {
            tab_active
        } else {
            inactive_fill
        })),
        // Thin border on all sides so adjacent tabs have a visible
        // divider between them. Iced Border is uniform 4-sided; the
        // vertical edges give the tab-strip its "cell" look, and the
        // horizontal edges blend into the surrounding strip padding.
        border: Border {
            width: 1.0,
            radius: 0.0.into(),
            color: border_c,
        },
        ..container::Style::default()
    }
}

/// Rail tab button (collapsed dock) — rounded corners.
pub fn rail_tab(
    tokens: &ThemeTokens,
    is_active: bool,
) -> impl Fn(&Theme, button::Status) -> button::Style + 'static {
    let tab_active = ti(tokens.hover);
    let border = ti(tokens.border);
    move |_: &Theme, status: button::Status| {
        let bg = match (is_active, status) {
            (true, _) => Some(Background::Color(tab_active)),
            (false, button::Status::Hovered) => Some(Background::Color(tab_active)),
            _ => None,
        };
        button::Style {
            background: bg,
            border: Border {
                width: 1.0,
                radius: 3.0.into(),
                color: border,
            },
            ..button::Style::default()
        }
    }
}

/// Menu item / popup list button — full-width hover highlight.
pub fn menu_item(
    tokens: &ThemeTokens,
) -> impl Fn(&Theme, button::Status) -> button::Style + 'static {
    let hover = ti(tokens.hover);
    let text = ti(tokens.text);
    move |_: &Theme, status: button::Status| {
        let bg = match status {
            button::Status::Hovered => Some(Background::Color(hover)),
            _ => None,
        };
        button::Style {
            background: bg,
            border: Border::default(),
            text_color: text,
            ..button::Style::default()
        }
    }
}

/// Accent-colored underline container (used below active dock tabs).
pub fn tab_underline(color: Color) -> impl Fn(&Theme) -> container::Style + 'static {
    move |_: &Theme| container::Style {
        background: Some(Background::Color(color)),
        ..container::Style::default()
    }
}
