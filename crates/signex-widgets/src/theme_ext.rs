//! Theme bridge layer — converts `signex_types::theme::ThemeTokens` to Iced styles.
//!
//! Widgets should take their colours from these helpers rather than
//! hard-coding values. The Graphite size scale (type, spacing, radii, chrome
//! heights), the thin scrollbar, disabled fading and the power colour live
//! here too, so widgets and the app read one set of tokens.

use iced::widget::{button, container, scrollable};
use iced::{Background, Border, Color, Theme};
use signex_types::theme::{CanvasColors, Color as SxColor, ThemeTokens};

// ---------------------------------------------------------------------------
// Core color conversion
// ---------------------------------------------------------------------------

/// Convert a signex `Color` (u8 components) to an Iced `Color` (f32 0..1).
pub fn to_color(c: &SxColor) -> Color {
    Color::from_rgba8(c.r, c.g, c.b, c.a as f32 / 255.0)
}

// ---------------------------------------------------------------------------
// Text color helpers
// ---------------------------------------------------------------------------

/// Primary text color from theme tokens.
pub fn text_primary(tokens: &ThemeTokens) -> Color {
    to_color(&tokens.text)
}

/// Secondary / muted text color.
pub fn text_secondary(tokens: &ThemeTokens) -> Color {
    to_color(&tokens.text_secondary)
}

/// Accent color (for highlights, active elements).
pub fn accent(tokens: &ThemeTokens) -> Color {
    to_color(&tokens.accent)
}

/// Error color.
pub fn error_color(tokens: &ThemeTokens) -> Color {
    to_color(&tokens.error)
}

/// Warning color.
pub fn warning_color(tokens: &ThemeTokens) -> Color {
    to_color(&tokens.warning)
}

/// Success / "on" indicator color.
pub fn success_color(tokens: &ThemeTokens) -> Color {
    to_color(&tokens.success)
}

/// Border color.
pub fn border_color(tokens: &ThemeTokens) -> Color {
    to_color(&tokens.border)
}

/// Selection highlight background.
pub fn selection_color(tokens: &ThemeTokens) -> Color {
    to_color(&tokens.selection)
}

/// Hover highlight color.
pub fn hover_color(tokens: &ThemeTokens) -> Color {
    to_color(&tokens.hover)
}

// ---------------------------------------------------------------------------
// Container style factories
// ---------------------------------------------------------------------------

/// Panel background container style (side panels, docks).
pub fn panel_bg(tokens: &ThemeTokens) -> container::Style {
    container::Style {
        background: Some(to_color(&tokens.panel_bg).into()),
        text_color: Some(to_color(&tokens.text)),
        border: Border {
            width: 1.0,
            radius: 0.0.into(),
            color: to_color(&tokens.border),
            ..Border::default()
        },
        ..container::Style::default()
    }
}

/// Toolbar background container style.
pub fn toolbar_bg(tokens: &ThemeTokens) -> container::Style {
    container::Style {
        background: Some(to_color(&tokens.toolbar_bg).into()),
        text_color: Some(to_color(&tokens.text)),
        border: Border {
            width: 0.0,
            radius: 0.0.into(),
            color: to_color(&tokens.border),
            ..Border::default()
        },
        ..container::Style::default()
    }
}

/// Status bar background container style (1px top border).
pub fn status_bar_bg(tokens: &ThemeTokens) -> container::Style {
    container::Style {
        background: Some(to_color(&tokens.statusbar_bg).into()),
        text_color: Some(to_color(&tokens.text)),
        border: Border {
            width: 1.0,
            radius: 0.0.into(),
            color: to_color(&tokens.border),
            ..Border::default()
        },
        ..container::Style::default()
    }
}

/// General application background.
pub fn app_bg(tokens: &ThemeTokens) -> container::Style {
    container::Style {
        background: Some(to_color(&tokens.bg).into()),
        text_color: Some(to_color(&tokens.text)),
        border: Border::default(),
        ..container::Style::default()
    }
}

/// Paper / content area background.
pub fn paper_bg(tokens: &ThemeTokens) -> container::Style {
    container::Style {
        background: Some(to_color(&tokens.paper).into()),
        text_color: Some(to_color(&tokens.text)),
        border: Border::default(),
        ..container::Style::default()
    }
}

/// Accent-colored container (for selected / active items).
pub fn accent_bg(tokens: &ThemeTokens) -> container::Style {
    container::Style {
        background: Some(to_color(&tokens.accent).into()),
        text_color: Some(to_color(&tokens.text)),
        border: Border::default(),
        ..container::Style::default()
    }
}

// ---------------------------------------------------------------------------
// Graphite size scale: text, spacing, radii, chrome heights
// ---------------------------------------------------------------------------
//
// One base size drives every step, so a base-size preference rescales the
// chrome from one number. `signex-app` re-exports these from `styles`; the
// literal-to-step mapping is in its `styles/typography.rs`.

/// Base size, in px, the scales are defined against. It is the `body` size.
pub const DEFAULT_BASE_SIZE: f32 = 12.0;

/// The base the step ratios were authored against (the pre-#1956 `body`).
const REFERENCE_BASE_SIZE: f32 = 11.0;

/// A fixed chrome length, authored in px at an 11 px base, scaled to the
/// default base so it keeps its proportion to the text it holds.
pub const fn chrome_px(px_at_reference: f32) -> f32 {
    px_at_reference * DEFAULT_BASE_SIZE / REFERENCE_BASE_SIZE
}

/// `px` at the default 12 px base, through [`chrome_px`].
const fn at_default_base(px: f32) -> f32 {
    chrome_px(px * REFERENCE_BASE_SIZE / DEFAULT_BASE_SIZE)
}

/// iced's default relative line height (the fork's `LineHeight::default()`).
pub const LINE_HEIGHT: f32 = 1.375;

/// Thickness, in px, of a divider rule: a line weight, not a gap, so it
/// stays 1 px at any base size.
pub const HAIRLINE: f32 = 1.0;

/// Text sizes, smallest to largest.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TypeScale {
    /// Hints, secondary status lines, small badges.
    pub caption: f32,
    /// Uppercase section labels.
    pub section: f32,
    /// Default running text and controls.
    pub body: f32,
    /// Dense tables and mono readouts.
    pub dense: f32,
    /// Emphasised control labels and inputs.
    pub label: f32,
    /// Card / section titles.
    pub title: f32,
    /// Dialog headings.
    pub heading: f32,
    /// Product name in the About card.
    pub display: f32,
    /// Status icons drawn as text glyphs.
    pub glyph: f32,
    /// Rows of rule characters used as thin separators.
    pub micro: f32,
    /// The large pause icon on the placement-paused card.
    pub banner: f32,
}

impl TypeScale {
    /// The scale at [`DEFAULT_BASE_SIZE`].
    pub const DEFAULT: Self = Self::from_base(DEFAULT_BASE_SIZE);

    /// Derive every step from `base` (the `body` size).
    pub const fn from_base(base: f32) -> Self {
        Self {
            caption: base * 10.0 / 11.0,
            section: base * 11.0 / 12.0,
            body: base,
            dense: base,
            label: base * 12.0 / 11.0,
            title: base * 13.0 / 11.0,
            heading: base * 14.0 / 11.0,
            display: base * 20.0 / 11.0,
            glyph: base * 24.0 / 11.0,
            micro: base * 4.0 / 11.0,
            banner: base * 64.0 / 11.0,
        }
    }
}

impl Default for TypeScale {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Padding / gap steps, smallest to largest.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spacing {
    /// Hairline gaps between tightly packed items.
    pub xs: f32,
    /// Control padding, list-row gaps.
    pub s: f32,
    /// Standard gap between controls.
    pub m: f32,
    /// Section and card padding.
    pub l: f32,
    /// Outer dialog padding.
    pub xl: f32,
}

impl Spacing {
    /// The scale at [`DEFAULT_BASE_SIZE`].
    pub const DEFAULT: Self = Self::from_base(DEFAULT_BASE_SIZE);

    /// Derive every step from `base` (the `body` size).
    pub const fn from_base(base: f32) -> Self {
        Self {
            xs: base * 2.0 / 11.0,
            s: base * 4.0 / 11.0,
            m: base * 8.0 / 11.0,
            l: base * 12.0 / 11.0,
            xl: base * 16.0 / 11.0,
        }
    }
}

impl Default for Spacing {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Corner radii of the Graphite design, in px, smallest to largest.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Radii {
    /// Rows, menu roots and chips inside a control.
    pub small: f32,
    /// Fields, icon buttons and the Active Bar buttons.
    pub control: f32,
    /// Dialog and toolbar buttons.
    pub button: f32,
    /// Cards, dropdowns and floating panels.
    pub card: f32,
    /// Dialog surfaces.
    pub dialog: f32,
    /// Pills and scrollbar thumbs: larger than any height, so the ends are
    /// round.
    pub pill: f32,
}

impl Radii {
    pub const GRAPHITE: Self = Self {
        small: 6.0,
        control: 7.0,
        button: 8.0,
        card: 10.0,
        dialog: 12.0,
        pill: 999.0,
    };
}

impl Default for Radii {
    fn default() -> Self {
        Self::GRAPHITE
    }
}

/// Heights of the chrome bands and controls, in px. Authored at the default
/// 12 px base and scaled through [`chrome_px`] like every chrome length.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChromeMetrics {
    /// Title and menu bar.
    pub menu: f32,
    /// Document tab strip.
    pub tab_strip: f32,
    /// Dock title bar.
    pub dock_title: f32,
    /// Dock tab strip.
    pub dock_tab_strip: f32,
    /// Status bar.
    pub status: f32,
    /// Collapsed dock rail width.
    pub rail: f32,
    /// Dialog and toolbar buttons.
    pub button: f32,
    /// Text fields.
    pub field: f32,
    /// List, tree and table rows.
    pub row: f32,
    /// Width of a scrollbar thumb.
    pub scroller: f32,
    /// Clear gap on each side of a scrollbar thumb.
    pub scroller_gutter: f32,
}

impl ChromeMetrics {
    pub const DEFAULT: Self = Self {
        menu: at_default_base(38.0),
        tab_strip: at_default_base(34.0),
        dock_title: at_default_base(28.0),
        dock_tab_strip: at_default_base(28.0),
        status: at_default_base(26.0),
        rail: at_default_base(28.0),
        button: at_default_base(30.0),
        field: at_default_base(26.0),
        row: at_default_base(24.0),
        scroller: at_default_base(4.0),
        scroller_gutter: at_default_base(3.0),
    };

    /// Full width a scrollbar takes: the thumb plus a gutter on each side.
    pub fn scrollbar_width(&self) -> f32 {
        self.scroller + 2.0 * self.scroller_gutter
    }
}

impl Default for ChromeMetrics {
    fn default() -> Self {
        Self::DEFAULT
    }
}

// ---------------------------------------------------------------------------
// Thin themed scrollbar
// ---------------------------------------------------------------------------

/// Geometry of the thin scrollbar: a narrow thumb with a clear gutter and no
/// visible rail. Pass it to `scrollable::Direction::{Vertical, Horizontal}`.
pub fn thin_scrollbar(metrics: &ChromeMetrics) -> scrollable::Scrollbar {
    scrollable::Scrollbar::new()
        .width(metrics.scroller)
        .scroller_width(metrics.scroller)
        .margin(metrics.scroller_gutter)
}

/// Whether the thumb of one rail is under the pointer or being dragged.
/// Hovering the content alone leaves the thumb quiet.
pub fn scroller_is_hot(status: scrollable::Status, vertical: bool) -> bool {
    match status {
        scrollable::Status::Active { .. } => false,
        scrollable::Status::Hovered {
            is_horizontal_scrollbar_hovered,
            is_vertical_scrollbar_hovered,
            ..
        } => {
            if vertical {
                is_vertical_scrollbar_hovered
            } else {
                is_horizontal_scrollbar_hovered
            }
        }
        scrollable::Status::Dragged {
            is_horizontal_scrollbar_dragged,
            is_vertical_scrollbar_dragged,
            ..
        } => {
            if vertical {
                is_vertical_scrollbar_dragged
            } else {
                is_horizontal_scrollbar_dragged
            }
        }
    }
}

/// Style of the thin scrollbar: transparent rail, pill thumb in `thumb`,
/// `thumb_hot` while that thumb is hovered or dragged. Graphite passes
/// `line_strong` and `ink4`.
pub fn thin_scrollbar_style(
    thumb: Color,
    thumb_hot: Color,
) -> impl Fn(&Theme, scrollable::Status) -> scrollable::Style + 'static {
    move |theme: &Theme, status: scrollable::Status| {
        let mut style = scrollable::default(theme, status);
        let rail = |vertical: bool| scrollable::Rail {
            background: None,
            border: Border::default(),
            scroller: scrollable::Scroller {
                background: Background::Color(if scroller_is_hot(status, vertical) {
                    thumb_hot
                } else {
                    thumb
                }),
                border: Border {
                    radius: Radii::GRAPHITE.pill.into(),
                    ..Border::default()
                },
            },
        };
        style.vertical_rail = rail(true);
        style.horizontal_rail = rail(false);
        style.gap = None;
        style
    }
}

// ---------------------------------------------------------------------------
// Disabled fading and the power colour
// ---------------------------------------------------------------------------

/// Opacity of a disabled control: fill, border and label fade together.
pub const DISABLED_OPACITY: f32 = 0.45;

/// `c` faded to [`DISABLED_OPACITY`].
pub fn dimmed(c: Color) -> Color {
    Color {
        a: c.a * DISABLED_OPACITY,
        ..c
    }
}

/// `style` faded as a disabled control: fill, label, border and shadow all
/// at [`DISABLED_OPACITY`].
pub fn dimmed_button(style: button::Style) -> button::Style {
    let background = style.background.map(|bg| match bg {
        Background::Color(c) => Background::Color(dimmed(c)),
        other => other,
    });
    button::Style {
        background,
        text_color: dimmed(style.text_color),
        border: Border {
            color: dimmed(style.border.color),
            ..style.border
        },
        shadow: iced::Shadow {
            color: dimmed(style.shadow.color),
            ..style.shadow
        },
        ..style
    }
}

/// Colour of a power port and its net name, on the canvas and in previews:
/// Graphite Light `#C0392B`, Graphite Dark `#E06C5C`.
pub fn power_port(canvas: &CanvasColors) -> Color {
    to_color(&canvas.power)
}

/// The five kinds of a Design Notebook annotation (mockup row 165), named here for
/// their colours: this crate does not depend on `signex-notebook`, whose
/// `AnnotationKind` the app maps onto these one to one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnnotationTone {
    Note,
    Calculation,
    Measurement,
    Issue,
    Decision,
}

/// The Decision colour in a light theme (the mock-up's `--k-dec`).
const DECISION_LIGHT: Color = Color::from_rgb8(0x7A, 0x3E, 0xA8);
/// The Decision colour in a dark theme.
const DECISION_DARK: Color = Color::from_rgb8(0xD2, 0xA8, 0xF0);

/// Whether `tokens` are a light theme: a light panel fill.
fn is_light(tokens: &ThemeTokens) -> bool {
    let panel = to_color(&tokens.panel_bg);
    0.2126 * panel.r + 0.7152 * panel.g + 0.0722 * panel.b > 0.5
}

/// Colour of an annotation kind: its icon box, its chip dot and its
/// canvas flag. Note is the secondary ink, Calculation the info blue,
/// Measurement the success green, Issue the error red and Decision a purple
/// (Light `#7A3EA8`, Dark `#D2A8F0`).
pub fn annotation_kind_colour(kind: AnnotationTone, tokens: &ThemeTokens) -> Color {
    match kind {
        AnnotationTone::Note => text_secondary(tokens),
        AnnotationTone::Calculation => tokens
            .extended
            .info
            .as_ref()
            .map_or_else(|| accent(tokens), to_color),
        AnnotationTone::Measurement => success_color(tokens),
        AnnotationTone::Issue => error_color(tokens),
        AnnotationTone::Decision if is_light(tokens) => DECISION_LIGHT,
        AnnotationTone::Decision => DECISION_DARK,
    }
}

/// Colour of the glyph on an annotation kind's icon box: white in a light
/// theme, the app background in a dark one (`#111214` in Graphite Dark).
pub fn annotation_glyph_colour(tokens: &ThemeTokens) -> Color {
    if is_light(tokens) {
        Color::WHITE
    } else {
        to_color(&tokens.bg)
    }
}

#[cfg(test)]
mod scale_tests {
    use super::*;
    use signex_types::theme::{ThemeId, canvas_colors};

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.05
    }

    /// The type and spacing steps match the mockup `:root` tokens
    /// (`--ts-*`, `--sp-*`, one decimal) at the default base.
    #[test]
    fn the_default_scales_match_the_mockup_tokens() {
        let ts = TypeScale::DEFAULT;
        for (got, want) in [
            (ts.caption, 10.9),
            (ts.section, 11.0),
            (ts.body, 12.0),
            (ts.label, 13.1),
            (ts.title, 14.2),
            (ts.heading, 15.3),
        ] {
            assert!(close(got, want), "{got} != {want}");
        }
        let sp = Spacing::DEFAULT;
        for (got, want) in [
            (sp.xs, 2.2),
            (sp.s, 4.4),
            (sp.m, 8.7),
            (sp.l, 13.1),
            (sp.xl, 17.5),
        ] {
            assert!(close(got, want), "{got} != {want}");
        }
    }

    /// The `--r-*` tokens.
    #[test]
    fn the_radii_match_the_mockup_tokens() {
        let r = Radii::GRAPHITE;
        assert_eq!(
            [r.small, r.control, r.button, r.card, r.dialog, r.pill],
            [6.0, 7.0, 8.0, 10.0, 12.0, 999.0]
        );
    }

    /// At the default base the heights are the spec px (`--dock-title`,
    /// `--dock-tabs`, `--field-h`, `--row`, the 10 px scrollbar).
    #[test]
    fn the_default_heights_are_the_spec_px() {
        let m = ChromeMetrics::DEFAULT;
        for (got, want) in [
            (m.menu, 38.0),
            (m.tab_strip, 34.0),
            (m.dock_title, 28.0),
            (m.dock_tab_strip, 28.0),
            (m.status, 26.0),
            (m.rail, 28.0),
            (m.button, 30.0),
            (m.field, 26.0),
            (m.row, 24.0),
            (m.scroller, 4.0),
            (m.scroller_gutter, 3.0),
            (m.scrollbar_width(), 10.0),
        ] {
            assert!((got - want).abs() < 1e-3, "{got} != {want}");
        }
    }

    fn hovered(vertical: bool, horizontal: bool) -> scrollable::Status {
        scrollable::Status::Hovered {
            is_horizontal_scrollbar_hovered: horizontal,
            is_vertical_scrollbar_hovered: vertical,
            is_horizontal_scrollbar_disabled: false,
            is_vertical_scrollbar_disabled: false,
        }
    }

    /// Only the thumb under the pointer or being dragged lights up.
    #[test]
    fn only_the_hovered_or_dragged_thumb_is_hot() {
        let idle = scrollable::Status::Active {
            is_horizontal_scrollbar_disabled: false,
            is_vertical_scrollbar_disabled: false,
        };
        assert!(!scroller_is_hot(idle, true));
        assert!(!scroller_is_hot(hovered(false, false), true));
        assert!(scroller_is_hot(hovered(true, false), true));
        assert!(!scroller_is_hot(hovered(true, false), false));
        let dragged = scrollable::Status::Dragged {
            is_horizontal_scrollbar_dragged: true,
            is_vertical_scrollbar_dragged: false,
            is_horizontal_scrollbar_disabled: false,
            is_vertical_scrollbar_disabled: false,
        };
        assert!(scroller_is_hot(dragged, false));
        assert!(!scroller_is_hot(dragged, true));
    }

    /// The rail is invisible and only the hot thumb swaps colour.
    #[test]
    fn the_thin_scrollbar_has_no_rail_and_a_pill_thumb() {
        let thumb = Color::from_rgb(0.2, 0.2, 0.2);
        let hot = Color::from_rgb(0.8, 0.8, 0.8);
        let style = thin_scrollbar_style(thumb, hot);
        let quiet = style(&Theme::Dark, hovered(false, false));
        assert!(quiet.vertical_rail.background.is_none());
        assert_eq!(
            quiet.vertical_rail.scroller.background,
            Background::Color(thumb)
        );
        let lit = style(&Theme::Dark, hovered(true, false));
        assert_eq!(
            lit.vertical_rail.scroller.background,
            Background::Color(hot)
        );
        assert_eq!(
            lit.horizontal_rail.scroller.background,
            Background::Color(thumb)
        );
    }

    /// A disabled button keeps its colours at the disabled opacity.
    #[test]
    fn a_dimmed_button_fades_every_colour() {
        let base = button::Style {
            background: Some(Background::Color(Color::WHITE)),
            text_color: Color::BLACK,
            ..button::Style::default()
        };
        let d = dimmed_button(base);
        assert_eq!(d.text_color.a, DISABLED_OPACITY);
        assert_eq!(d.background, Some(Background::Color(dimmed(Color::WHITE))));
    }

    /// Power ports are red on both themes: `#C0392B` light, `#E06C5C` dark.
    #[test]
    fn power_ports_use_the_power_red() {
        let light = power_port(&canvas_colors(ThemeId::GraphiteLight));
        let dark = power_port(&canvas_colors(ThemeId::GraphiteDark));
        assert_eq!(light, Color::from_rgb8(0xC0, 0x39, 0x2B));
        assert_eq!(dark, Color::from_rgb8(0xE0, 0x6C, 0x5C));
    }

    /// Mockup row 165: the annotation kinds take the status tokens, Decision its own
    /// purple, and the icon-box glyph is white in Light and the app
    /// background in Dark.
    #[test]
    fn annotation_kinds_use_the_mockup_colours() {
        use signex_types::theme::theme_tokens;
        let light = theme_tokens(ThemeId::GraphiteLight);
        let dark = theme_tokens(ThemeId::GraphiteDark);
        let colour = annotation_kind_colour;
        assert_eq!(
            colour(AnnotationTone::Calculation, &light),
            Color::from_rgb8(0x1F, 0x45, 0xAE)
        );
        assert_eq!(
            colour(AnnotationTone::Calculation, &dark),
            Color::from_rgb8(0xA9, 0xC2, 0xFF)
        );
        for tokens in [&light, &dark] {
            assert_eq!(colour(AnnotationTone::Note, tokens), text_secondary(tokens));
            assert_eq!(
                colour(AnnotationTone::Measurement, tokens),
                success_color(tokens)
            );
            assert_eq!(colour(AnnotationTone::Issue, tokens), error_color(tokens));
        }
        assert_eq!(
            colour(AnnotationTone::Decision, &light),
            Color::from_rgb8(0x7A, 0x3E, 0xA8)
        );
        assert_eq!(
            colour(AnnotationTone::Decision, &dark),
            Color::from_rgb8(0xD2, 0xA8, 0xF0)
        );
        assert_eq!(annotation_glyph_colour(&light), Color::WHITE);
        assert_eq!(
            annotation_glyph_colour(&dark),
            Color::from_rgb8(0x11, 0x12, 0x14)
        );
    }
}
