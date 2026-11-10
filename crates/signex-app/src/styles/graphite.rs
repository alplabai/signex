//! Graphite surface styles: shadow, pill, key cap, section label, card,
//! field and the 1 px splitter.

use iced::widget::{container, text};
use iced::{Background, Border, Color, Shadow, Theme, Vector};
use signex_types::theme::ThemeTokens;

use super::radii::Radii;
use super::ti;
use super::tokens::resolve;

/// Shadow of a floating surface: offset (0, 6), blur 20, the theme's
/// shadow token.
pub fn floating_shadow(tokens: &ThemeTokens) -> Shadow {
    Shadow {
        color: resolve(tokens).shadow,
        offset: Vector::new(0.0, 6.0),
        blur_radius: 20.0,
    }
}

/// Status tone of a [`pill`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PillTone {
    Neutral,
    Accent,
    Ok,
    Warn,
    Err,
    Info,
}

/// `(fill, ink)` of a pill tone.
pub fn pill_colors(tokens: &ThemeTokens, tone: PillTone) -> (Color, Color) {
    let r = resolve(tokens);
    match tone {
        PillTone::Neutral => (r.sunken, r.ink3),
        PillTone::Accent => (r.accent_soft, r.accent_soft_ink),
        PillTone::Ok => (r.ok_soft, ti(tokens.success)),
        PillTone::Warn => (r.warn_soft, ti(tokens.warning)),
        PillTone::Err => (r.err_soft, ti(tokens.error)),
        PillTone::Info => (r.info_soft, r.info),
    }
}

/// Fully rounded status chip: soft fill, toned ink.
pub fn pill(tokens: &ThemeTokens, tone: PillTone) -> impl Fn(&Theme) -> container::Style + 'static {
    let (bg, ink) = pill_colors(tokens, tone);
    move |_| container::Style {
        background: Some(Background::Color(bg)),
        text_color: Some(ink),
        border: Border {
            radius: Radii::GRAPHITE.pill.into(),
            ..Border::default()
        },
        ..container::Style::default()
    }
}

/// Keyboard key cap ("Ctrl K"): sunken fill, hairline border, small radius.
pub fn key_cap(tokens: &ThemeTokens) -> impl Fn(&Theme) -> container::Style + 'static {
    let r = resolve(tokens);
    let (bg, ink, line) = (r.sunken, r.ink3, r.line_soft);
    move |_| container::Style {
        background: Some(Background::Color(bg)),
        text_color: Some(ink),
        border: Border {
            width: 1.0,
            color: line,
            radius: 4.0.into(),
            ..Border::default()
        },
        ..container::Style::default()
    }
}

/// Style of an uppercase section label (plain, no tracking): muted ink.
/// Pair it with `text(label.to_uppercase()).size(TypeScale::DEFAULT.section)`.
pub fn section_label(tokens: &ThemeTokens) -> impl Fn(&Theme) -> text::Style + 'static {
    let ink = resolve(tokens).ink3;
    move |_| text::Style {
        color: Some(ink),
        ..Default::default()
    }
}

/// Card: raised fill, hairline border, card radius, no shadow.
pub fn card(tokens: &ThemeTokens) -> impl Fn(&Theme) -> container::Style + 'static {
    let (bg, ink) = (resolve(tokens).raised, ti(tokens.text));
    let line = ti(tokens.border);
    move |_| container::Style {
        background: Some(Background::Color(bg)),
        text_color: Some(ink),
        border: Border {
            width: 1.0,
            color: line,
            radius: Radii::GRAPHITE.card.into(),
            ..Border::default()
        },
        ..container::Style::default()
    }
}

/// Field frame: `field` fill, hairline border, control radius. `focused`
/// swaps the border for the accent.
pub fn field(tokens: &ThemeTokens, focused: bool) -> impl Fn(&Theme) -> container::Style + 'static {
    let r = resolve(tokens);
    let line = if focused {
        ti(tokens.accent)
    } else {
        ti(tokens.border)
    };
    let (bg, ink) = (r.field, ti(tokens.text));
    move |_| container::Style {
        background: Some(Background::Color(bg)),
        text_color: Some(ink),
        border: Border {
            width: 1.0,
            color: line,
            radius: Radii::GRAPHITE.control.into(),
            ..Border::default()
        },
        ..container::Style::default()
    }
}

/// Dock splitter hit area. Graphite: transparent with a 1 px line (the
/// `border` colour) on the leading edge, left for a vertical bar and top for
/// a horizontal one.
/// `horizontal` is the flag of `view_resize_handle`: a vertical bar that
/// resizes horizontally.
pub fn resize_handle_edge(
    tokens: &ThemeTokens,
    horizontal: bool,
) -> impl Fn(&Theme) -> container::Style + 'static {
    let line = ti(tokens.border);
    move |_| {
        let edge = iced::border::Side::default().width(1.0);
        let mut border = Border {
            color: line,
            ..Border::default()
        };
        if horizontal {
            border.left = edge;
        } else {
            border.top = edge;
        }
        container::Style {
            border,
            ..container::Style::default()
        }
    }
}
