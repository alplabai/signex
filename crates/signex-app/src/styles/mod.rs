//! Custom Iced styles matching Altium Designer's dark theme chrome.
//!
//! All style functions are token-aware factories that accept `&ThemeTokens`
//! and return closures. This ensures every UI component picks up theme
//! changes in real time.

use iced::widget::container;
use iced::{Background, Border, Color, Theme};
use signex_types::theme::ThemeTokens;
use signex_widgets::button;

pub mod board_palette;
mod buttons;
mod dashed_outline;
mod dialog_buttons;
mod dialog_kit;
mod dialog_parts;
mod dialog_search;
mod dialog_shell;
pub mod disabled;
mod graphite;
mod graphite_buttons;
pub mod metrics;
pub mod radii;
pub mod scrollbar;
pub mod tokens;
pub mod typography;

pub use dashed_outline::dashed_frame;
pub use disabled::{DISABLED_OPACITY, dimmed, dimmed_button, read_only_field, read_only_row};
pub use graphite::{
    PillTone, card, field, floating_shadow, key_cap, key_cap_of, pill, pill_colors,
    resize_handle_edge, section_label,
};
pub use graphite_buttons::{ghost_button, primary_button};
pub use metrics::ChromeMetrics;
pub use radii::Radii;
pub use scrollbar::{scrollbar_colors, scrollbar_style, thin_both, thin_scrollbar, thin_vertical};

pub use dialog_kit::{dense_band_footer_height, dialog_band_padding};

pub use buttons::{
    accent_button, danger_button, hover_row, outline_button, subtle_button, subtle_button_with_rest,
};
pub use dialog_kit::{
    DialogButtonKind, dense_footer_height, dialog_body, dialog_button, dialog_footer,
    dialog_footer_band, dialog_footer_height, dialog_footer_split, dialog_hint, dialog_icon_button,
    dialog_list, dialog_message, dialog_section,
};
pub(crate) use dialog_parts::medium_font;
pub use dialog_parts::{
    CalloutTone, dialog_callout, dialog_caption, dialog_field_label, dialog_head_row,
    dialog_inline_error, dialog_row, dialog_rows, dialog_section_label, dirty_dot, file_kind_icon,
    violet_pair,
};
pub use dialog_search::dialog_search_field;
pub use dialog_shell::{
    DIALOG_ICON_TILE, DIALOG_INTRO_EXTRA, DialogIcon, dialog_glyph, dialog_icon_tile, dialog_intro,
    dialog_intro_with, tinted_glyph,
};
pub use typography::{HAIRLINE, LINE_HEIGHT, Spacing, TypeScale, chrome_px};

/// Corner radius, in px, of buttons, badges and cards.
pub const RADIUS: f32 = 3.0;

#[cfg(test)]
mod graphite_tests;

#[cfg(test)]
mod selected_segment_tests;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod typography_tests;

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
            ..Border::default()
        },
        ..container::Style::default()
    }
}

/// 1 px horizontal divider that sits between two strips of chrome —
/// used between the menu/Active-Bar row and the document tab strip
/// so the two zones read as separate UI bands.
pub fn chrome_separator(tokens: &ThemeTokens) -> impl Fn(&Theme) -> container::Style + 'static {
    let border = ti(tokens.border);
    move |_| container::Style {
        background: Some(Background::Color(border)),
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
            ..Border::default()
        },
        ..container::Style::default()
    }
}

/// Modal header strip — same toolbar bg as `toolbar_strip` but with a
/// top-only rounded radius matching `MODAL_CORNER_RADIUS`. Without
/// this, the modal's outer 8 px rounded border is visually masked by
/// the header's own rectangular background filling into the corners
/// (iced's `Container::clip(true)` clips to the bounds rectangle, not
/// the rounded path).
pub fn modal_header_strip(tokens: &ThemeTokens) -> impl Fn(&Theme) -> container::Style + 'static {
    let bg = ti(tokens.toolbar_bg);
    let text = ti(tokens.text);
    let border = ti(tokens.border);
    move |theme| container::Style {
        background: Some(bg.into()),
        text_color: Some(text),
        border: Border {
            width: 0.0,
            radius: if is_dialog_window(theme) {
                iced::border::Radius::default()
            } else {
                iced::border::Radius::default()
                    .top_left(MODAL_CORNER_RADIUS)
                    .top_right(MODAL_CORNER_RADIUS)
            },
            color: border,
            ..Border::default()
        },
        ..container::Style::default()
    }
}

/// Modal footer strip — mirrors `modal_header_strip` for the bottom
/// of a modal: same toolbar bg, BL+BR rounded radius matching
/// `MODAL_CORNER_RADIUS`. Apply to the very bottom container of a
/// modal that's wider than the body padding (button rows, status
/// rows) so the rectangular bg doesn't paint into the modal's
/// rounded bottom corners.
pub fn modal_footer_strip(tokens: &ThemeTokens) -> impl Fn(&Theme) -> container::Style + 'static {
    let bg = ti(tokens.toolbar_bg);
    let text = ti(tokens.text);
    let border = ti(tokens.border);
    move |theme| container::Style {
        background: Some(bg.into()),
        text_color: Some(text),
        border: Border {
            width: 0.0,
            radius: if is_dialog_window(theme) {
                iced::border::Radius::default()
            } else {
                iced::border::Radius::default()
                    .bottom_left(MODAL_CORNER_RADIUS)
                    .bottom_right(MODAL_CORNER_RADIUS)
            },
            color: border,
            ..Border::default()
        },
        ..container::Style::default()
    }
}

/// Active Bar strip (centered toolbar above canvas)
pub fn active_bar_strip(tokens: &ThemeTokens) -> impl Fn(&Theme) -> container::Style + 'static {
    let bg = ti(tokens.statusbar_bg);
    let shadow = floating_shadow(tokens);
    move |_| container::Style {
        background: Some(bg.into()),
        shadow,
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
        // A 1 px line along the TOP edge only (separating the bar from
        // the content above); the other three sides have no border.
        border: Border {
            width: 0.0,
            radius: 0.0.into(),
            color: border,
            top: iced::border::Side::default().width(1.0),
            ..Border::default()
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
            ..Border::default()
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
    let shadow = floating_shadow(tokens);
    move |_| container::Style {
        background: Some(bg.into()),
        text_color: Some(text),
        border: Border {
            width: 1.0,
            radius: 4.0.into(),
            color: border,
            ..Border::default()
        },
        shadow,
        ..container::Style::default()
    }
}

/// Corner radius shared by every modal card — sized to match the
/// Windows 11 OS-window rounding so the modal chrome reads as a
/// continuation of the app shell, not a separate panel.
pub const MODAL_CORNER_RADIUS: f32 = 8.0;

/// Modal-card surface — same panel/text/border palette as
/// `context_menu`, but with a wider corner radius matching the
/// OS chrome. Used by every modal so the rounding stays in step
/// with the surrounding window shell.
/// Marks the theme a dialog's own OS window is drawn with, so the dialog
/// surface there is flat (the window already is the frame): see
/// [`modal_card`]. The mark rides in the theme name; nothing else changes.
const DIALOG_WINDOW_MARK: &str = "\u{200B}dialog-window";

/// `base` as the theme of a dialog window. The window's own background is the
/// dialog surface (`paper`) of the active theme, so no strip of another colour
/// can show around the content.
pub fn dialog_window_theme(base: &Theme, paper: Color) -> Theme {
    let mut seed = base.seed();
    seed.background = paper;
    Theme::custom(format!("{base}{DIALOG_WINDOW_MARK}"), seed)
}

/// Whether `theme` is a dialog window's theme.
pub fn is_dialog_window(theme: &Theme) -> bool {
    theme.to_string().ends_with(DIALOG_WINDOW_MARK)
}

/// A dialog's surface: the rounded, bordered, shadowed card in the main
/// window, and a flat fill of the same colour inside its own OS window (no
/// second frame, no inset).
pub fn modal_card(tokens: &ThemeTokens) -> impl Fn(&Theme) -> container::Style + 'static {
    let bg = ti(tokens.paper);
    let text = ti(tokens.text);
    let border = ti(tokens.border);
    let radius = Radii::GRAPHITE.dialog;
    let shadow = floating_shadow(tokens);
    move |theme| {
        if is_dialog_window(theme) {
            return container::Style {
                background: Some(bg.into()),
                text_color: Some(text),
                ..container::Style::default()
            };
        }
        card_style(bg, text, border, shadow, radius)
    }
}

fn card_style(
    bg: Color,
    text: Color,
    border: Color,
    shadow: iced::Shadow,
    radius: f32,
) -> container::Style {
    container::Style {
        background: Some(bg.into()),
        text_color: Some(text),
        border: Border {
            width: 1.0,
            radius: radius.into(),
            color: border,
            ..Border::default()
        },
        shadow,
        ..container::Style::default()
    }
}

/// The dock region a dragged floating panel would dock into if released now
/// (mockup row 8): `accent-soft` fill, a 2 px accent border, card corners.
pub fn dock_zone_highlight(tokens: &ThemeTokens) -> impl Fn(&Theme) -> container::Style + 'static {
    let accent = ti(tokens.accent);
    let soft = tokens::resolve(tokens).accent_soft;
    move |_| container::Style {
        background: Some(Background::Color(soft)),
        border: Border {
            width: 2.0,
            radius: Radii::GRAPHITE.card.into(),
            color: accent,
            ..Border::default()
        },
        ..container::Style::default()
    }
}

/// A dashed outline drawn on a canvas: iced's `Border` has no dash.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DashedOutline {
    pub color: Color,
    pub width: f32,
    pub radius: f32,
    /// Dash and gap lengths, in px.
    pub dash: [f32; 2],
}

/// The faint outline of a dock region a dragged floating panel could dock
/// into but would not now (mockup row 8): 1.5 px dashed `line-strong`, card corners.
pub fn dock_zone_outline(tokens: &ThemeTokens) -> DashedOutline {
    DashedOutline {
        color: tokens::resolve(tokens).line_strong,
        width: 1.5,
        radius: Radii::GRAPHITE.card,
        dash: [5.0, 4.0],
    }
}

// ─── Button styles ────────────────────────────────────────────

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
            ..Border::default()
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
                ..Border::default()
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

/// Colours of every board view (PCB canvas, footprint editor, footprint
/// previews): the Graphite Dark canvas in both Graphite themes. The one
/// derivation of the board palette; only the chrome around a board view
/// follows the light/dark choice.
pub fn board_canvas_colors() -> signex_types::theme::CanvasColors {
    signex_types::theme::canvas_colors(signex_types::theme::ThemeId::GraphiteDark)
}
