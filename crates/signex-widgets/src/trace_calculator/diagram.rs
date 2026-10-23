//! Explanatory cross-section drawn above the results: an outer trace on
//! the board surface and an inner trace buried between two dielectric
//! layers, annotated with the quantities the calculator works with (width
//! W, copper thickness T, area A = W x T, current I, length L, rise dT).
//!
//! The proportions are schematic and never scaled to the entered numbers;
//! the only live content is the computed width next to each W. Colours
//! come from [`ThemeTokens`] only. Own drawing, not a copy of any figure.

use std::cell::RefCell;
use std::fmt;

use iced::widget::canvas::{self, Geometry, Path, Stroke, Text};
use iced::{Color, Element, Length, Point, Rectangle, Size, Theme, alignment, mouse};
use signex_types::theme::ThemeTokens;

use super::control::format_number;
use super::domain::{LayerResult, TraceCalculation, WidthUnit};
use crate::theme_ext;

/// Height the control reserves for the drawing.
pub const DIAGRAM_HEIGHT: f32 = 158.0;

const LABEL_SIZE: f32 = 10.0;
const LINE_WIDTH: f32 = 1.0;
const HATCH_SPACING: f32 = 5.0;
const TICK: f32 = 3.0;
const FILL_ALPHA: f32 = 0.25;
const MARKER_RADIUS: f32 = 5.0;

/// The text the diagram shows next to W for each layer, `None` when the
/// inputs gave no result.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DiagramLabels {
    pub outer_width: Option<String>,
    pub inner_width: Option<String>,
}

/// Build the W labels from a calculation. Pure, so it is tested without
/// rendering.
pub fn width_labels(
    calculation: Option<&TraceCalculation>,
    outer_unit: WidthUnit,
    inner_unit: WidthUnit,
) -> DiagramLabels {
    DiagramLabels {
        outer_width: calculation.map(|c| width_label(c.outer, outer_unit)),
        inner_width: calculation.map(|c| width_label(c.inner, inner_unit)),
    }
}

fn width_label(result: LayerResult, unit: WidthUnit) -> String {
    format!("W = {} {unit}", format_number(result.width(unit)))
}

/// The colours the drawing uses, taken from the theme.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Palette {
    text: Color,
    muted: Color,
    outline: Color,
    copper: Color,
    dielectric: Color,
    background: Color,
}

impl Palette {
    fn from_tokens(tokens: &ThemeTokens) -> Self {
        Self {
            text: theme_ext::text_primary(tokens),
            muted: theme_ext::text_secondary(tokens),
            outline: theme_ext::border_color(tokens),
            copper: theme_ext::accent(tokens),
            dielectric: theme_ext::to_color(&tokens.panel_bg),
            background: theme_ext::to_color(&tokens.bg),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct DiagramKey {
    labels: DiagramLabels,
    palette: Palette,
}

/// Canvas geometry cache, cleared only when the labels or the theme
/// colours change (a size change is handled by the canvas cache itself).
#[derive(Default)]
pub struct DiagramCache {
    cache: canvas::Cache,
    key: RefCell<Option<DiagramKey>>,
}

impl Clone for DiagramCache {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl fmt::Debug for DiagramCache {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DiagramCache")
    }
}

impl DiagramCache {
    fn geometry(&self, renderer: &iced::Renderer, size: Size, key: DiagramKey) -> Geometry {
        if self.key.borrow().as_ref() != Some(&key) {
            self.cache.clear();
            *self.key.borrow_mut() = Some(key.clone());
        }
        self.cache.draw(renderer, size, |frame| paint(frame, &key))
    }
}

struct Diagram<'a> {
    cache: &'a DiagramCache,
    key: DiagramKey,
}

/// The diagram as an element, borrowing the control's cache.
pub fn view<'a, M: 'a>(
    cache: &'a DiagramCache,
    labels: DiagramLabels,
    tokens: &ThemeTokens,
) -> Element<'a, M> {
    let key = DiagramKey {
        labels,
        palette: Palette::from_tokens(tokens),
    };
    canvas::Canvas::new(Diagram { cache, key })
        .width(Length::Fill)
        .height(DIAGRAM_HEIGHT)
        .into()
}

impl<M> canvas::Program<M> for Diagram<'_> {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        vec![
            self.cache
                .geometry(renderer, bounds.size(), self.key.clone()),
        ]
    }
}

fn stroke(color: Color) -> Stroke<'static> {
    Stroke::default().with_color(color).with_width(LINE_WIDTH)
}

fn label(
    frame: &mut canvas::Frame,
    content: &str,
    at: Point,
    color: Color,
    align_x: alignment::Horizontal,
) {
    frame.fill_text(Text {
        content: content.to_string(),
        position: at,
        size: LABEL_SIZE.into(),
        color,
        align_x: align_x.into(),
        align_y: alignment::Vertical::Center,
        ..Text::default()
    });
}

/// Where everything sits, in frame pixels. Pure so the spacing rules can
/// be tested without rendering. Inner is on the left and outer on the
/// right, the same order as the result panels below.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Layout {
    slab: Rectangle,
    /// Joint of the two dielectric layers; the inner trace is centred on it.
    joint_y: f32,
    inner: Rectangle,
    outer: Rectangle,
    outer_dimension_y: f32,
    outer_label_y: f32,
    inner_dimension_y: f32,
    inner_label_y: f32,
    caption_y: f32,
    legend: Point,
}

const COPPER_DRAWN: f32 = 18.0;
const TOP_BAND: f32 = 48.0;
const UPPER_LAYER: f32 = 46.0;
const LOWER_LAYER: f32 = 38.0;
const DIMENSION_GAP: f32 = 12.0;
const LABEL_GAP: f32 = 9.0;
const CAPTION_GAP: f32 = 14.0;

fn layout(width: f32) -> Layout {
    let slab_top = TOP_BAND;
    let joint_y = slab_top + UPPER_LAYER;
    let slab_bottom = joint_y + LOWER_LAYER;
    let trace_w = 0.22 * width;
    let outer = Rectangle::new(
        Point::new(0.62 * width, slab_top - COPPER_DRAWN),
        Size::new(trace_w, COPPER_DRAWN),
    );
    let inner = Rectangle::new(
        Point::new(0.12 * width, joint_y - COPPER_DRAWN / 2.0),
        Size::new(trace_w, COPPER_DRAWN),
    );
    let inner_dimension_y = inner.y - DIMENSION_GAP;
    let outer_dimension_y = outer.y - DIMENSION_GAP;
    Layout {
        slab: Rectangle::new(
            Point::new(0.05 * width, slab_top),
            Size::new(0.90 * width, slab_bottom - slab_top),
        ),
        joint_y,
        inner,
        outer,
        outer_dimension_y,
        outer_label_y: outer_dimension_y - LABEL_GAP,
        inner_dimension_y,
        inner_label_y: inner_dimension_y - LABEL_GAP,
        caption_y: slab_bottom + CAPTION_GAP,
        legend: Point::new(0.12 * width, 12.0),
    }
}

fn paint(frame: &mut canvas::Frame, key: &DiagramKey) {
    let palette = key.palette;
    let layout = layout(frame.width());
    let slab = layout.slab;

    // Two dielectric layers; the inner trace sits on their joint.
    let layers = [
        (slab.y, layout.joint_y),
        (layout.joint_y, slab.y + slab.height),
    ];
    for (top, bottom) in layers {
        let origin = Point::new(slab.x, top);
        let size = Size::new(slab.width, bottom - top);
        frame.fill_rectangle(origin, size, palette.dielectric);
        frame.stroke(&Path::rectangle(origin, size), stroke(palette.outline));
    }

    for trace in [layout.inner, layout.outer] {
        draw_trace(frame, trace, &palette);
    }

    annotate(
        frame,
        layout.inner,
        (layout.inner_dimension_y, layout.inner_label_y),
        &key.labels.inner_width,
        &palette,
    );
    annotate(
        frame,
        layout.outer,
        (layout.outer_dimension_y, layout.outer_label_y),
        &key.labels.outer_width,
        &palette,
    );
    for (name, trace) in [("Inner layer", layout.inner), ("Outer layer", layout.outer)] {
        label(
            frame,
            name,
            Point::new(trace.center_x(), layout.caption_y),
            palette.text,
            alignment::Horizontal::Center,
        );
    }
    legend(frame, layout.legend, &palette);
}

fn draw_trace(frame: &mut canvas::Frame, trace: Rectangle, palette: &Palette) {
    let fill = Color {
        a: FILL_ALPHA,
        ..palette.copper
    };
    frame.fill_rectangle(trace.position(), trace.size(), fill);
    hatch(frame, trace, stroke(palette.copper));
    frame.stroke(
        &Path::rectangle(trace.position(), trace.size()),
        stroke(palette.copper),
    );
    marker(frame, trace.center(), palette);
}

/// 45 degree hatching clipped to `rect`.
fn hatch(frame: &mut canvas::Frame, rect: Rectangle, line: Stroke<'static>) {
    let (x0, y0) = (rect.x, rect.y);
    let (x1, y1) = (rect.x + rect.width, rect.y + rect.height);
    let mut c = x0 - y1;
    while c < x1 - y0 {
        // The line y = x - c, clipped to the rectangle.
        let xa = x0.max(c + y0);
        let xb = x1.min(c + y1);
        if xa < xb {
            frame.stroke(
                &Path::line(Point::new(xa, xa - c), Point::new(xb, xb - c)),
                line,
            );
        }
        c += HATCH_SPACING;
    }
}

/// Circle with a dot: a vector pointing out of the page, which is the
/// direction of both the current I and the length L.
fn marker(frame: &mut canvas::Frame, at: Point, palette: &Palette) {
    frame.fill(&Path::circle(at, MARKER_RADIUS), palette.background);
    frame.stroke(&Path::circle(at, MARKER_RADIUS), stroke(palette.text));
    frame.fill(&Path::circle(at, 1.5), palette.text);
}

fn horizontal_dimension(frame: &mut canvas::Frame, x0: f32, x1: f32, y: f32, color: Color) {
    frame.stroke(
        &Path::line(Point::new(x0, y), Point::new(x1, y)),
        stroke(color),
    );
    for x in [x0, x1] {
        frame.stroke(
            &Path::line(Point::new(x, y - TICK), Point::new(x, y + TICK)),
            stroke(color),
        );
    }
}

fn vertical_dimension(frame: &mut canvas::Frame, x: f32, y0: f32, y1: f32, color: Color) {
    frame.stroke(
        &Path::line(Point::new(x, y0), Point::new(x, y1)),
        stroke(color),
    );
    for y in [y0, y1] {
        frame.stroke(
            &Path::line(Point::new(x - TICK, y), Point::new(x + TICK, y)),
            stroke(color),
        );
    }
}

fn width_text(value: &Option<String>) -> &str {
    value.as_deref().unwrap_or("W")
}

/// W above the trace, T on its left, A = W x T on its right. The W
/// dimension line and its label sit above the trace, clear of the slab's
/// borders and of the layer caption below the slab.
fn annotate(
    frame: &mut canvas::Frame,
    trace: Rectangle,
    (dimension_y, label_y): (f32, f32),
    width: &Option<String>,
    palette: &Palette,
) {
    horizontal_dimension(
        frame,
        trace.x,
        trace.x + trace.width,
        dimension_y,
        palette.muted,
    );
    label(
        frame,
        width_text(width),
        Point::new(trace.center_x(), label_y),
        palette.text,
        alignment::Horizontal::Center,
    );
    let t_x = trace.x - 10.0;
    vertical_dimension(frame, t_x, trace.y, trace.y + trace.height, palette.muted);
    label(
        frame,
        "T",
        Point::new(t_x - 6.0, trace.center_y()),
        palette.text,
        alignment::Horizontal::Right,
    );
    label(
        frame,
        "A = W \u{d7} T",
        Point::new(trace.x + trace.width + 10.0, trace.center_y()),
        palette.text,
        alignment::Horizontal::Left,
    );
}

/// What the symbols mean, in the free band above the slab, over the inner trace.
fn legend(frame: &mut canvas::Frame, at: Point, palette: &Palette) {
    marker(frame, at, palette);
    label(
        frame,
        "I and L run along the trace",
        Point::new(at.x + 12.0, at.y),
        palette.muted,
        alignment::Horizontal::Left,
    );
    label(
        frame,
        "\u{394}T = rise above ambient",
        Point::new(at.x, at.y + 18.0),
        palette.muted,
        alignment::Horizontal::Left,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trace_calculator::control::{TraceCalculatorControl, TraceCalculatorMessage};

    #[test]
    fn labels_come_from_the_computed_widths_in_the_selected_units() {
        let control = TraceCalculatorControl::default();
        let calculation = control.calculation().expect("defaults are valid");

        let labels = width_labels(Some(&calculation), WidthUnit::Millimeter, WidthUnit::Mil);

        let outer_mm = calculation.outer.width(WidthUnit::Millimeter);
        let inner_mil = calculation.inner.width(WidthUnit::Mil);
        assert_eq!(
            labels.outer_width,
            Some(format!("W = {} mm", format_number(outer_mm)))
        );
        assert_eq!(
            labels.inner_width,
            Some(format!("W = {} mil", format_number(inner_mil)))
        );
    }

    #[test]
    fn outer_and_inner_labels_differ_because_k_differs() {
        let control = TraceCalculatorControl::default();
        let calculation = control.calculation().expect("defaults are valid");
        let labels = width_labels(Some(&calculation), WidthUnit::Mil, WidthUnit::Mil);
        assert_ne!(labels.outer_width, labels.inner_width);
    }

    #[test]
    fn invalid_input_yields_no_value_labels() {
        let mut control = TraceCalculatorControl::default();
        control.update(TraceCalculatorMessage::CurrentChanged("x".into()));
        assert!(control.calculation().is_err());

        let labels = width_labels(
            control.calculation().ok().as_ref(),
            WidthUnit::Millimeter,
            WidthUnit::Millimeter,
        );

        assert_eq!(labels, DiagramLabels::default());
        assert_eq!(width_text(&labels.outer_width), "W");
    }

    #[test]
    fn the_palette_follows_the_theme_tokens() {
        use signex_types::theme::{ThemeId, theme_tokens};
        let dark = Palette::from_tokens(&theme_tokens(ThemeId::Signex));
        let light = Palette::from_tokens(&theme_tokens(ThemeId::SolarizedLight));
        assert_ne!(dark, light);
        assert_eq!(
            dark.copper,
            theme_ext::accent(&theme_tokens(ThemeId::Signex))
        );
    }

    const WIDTHS: [f32; 3] = [560.0, 616.0, 1100.0];

    #[test]
    fn inner_is_left_and_outer_right_like_the_result_panels() {
        for width in WIDTHS {
            let l = layout(width);
            assert!(l.inner.x + l.inner.width < l.outer.x, "at {width}");
        }
    }

    #[test]
    fn the_outer_trace_is_on_the_surface_and_the_inner_one_is_buried() {
        let l = layout(616.0);
        assert_eq!(l.outer.y + l.outer.height, l.slab.y);
        assert!(l.inner.y > l.slab.y);
        assert!(l.inner.y + l.inner.height < l.slab.y + l.slab.height);
    }

    #[test]
    fn the_inner_dimension_label_clears_the_slab_border_and_the_trace() {
        let l = layout(616.0);
        let half = LABEL_SIZE / 2.0;
        assert!(
            l.inner_label_y - half > l.slab.y,
            "label touches the top border"
        );
        assert!(l.inner_label_y + half < l.inner_dimension_y);
        assert!(l.inner_dimension_y < l.inner.y);
    }

    #[test]
    fn captions_sit_below_the_slab_and_inside_the_canvas() {
        let l = layout(616.0);
        let half = LABEL_SIZE / 2.0;
        assert!(l.caption_y - half > l.slab.y + l.slab.height + 4.0);
        assert!(l.caption_y + half <= DIAGRAM_HEIGHT);
    }

    #[test]
    fn the_legend_stays_clear_of_both_width_labels() {
        for width in WIDTHS {
            let l = layout(width);
            // Rough text extent: LABEL_SIZE * 0.55 per character.
            let legend_right = l.legend.x + 12.0 + 27.0 * LABEL_SIZE * 0.55;
            let outer_label_left = l.outer.center_x() - 6.0 * LABEL_SIZE * 0.55;
            assert!(legend_right < outer_label_left, "at {width}");
            assert!(l.legend.x <= l.inner.x, "at {width}");
        }
    }

    #[test]
    fn the_top_band_holds_the_outer_label_and_the_legend() {
        let l = layout(616.0);
        assert!(l.outer_label_y - LABEL_SIZE / 2.0 >= 0.0);
        assert!(l.legend.y + 18.0 + LABEL_SIZE / 2.0 < l.slab.y);
    }
}
