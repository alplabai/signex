//! Target band: one length against its target and tolerance, drawn as a
//! fill bar over a thin track with the tolerance band and the target line.
//! The length-matching tab, its compact dock panel and the polygon lists
//! share it.
//!
//! The axis is zoomed to the target ± [`AXIS_TOLERANCES`] tolerances, so
//! the band always covers the middle quarter and a few tenths of a
//! millimetre stay visible. A length outside the band draws its fill in the
//! error colour. The caller passes millimetres; every readout goes through
//! [`LengthFormat`], so the widget never parses or picks a unit.
//!
//! Every length is an em of the style's text size, authored at 12 px.

use iced::widget::canvas::{self, Canvas, LineDash, Path, Stroke};
use iced::widget::text::Wrapping;
use iced::widget::{Space, row};
use iced::{
    Alignment, Color, Element, Font, Length, Point, Rectangle, Renderer, Size, Theme, Widget,
    border, font, mouse,
};
use signex_i18n::UserText;
use signex_types::theme::ThemeTokens;

use crate::theme_ext::{HAIRLINE, TypeScale, to_color};
use crate::{Label, LengthFormat, text};

/// Tolerances from the target to each end of the axis.
pub const AXIS_TOLERANCES: f64 = 4.0;
/// Dash and gap of the band edges, in em.
const DASH_EM: f32 = 3.0 / 12.0;

/// Where the band, the target and the fill sit on the axis, each a fraction
/// of the bar width (0 at the axis start, 1 at its end).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BandGeometry {
    /// Axis start, in mm (target − 4 × tolerance).
    pub axis_min: f64,
    /// Axis end, in mm (target + 4 × tolerance).
    pub axis_max: f64,
    pub band_start: f32,
    pub band_end: f32,
    pub target_at: f32,
    /// Right end of the fill, clamped to the axis.
    pub fill_end: f32,
    /// The length is further from the target than the tolerance.
    pub is_out_of_band: bool,
}

/// Whether `value` is further than `tolerance` from `target`, compared in
/// whole nanometres. A value that is not a number is out of band; a
/// negative tolerance counts as zero. The length report's own test
/// (`signex_types::pcb_length::is_out_of_band`), so the bar and the
/// report never disagree.
pub use signex_types::pcb_length::is_out_of_band;

/// The bar of `value` against `target` ± `tolerance`, all in mm.
pub fn band_geometry(value: f64, target: f64, tolerance: f64) -> BandGeometry {
    let half_band = (0.5 / AXIS_TOLERANCES) as f32;
    let span = AXIS_TOLERANCES * tolerance;
    let fill_end = if !value.is_finite() {
        0.0
    } else if span.is_finite() && span > 0.0 {
        (((value - (target - span)) / (2.0 * span)).clamp(0.0, 1.0)) as f32
    } else if value < target {
        // No tolerance: the axis has no width, so only the side shows.
        0.0
    } else if value > target {
        1.0
    } else {
        0.5
    };
    BandGeometry {
        axis_min: target - span,
        axis_max: target + span,
        band_start: 0.5 - half_band,
        band_end: 0.5 + half_band,
        target_at: 0.5,
        fill_end,
        is_out_of_band: is_out_of_band(value, target, tolerance),
    }
}

/// `delta_mm` as a signed readout in the display unit: `+0.410`, `−0.160`
/// (a true minus sign). A delta that rounds to zero reads `+0.000`.
pub fn signed_delta(format: &LengthFormat, delta_mm: f64) -> Label<'static> {
    let shown = format.field_text(delta_mm.abs());
    let rounds_to_zero = shown.chars().all(|c| matches!(c, '0' | '.' | ','));
    let sign = if delta_mm < 0.0 && !rounds_to_zero {
        '\u{2212}'
    } else {
        '+'
    };
    UserText::new(format!("{sign}{shown}")).into()
}

/// The full bar of the tab, or the small one of the compact panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BandSize {
    Full,
    Small,
}

/// Lengths of one bar size, in px at the style's text size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BandMetrics {
    pub height: f32,
    pub track_top: f32,
    pub track_height: f32,
    /// Gap between the band and the top and bottom of the bar.
    pub band_inset: f32,
    pub fill_top: f32,
    pub fill_height: f32,
    pub tick_width: f32,
    pub tick_height: f32,
    pub dash: f32,
}

impl BandMetrics {
    /// Full: 18 high, fill at 6. Small: 14 high, fill at 4 (em of 12 px).
    pub fn new(size: BandSize, text_size: f32) -> Self {
        let em = |px: f32| px * text_size / 12.0;
        let (height, track_top, fill_top) = match size {
            BandSize::Full => (18.0, 8.0, 6.0),
            BandSize::Small => (14.0, 6.0, 4.0),
        };
        Self {
            height: em(height),
            track_top: em(track_top),
            track_height: em(2.0),
            band_inset: em(1.0),
            fill_top: em(fill_top),
            fill_height: em(6.0),
            tick_width: em(2.0),
            tick_height: em(12.0),
            dash: text_size * DASH_EM,
        }
    }
}

/// Colours of the bar and its axis row, and the text size its lengths
/// derive from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TargetBandStyle {
    pub text_size: f32,
    pub track: Color,
    pub band: Color,
    pub ok: Color,
    pub fill: Color,
    pub err: Color,
    /// Axis end labels.
    pub axis_ink: Color,
}

impl TargetBandStyle {
    /// Graphite tokens: track `line-soft`, band `ok-soft` edged `ok`, fill
    /// `ink4`, out of band `err`. An unset extended token falls back to the
    /// nearest base token.
    pub fn from_tokens(tokens: &ThemeTokens, text_size: f32) -> Self {
        let ext = &tokens.extended;
        let or = |c: Option<signex_types::theme::Color>, base| to_color(&c.unwrap_or(base));
        Self {
            text_size,
            track: or(ext.line_soft, tokens.border),
            band: or(ext.ok_soft, tokens.hover),
            ok: to_color(&tokens.success),
            fill: or(ext.ink4, tokens.text_secondary),
            err: to_color(&tokens.error),
            axis_ink: or(ext.ink4, tokens.text_secondary),
        }
    }
}

/// Draws one bar; holds no state.
struct BandProgram {
    geometry: BandGeometry,
    metrics: BandMetrics,
    style: TargetBandStyle,
}

impl BandProgram {
    fn paint(&self, frame: &mut canvas::Frame, width: f32) {
        let (g, m, s) = (self.geometry, self.metrics, self.style);
        let x = |at: f32| at * width;
        frame.fill(
            &Path::rounded_rectangle(
                Point::new(0.0, m.track_top),
                Size::new(width, m.track_height),
                (m.track_height / 2.0).into(),
            ),
            s.track,
        );
        let (band_x0, band_x1) = (x(g.band_start), x(g.band_end));
        let band_h = m.height - 2.0 * m.band_inset;
        frame.fill_rectangle(
            Point::new(band_x0, m.band_inset),
            Size::new(band_x1 - band_x0, band_h),
            s.band,
        );
        let segments = [m.dash, m.dash];
        let dashed = || Stroke {
            line_dash: LineDash {
                segments: &segments,
                offset: 0,
            },
            ..Stroke::default().with_color(s.ok).with_width(HAIRLINE)
        };
        for edge in [band_x0 + HAIRLINE / 2.0, band_x1 - HAIRLINE / 2.0] {
            frame.stroke(
                &Path::line(
                    Point::new(edge, m.band_inset),
                    Point::new(edge, m.height - m.band_inset),
                ),
                dashed(),
            );
        }
        let target_x = x(g.target_at) - HAIRLINE / 2.0;
        frame.fill_rectangle(
            Point::new(target_x, 0.0),
            Size::new(HAIRLINE, m.height),
            s.ok,
        );
        let ink = if g.is_out_of_band { s.err } else { s.fill };
        let fill_x = x(g.fill_end);
        if fill_x > 0.0 {
            let r = (m.fill_height / 2.0).min(fill_x);
            let rounded_right = border::Radius {
                top_left: 0.0,
                top_right: r,
                bottom_right: r,
                bottom_left: 0.0,
            };
            frame.fill(
                &Path::rounded_rectangle(
                    Point::new(0.0, m.fill_top),
                    Size::new(fill_x, m.fill_height),
                    rounded_right,
                ),
                ink,
            );
        }
        let tick_x = (fill_x - m.tick_width / 2.0).clamp(0.0, (width - m.tick_width).max(0.0));
        let tick_top = m.fill_top + m.fill_height / 2.0 - m.tick_height / 2.0;
        frame.fill(
            &Path::rounded_rectangle(
                Point::new(tick_x, tick_top),
                Size::new(m.tick_width, m.tick_height),
                (m.tick_width / 2.0).into(),
            ),
            ink,
        );
    }
}

impl<M> canvas::Program<M> for BandProgram {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        self.paint(&mut frame, bounds.width);
        vec![frame.into_geometry()]
    }
}

/// The bar of `value` against `target` ± `tolerance` (mm), filling the
/// width it is given.
pub fn target_band<'a, M: 'a>(
    value: f64,
    target: f64,
    tolerance: f64,
    size: BandSize,
    style: TargetBandStyle,
) -> Element<'a, M> {
    let metrics = BandMetrics::new(size, style.text_size);
    Canvas::<BandProgram, M>::new(BandProgram {
        geometry: band_geometry(value, target, tolerance),
        metrics,
        style,
    })
    .width(Length::Fill)
    .height(Length::Fixed(metrics.height))
    .boxed()
}

/// The axis caption under a column of full bars: the axis start, the target
/// (in the `ok` ink, medium weight) and the axis end, in the mono face.
pub fn axis_row<'a, M: 'a>(
    format: &LengthFormat,
    target: f64,
    tolerance: f64,
    style: TargetBandStyle,
) -> Element<'a, M> {
    let g = band_geometry(target, target, tolerance);
    let size = TypeScale::from_base(style.text_size).caption;
    let shown = |mm: f64| -> Label<'static> { UserText::new(format.field_text(mm)).into() };
    let end = |mm: f64| {
        text(shown(mm))
            .size(size)
            .font(Font::MONOSPACE)
            .color(style.axis_ink)
            .wrapping(Wrapping::None)
    };
    let centre = text(shown(target))
        .size(size)
        .font(Font {
            weight: font::Weight::Medium,
            ..Font::MONOSPACE
        })
        .color(style.ok)
        .wrapping(Wrapping::None);
    row![
        end(g.axis_min),
        Space::new().width(Length::Fill),
        centre,
        Space::new().width(Length::Fill),
        end(g.axis_max),
    ]
    .align_y(Alignment::Center)
    .width(Length::Fill)
    .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_i18n::I18n;
    use signex_types::coord::Unit;
    use signex_types::theme::{ThemeId, theme_tokens};

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    #[test]
    fn the_band_is_the_middle_quarter_of_the_axis() {
        let g = band_geometry(28.5, 28.5, 0.25);
        assert!(close(g.band_start, 0.375));
        assert!(close(g.band_end, 0.625));
        assert!(close(g.target_at, 0.5));
        assert!(close(g.fill_end, 0.5));
        assert!((g.axis_min - 27.5).abs() < 1e-12);
        assert!((g.axis_max - 29.5).abs() < 1e-12);
    }

    #[test]
    fn the_fill_follows_the_value_along_the_axis() {
        // 28.91 on 28.50 ± 0.25: (28.91 − 27.5) / 2.0.
        assert!(close(band_geometry(28.91, 28.5, 0.25).fill_end, 0.705));
        assert!(close(band_geometry(28.25, 28.5, 0.25).fill_end, 0.375));
    }

    #[test]
    fn a_value_off_the_axis_is_clamped_to_its_end() {
        assert_eq!(band_geometry(10.0, 28.5, 0.25).fill_end, 0.0);
        assert_eq!(band_geometry(99.0, 28.5, 0.25).fill_end, 1.0);
        assert_eq!(band_geometry(f64::NAN, 28.5, 0.25).fill_end, 0.0);
    }

    #[test]
    fn err_is_set_exactly_when_the_value_leaves_the_band() {
        assert!(!band_geometry(28.75, 28.5, 0.25).is_out_of_band);
        assert!(!band_geometry(28.25, 28.5, 0.25).is_out_of_band);
        assert!(band_geometry(28.750_001, 28.5, 0.25).is_out_of_band);
        assert!(band_geometry(28.249_999, 28.5, 0.25).is_out_of_band);
        // Float noise on the edge (12.55 − 12.40 is 0.150…036) is in band.
        assert!(!band_geometry(12.55, 12.4, 0.15).is_out_of_band);
        assert!(band_geometry(28.91, 28.5, 0.25).is_out_of_band);
        assert!(band_geometry(f64::NAN, 28.5, 0.25).is_out_of_band);
    }

    #[test]
    fn no_tolerance_shows_the_side_only() {
        assert_eq!(band_geometry(1.0, 2.0, 0.0).fill_end, 0.0);
        assert_eq!(band_geometry(3.0, 2.0, 0.0).fill_end, 1.0);
        assert_eq!(band_geometry(2.0, 2.0, 0.0).fill_end, 0.5);
        assert!(!band_geometry(2.0, 2.0, 0.0).is_out_of_band);
        assert_eq!(band_geometry(2.0, 2.0, -1.0).fill_end, 0.5);
    }

    #[test]
    fn the_delta_reads_with_a_sign_in_the_display_unit() {
        let mm = LengthFormat::new(I18n::en_us_ref(), Unit::Mm);
        assert_eq!(signed_delta(&mm, 0.41).as_str(), "+0.410");
        assert_eq!(signed_delta(&mm, -0.16).as_str(), "\u{2212}0.160");
        assert_eq!(signed_delta(&mm, -0.000_01).as_str(), "+0.000");
        let mil = LengthFormat::new(I18n::en_us_ref(), Unit::Mil);
        assert_eq!(signed_delta(&mil, -0.254).as_str(), "\u{2212}10.00");
    }

    #[test]
    fn the_small_bar_is_shorter_and_its_fill_sits_higher() {
        let full = BandMetrics::new(BandSize::Full, 12.0);
        let small = BandMetrics::new(BandSize::Small, 12.0);
        assert!(close(full.height, 18.0) && close(full.fill_top, 6.0));
        assert!(close(small.height, 14.0) && close(small.fill_top, 4.0));
        assert!(close(BandMetrics::new(BandSize::Full, 13.0).height, 19.5));
    }

    #[test]
    fn ok_and_err_differ_on_both_themes() {
        for id in ThemeId::BUILTINS {
            let s = TargetBandStyle::from_tokens(&theme_tokens(*id), 12.0);
            assert_ne!(s.ok, s.err, "{id:?}");
            assert_ne!(s.fill, s.err, "{id:?}");
        }
    }

    #[derive(Debug, Clone)]
    enum Msg {}

    #[test]
    fn builds_both_sizes_and_the_axis_row() {
        let style = TargetBandStyle::from_tokens(&theme_tokens(ThemeId::DEFAULT), 12.0);
        let mm = LengthFormat::new(I18n::en_us_ref(), Unit::Mm);
        let _: Element<'_, Msg> = target_band(28.4, 28.5, 0.25, BandSize::Full, style);
        let _: Element<'_, Msg> = target_band(28.91, 28.5, 0.25, BandSize::Small, style);
        let _: Element<'_, Msg> = axis_row(&mm, 28.5, 0.25, style);
    }
}
