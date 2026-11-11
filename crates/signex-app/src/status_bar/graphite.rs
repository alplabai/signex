//! Graphite look of the status bar and its Panels popover. Legacy themes
//! never reach these helpers.

use iced::widget::{Text, Widget, container, row, space};
use iced::{Background, Border, Color, Element, Length, Padding, Theme};
use signex_types::coord::Unit;
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, raw, text};

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

/// Active-layer pill: `accent_soft` fill, a `hover` fill under the pointer.
pub fn layer_pill(
    tokens: &ThemeTokens,
) -> impl Fn(&Theme, button::Status) -> button::Style + 'static {
    let r = resolve(tokens);
    let (fill, ink, hover) = (r.accent_soft, r.accent_soft_ink, ti(tokens.hover));
    move |_, status| button::Style {
        background: Some(Background::Color(match status {
            button::Status::Hovered | button::Status::Pressed => hover,
            _ => fill,
        })),
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
    let shadow = crate::styles::floating_shadow(tokens);
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

/// The Grid span's value: the spacing in `unit`, or "OFF".
pub(super) fn grid_label(visible: bool, size_mm: f32, unit: Unit) -> String {
    if !visible {
        return "OFF".to_string();
    }
    let mm = f64::from(size_mm);
    match unit {
        Unit::Mm => format!("{mm:.3} {unit}"),
        Unit::Mil => format!("{:.1} {unit}", mm / 0.0254),
        Unit::Inch => format!("{:.4} {unit}", mm / 25.4),
        Unit::Micrometer => format!("{:.0} {unit}", mm * 1000.0),
    }
}

fn mono_text(s: String, color: Color) -> Text<'static> {
    text(raw(s)).size(TS.body).color(color).font(mono_font())
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

/// Label of the Snap pill: "Snap on" while snapping, "Snap off" while free.
pub(super) fn snap_label(snapping: bool) -> &'static str {
    if snapping { "Snap on" } else { "Snap off" }
}

/// The Graphite status bar: mono X / Y / Grid spans, a Snap on/off pill and
/// the tool at the left; the selection, zoom, unit and the bordered Panels
/// button at the right.
/// No separators; the spans are spaced apart.
pub(super) fn view<'a>(
    r: &Readout<'_>,
    tokens: &ThemeTokens,
    i18n: &signex_i18n::I18n,
) -> Element<'a, StatusBarRequest> {
    let ink = ti(tokens.text);
    let muted = resolve(tokens).ink3;
    let unit = r.unit;
    let (xs, ys) = coord_values(unit, r.x, r.y);
    let grid = grid_label(r.grid_visible, r.grid_size_mm, unit);

    let mut bar = row![
        mono_text(format!("X {xs} {unit}"), muted).boxed(),
        mono_text(format!("Y {ys} {unit}"), muted).boxed(),
        mono_button(format!("Grid {grid}"), muted, StatusBarRequest::ToggleGrid).boxed(),
        button(mono_text(
            snap_label(r.snap_enabled).to_string(),
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
        mono_text(format!("Tool: {}", r.tool), muted).boxed(),
    ];
    if let Some(activity) = r.activity {
        let ink = if activity.is_error {
            signex_widgets::theme_ext::error_color(tokens)
        } else {
            muted
        };
        bar = bar.push(mono_text(activity.text.clone(), ink).boxed());
    }
    // The active copper layer of an open board; a click steps to the next.
    if let Some((caption, name)) = r.active_layer.clone() {
        bar = bar.push(
            button(
                row![
                    text(caption).size(TS.body).color(muted),
                    text(name).size(TS.body).color(ink),
                ]
                .spacing(SP.xs)
                .align_y(iced::Alignment::Center),
            )
            .padding([1.5, 6.0])
            .style(layer_pill(tokens))
            .on_press(StatusBarRequest::ActiveLayerStepRequested)
            .boxed(),
        );
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
    bar = bar.push(space::horizontal().boxed());
    if let Some(summary) = r.selection.clone() {
        bar = bar
            .push(mono_button(summary, ink, StatusBarRequest::OpenPropertiesForSelection).boxed());
    }
    let bar = bar
        .push(mono_text(format!("{:.0}%", r.zoom), muted).boxed())
        .push(mono_button(format!("{unit}"), muted, StatusBarRequest::CycleUnit).boxed())
        .push(
            button(
                text(signex_i18n::t!(
                    i18n,
                    signex_i18n::ids::PANELS_STATUS_BAR_PANELS
                ))
                .size(TS.section)
                .color(ink),
            )
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

    /// The grid spacing reads in the unit the bar shows, not as mm.
    #[test]
    fn the_grid_reads_in_the_shown_unit() {
        assert_eq!(grid_label(true, 0.254, Unit::Mil), "10.0 mil");
        assert_eq!(grid_label(true, 1.27, Unit::Mm), "1.270 mm");
        assert_eq!(grid_label(false, 1.27, Unit::Mm), "OFF");
    }

    #[test]
    fn the_snap_pill_reads_on_or_off() {
        assert_eq!(snap_label(true), "Snap on");
        assert_eq!(snap_label(false), "Snap off");
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
