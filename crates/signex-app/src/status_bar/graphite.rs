//! Graphite look of the status bar and its Panels popover. Legacy themes
//! never reach these helpers.

use iced::widget::{Text, Widget, button, container, row, space, text};
use iced::{Background, Border, Color, Element, Length, Padding, Shadow, Theme};
use signex_types::coord::Unit;
use signex_types::theme::ThemeTokens;

use super::Readout;
use crate::app::StatusBarRequest;
use crate::styles::tokens::resolve;
use crate::styles::{ChromeMetrics, Radii, Spacing, TypeScale, ti};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Width of the Panels popover.
pub const POPUP_WIDTH: f32 = 200.0;
/// Inner padding of the Panels popover.
pub const POPUP_PADDING: f32 = 4.0;
/// Gap between the popover's right edge and the window edge.
pub const POPUP_RIGHT_MARGIN: f32 = 6.0;
/// Gap between the popover's bottom edge and the status bar.
pub const POPUP_GAP: f32 = 4.0;
/// Radius of the small controls in the bar (Snap pill, Panels button).
const BAR_CONTROL_RADIUS: f32 = 5.0;

/// Padding of the bar: 12 px left, 4 px right, centred vertically.
pub const BAR_PADDING: Padding = Padding {
    top: 0.0,
    right: 4.0,
    bottom: 0.0,
    left: 12.0,
};

/// Height of a Panels popover row.
pub fn popup_row_height() -> f32 {
    ChromeMetrics::DEFAULT.field
}

/// Total height of a popover with `rows` rows.
pub fn popup_height(rows: usize) -> f32 {
    rows as f32 * popup_row_height() + 2.0 * POPUP_PADDING
}

/// The chrome mono face (coordinates, grid, snap).
pub fn mono_font() -> iced::Font {
    crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT)
}

/// Snap toggle: `ok_soft` fill and `ok` ink while snapping, flat and muted
/// while free.
pub fn snap_pill(
    tokens: &ThemeTokens,
    snapping: bool,
) -> impl Fn(&Theme, button::Status) -> button::Style + 'static {
    let r = resolve(tokens);
    let (fill, ink) = if snapping {
        (Some(Background::Color(r.ok_soft)), ti(tokens.success))
    } else {
        (None, r.ink3)
    };
    let hover = ti(tokens.hover);
    move |_, status| button::Style {
        background: match (status, fill) {
            (button::Status::Hovered, None) => Some(Background::Color(hover)),
            _ => fill,
        },
        text_color: ink,
        border: Border {
            radius: BAR_CONTROL_RADIUS.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

/// The bordered Panels button.
pub fn panels_button(
    tokens: &ThemeTokens,
) -> impl Fn(&Theme, button::Status) -> button::Style + 'static {
    let (raised, line) = (resolve(tokens).raised, ti(tokens.border));
    let (hover, ink) = (ti(tokens.hover), ti(tokens.text));
    move |_, status| button::Style {
        background: Some(Background::Color(match status {
            button::Status::Hovered | button::Status::Pressed => hover,
            _ => raised,
        })),
        text_color: ink,
        border: Border {
            width: 1.0,
            color: line,
            radius: BAR_CONTROL_RADIUS.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

/// The popover surface: raised fill, hairline, card radius, floating shadow.
pub fn popup(tokens: &ThemeTokens) -> impl Fn(&Theme) -> container::Style + 'static {
    let (raised, line, ink) = (resolve(tokens).raised, ti(tokens.border), ti(tokens.text));
    let shadow = crate::styles::floating_shadow(tokens, Shadow::default());
    move |_| container::Style {
        background: Some(Background::Color(raised)),
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

/// A popover row: flat, with a `hover` fill and the small radius.
pub fn popup_row(
    tokens: &ThemeTokens,
) -> impl Fn(&Theme, button::Status) -> button::Style + 'static {
    let (hover, ink) = (ti(tokens.hover), ti(tokens.text));
    move |_, status| button::Style {
        background: match status {
            button::Status::Hovered | button::Status::Pressed => Some(Background::Color(hover)),
            _ => None,
        },
        text_color: ink,
        border: Border {
            radius: Radii::GRAPHITE.small.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

/// Ink of the "open" check mark in a popover row.
pub fn check_ink(tokens: &ThemeTokens) -> Color {
    resolve(tokens).accent_text
}

/// The cursor readout: `(x, y)` as numbers in `unit`.
pub(super) fn coord_values(unit: Unit, x: f64, y: f64) -> (String, String) {
    match unit {
        Unit::Mm => (format!("{x:.2}"), format!("{y:.2}")),
        Unit::Mil => (format!("{:.1}", x / 0.0254), format!("{:.1}", y / 0.0254)),
        Unit::Inch => (format!("{:.4}", x / 25.4), format!("{:.4}", y / 25.4)),
        Unit::Micrometer => (format!("{:.0}", x * 1000.0), format!("{:.0}", y * 1000.0)),
    }
}

fn mono_text(s: String, color: Color) -> Text<'static> {
    text(s).size(TS.body).color(color).font(mono_font())
}

fn mono_button(
    s: String,
    color: Color,
    request: StatusBarRequest,
) -> button::Button<'static, StatusBarRequest, Text<'static>> {
    button(mono_text(s, color))
        .padding([1.0, SP.s])
        .style(button::text)
        .on_press(request)
}

/// The Graphite status bar: mono X / Y / Grid spans, a Snap pill, the
/// tool and selection context, and the bordered Panels button on the right.
/// No separators; the spans are spaced apart.
pub(super) fn view<'a>(r: &Readout<'_>, tokens: &ThemeTokens) -> Element<'a, StatusBarRequest> {
    let ink = ti(tokens.text);
    let muted = resolve(tokens).ink3;
    let unit = r.unit;
    let (xs, ys) = coord_values(unit, r.x, r.y);
    let grid = if r.grid_visible {
        format!("{:.3} {unit}", r.grid_size_mm)
    } else {
        "OFF".to_string()
    };

    let mut bar = row![
        mono_text(format!("X {xs} {unit}"), muted).boxed(),
        mono_text(format!("Y {ys} {unit}"), muted).boxed(),
        mono_button(format!("Grid {grid}"), muted, StatusBarRequest::ToggleGrid).boxed(),
        button(mono_text(
            if r.snap_enabled { "Snap" } else { "Free" }.to_string(),
            if r.snap_enabled {
                ti(tokens.success)
            } else {
                muted
            },
        ))
        .padding([1.5, 6.0])
        .style(snap_pill(tokens, r.snap_enabled))
        .on_press(StatusBarRequest::ToggleSnap)
        .boxed(),
        mono_text("E-Snap".to_string(), muted).boxed(),
        mono_text(format!("Tool: {}", r.tool), muted).boxed(),
    ];
    if let Some(activity) = r.activity {
        bar = bar.push(mono_text(activity.to_owned(), muted).boxed());
    }
    if let Some(summary) = r.selection.clone() {
        bar = bar
            .push(mono_button(summary, ink, StatusBarRequest::OpenPropertiesForSelection).boxed());
    }
    if r.pending_git_commits > 0 {
        let label = if r.pending_git_commits == 1 {
            "Saving…".to_string()
        } else {
            format!("Saving {}…", r.pending_git_commits)
        };
        bar = bar.push(mono_text(label, muted).boxed());
    }
    if let Some(name) = r.altium_import {
        bar = bar.push(
            mono_button(
                format!("Importing {name}… ✕"),
                ink,
                StatusBarRequest::CancelAltiumImport,
            )
            .boxed(),
        );
    }
    let bar = bar
        .push(space::horizontal().boxed())
        .push(mono_text(format!("{:.0}%", r.zoom), muted).boxed())
        .push(mono_button(format!("{unit}"), muted, StatusBarRequest::CycleUnit).boxed())
        .push(
            button(text("Panels").size(TS.section).color(ink))
                .padding([1.5, 10.0])
                .style(panels_button(tokens))
                .on_press(StatusBarRequest::TogglePanelList)
                .boxed(),
        )
        .spacing(SP.xl)
        .align_y(iced::Alignment::Center);

    container(bar)
        .width(Length::Fill)
        .height(ChromeMetrics::DEFAULT.status)
        .padding(BAR_PADDING)
        .align_y(iced::alignment::Vertical::Center)
        .style(crate::styles::status_bar(tokens))
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::theme::{ThemeId, theme_tokens};

    #[test]
    fn the_snap_pill_is_filled_only_while_snapping() {
        let t = theme_tokens(ThemeId::GraphiteLight);
        let on = snap_pill(&t, true)(&Theme::Dark, button::Status::Active);
        let off = snap_pill(&t, false)(&Theme::Dark, button::Status::Active);
        assert!(on.background.is_some());
        assert!(off.background.is_none());
        assert_ne!(on.text_color, off.text_color);
    }

    #[test]
    fn the_panels_button_has_a_border() {
        let t = theme_tokens(ThemeId::GraphiteDark);
        let s = panels_button(&t)(&Theme::Dark, button::Status::Active);
        assert_eq!(s.border.width, 1.0);
    }

    #[test]
    fn the_popup_is_a_card_with_a_shadow() {
        let t = theme_tokens(ThemeId::GraphiteLight);
        let s = popup(&t)(&Theme::Dark);
        assert_eq!(
            s.border.radius,
            iced::border::Radius::from(Radii::GRAPHITE.card)
        );
        assert!(s.shadow.blur_radius > 0.0);
        assert_eq!(popup_height(3), 3.0 * 26.0 + 8.0);
    }
}
