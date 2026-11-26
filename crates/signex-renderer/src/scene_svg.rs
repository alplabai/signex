//! A world-space [`Scene`] as an SVG document, headless (#3116).
//!
//! The editors fill a `Scene` through the renderer's own builders and replay
//! it onto a canvas; this replays the same buckets, in the same shared order
//! ([`GPU_SCENE_DRAW_ORDER`]), as SVG elements, one user unit per millimetre,
//! y down as the editors' canvases map it. Output is deterministic: every
//! number is printed with four decimals, trailing zeros trimmed.

use std::fmt::Write as _;

use signex_gfx::primitive::arc::Arc;
use signex_gfx::primitive::arc::{arc_is_full_turn_rad, ccw_wrapped_sweep_rad};
use signex_gfx::primitive::circle::Circle;
use signex_gfx::primitive::line::LineSegment;
use signex_gfx::primitive::ordered::OrderedItem;
use signex_gfx::primitive::polygon::GpuPolygon;
use signex_gfx::primitive::text::{TextHAlign, TextItem, TextVAlign};
use signex_gfx::scene::{GPU_SCENE_DRAW_ORDER, Scene, SceneBucket};

/// How a scene is framed.
#[derive(Debug, Clone, Copy)]
pub struct SceneSvgOptions {
    /// Page colour, RGBA 0..=1.
    pub background: [f32; 4],
    /// Free margin around the drawn extent, mm.
    pub margin_mm: f32,
    /// Narrowest stroke drawn, mm: the canvases floor a hairline in pixels.
    pub min_stroke_mm: f32,
}

impl Default for SceneSvgOptions {
    fn default() -> Self {
        Self {
            background: [1.0, 1.0, 1.0, 1.0],
            margin_mm: 1.0,
            min_stroke_mm: 0.05,
        }
    }
}

/// Number text: four decimals, trailing zeros trimmed, no `-0`.
fn num(v: f32) -> String {
    let mut s = format!("{v:.4}");
    if s.contains('.') {
        s.truncate(s.trim_end_matches('0').trim_end_matches('.').len());
    }
    if s == "-0" { "0".into() } else { s }
}

fn rgb(c: [f32; 4]) -> String {
    let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}", b(c[0]), b(c[1]), b(c[2]))
}

/// ` <attr>="#rrggbb"` plus `<attr>-opacity` when not opaque.
fn paint(attr: &str, c: [f32; 4]) -> String {
    if c[3] >= 0.999 {
        format!(" {attr}=\"{}\"", rgb(c))
    } else {
        format!(" {attr}=\"{}\" {attr}-opacity=\"{}\"", rgb(c), num(c[3]))
    }
}

#[derive(Default)]
struct Extent(Option<[f32; 4]>);

impl Extent {
    fn grow(&mut self, x: f32, y: f32, pad: f32) {
        if !(x.is_finite() && y.is_finite()) {
            return;
        }
        let e = self.0.get_or_insert([x, y, x, y]);
        *e = [
            e[0].min(x - pad),
            e[1].min(y - pad),
            e[2].max(x + pad),
            e[3].max(y + pad),
        ];
    }
}

struct Writer<'a> {
    out: String,
    extent: Extent,
    opts: &'a SceneSvgOptions,
}

impl Writer<'_> {
    fn width(&self, w: f32) -> f32 {
        w.max(self.opts.min_stroke_mm)
    }

    fn line(&mut self, l: &LineSegment) {
        let w = self.width(l.width);
        self.extent.grow(l.p0[0], l.p0[1], w / 2.0);
        self.extent.grow(l.p1[0], l.p1[1], w / 2.0);
        let dash = if l.is_dashed() {
            format!(" stroke-dasharray=\"{} {}\"", num(w * 4.0), num(w * 3.0))
        } else {
            String::new()
        };
        let _ = writeln!(
            self.out,
            "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke-width=\"{}\" stroke-linecap=\"round\"{}{dash}/>",
            num(l.p0[0]),
            num(l.p0[1]),
            num(l.p1[0]),
            num(l.p1[1]),
            num(w),
            paint("stroke", l.color)
        );
    }

    fn circle(&mut self, c: &Circle) {
        let r = c.radius.max(0.0);
        self.extent
            .grow(c.center[0], c.center[1], r + c.stroke_width.max(0.0) / 2.0);
        let look = if c.is_filled() {
            paint("fill", c.color)
        } else {
            format!(
                " fill=\"none\" stroke-width=\"{}\"{}",
                num(self.width(c.stroke_width)),
                paint("stroke", c.color)
            )
        };
        let _ = writeln!(
            self.out,
            "<circle cx=\"{}\" cy=\"{}\" r=\"{}\"{look}/>",
            num(c.center[0]),
            num(c.center[1]),
            num(r)
        );
    }

    fn arc(&mut self, a: &Arc) {
        let w = self.width(a.width);
        self.extent
            .grow(a.center[0], a.center[1], a.radius + w / 2.0);
        let at = |t: f32| {
            (
                a.center[0] + a.radius * t.cos(),
                a.center[1] + a.radius * t.sin(),
            )
        };
        let (x0, y0) = at(a.start_angle);
        let look = format!(
            " fill=\"none\" stroke-width=\"{}\"{}",
            num(w),
            paint("stroke", a.color)
        );
        if arc_is_full_turn_rad(a.start_angle, a.end_angle) {
            let _ = writeln!(
                self.out,
                "<circle cx=\"{}\" cy=\"{}\" r=\"{}\"{look}/>",
                num(a.center[0]),
                num(a.center[1]),
                num(a.radius)
            );
            return;
        }
        let sweep = ccw_wrapped_sweep_rad(a.start_angle, a.end_angle);
        let (x1, y1) = at(a.start_angle + sweep);
        let large = u8::from(sweep > std::f32::consts::PI);
        let _ = writeln!(
            self.out,
            "<path d=\"M{} {} A{r} {r} 0 {large} 1 {} {}\"{look}/>",
            num(x0),
            num(y0),
            num(x1),
            num(y1),
            r = num(a.radius)
        );
    }

    fn polygon(&mut self, p: &GpuPolygon) {
        if p.vertices.len() < 2 {
            return;
        }
        let mut d = String::new();
        for contour in std::iter::once(&p.vertices).chain(p.holes.iter().filter(|h| h.len() >= 3)) {
            for (i, v) in contour.iter().enumerate() {
                self.extent.grow(v[0], v[1], 0.0);
                let _ = write!(
                    d,
                    "{}{} {}",
                    if i == 0 { 'M' } else { 'L' },
                    num(v[0]),
                    num(v[1])
                );
            }
            d.push('Z');
        }
        let fill = if p.fill_color[3] > 0.0 {
            paint("fill", p.fill_color)
        } else {
            " fill=\"none\"".into()
        };
        let stroke = p.stroke_color.map_or(String::new(), |c| {
            format!(
                " stroke-width=\"{}\" stroke-linejoin=\"round\"{}",
                num(self.width(p.stroke_width)),
                paint("stroke", c)
            )
        });
        let _ = writeln!(
            self.out,
            "<path d=\"{d}\" fill-rule=\"evenodd\"{fill}{stroke}/>"
        );
    }

    fn text(&mut self, t: &TextItem) {
        if t.content.is_empty() {
            return;
        }
        let em = t.size_mm / signex_types::font::mm_per_em(t.family.as_deref());
        self.extent.grow(
            t.position[0],
            t.position[1],
            em * 0.6 * t.content.chars().count() as f32 / 2.0,
        );
        let anchor = match t.h_align {
            TextHAlign::Left => "start",
            TextHAlign::Center => "middle",
            TextHAlign::Right => "end",
        };
        // The line box is `TEXT_LINE_HEIGHT_EM` tall; the baseline sits in it.
        let baseline = match t.v_align {
            TextVAlign::Top => em * 0.9,
            TextVAlign::Center => em * 0.35,
            TextVAlign::Bottom => -em * 0.2,
        };
        let turn = if t.rotation.abs() > 1e-6 {
            format!(
                " transform=\"rotate({} {} {})\"",
                num(t.rotation.to_degrees()),
                num(t.position[0]),
                num(t.position[1])
            )
        } else {
            String::new()
        };
        let weight = if t.bold { " font-weight=\"bold\"" } else { "" };
        let style = if t.italic {
            " font-style=\"italic\""
        } else {
            ""
        };
        let family = t
            .family
            .as_deref()
            .unwrap_or(signex_types::font::BUNDLED_DEFAULT_FAMILY);
        let _ = writeln!(
            self.out,
            "<text x=\"{}\" y=\"{}\" font-size=\"{}\" font-family=\"{}, sans-serif\" text-anchor=\"{anchor}\"{weight}{style}{}{turn}>{}</text>",
            num(t.position[0]),
            num(t.position[1] + baseline),
            num(em),
            escape(family),
            paint("fill", t.color),
            escape(&t.content)
        );
    }

    fn ordered(&mut self, items: &[OrderedItem]) {
        for item in items {
            match item {
                OrderedItem::Line(l) => self.line(l),
                OrderedItem::Circle(c) => self.circle(c),
                OrderedItem::Arc(a) => self.arc(a),
                OrderedItem::Polygon(p) => self.polygon(p),
            }
        }
    }
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// `scene`'s base buckets as an SVG document sized to its drawn extent, or
/// `None` when it draws nothing. Overlay and marker buckets are editing aids
/// and are left out.
pub fn scene_to_svg(scene: &Scene, opts: &SceneSvgOptions) -> Option<String> {
    let mut w = Writer {
        out: String::new(),
        extent: Extent::default(),
        opts,
    };
    for bucket in GPU_SCENE_DRAW_ORDER {
        match bucket {
            SceneBucket::Drawings => w.ordered(&scene.drawings),
            SceneBucket::DrawingsFront => w.ordered(&scene.drawings_front),
            SceneBucket::Polygons => scene.polygons.iter().for_each(|p| w.polygon(p)),
            SceneBucket::TopPolygons => scene.top_polygons.iter().for_each(|p| w.polygon(p)),
            SceneBucket::Lines => scene.lines.iter().for_each(|l| w.line(l)),
            SceneBucket::Arcs => scene.arcs.iter().for_each(|a| w.arc(a)),
            SceneBucket::Circles => scene.circles.iter().for_each(|c| w.circle(c)),
            SceneBucket::Texts => scene.texts.iter().for_each(|t| w.text(t)),
            _ => {}
        }
    }
    let [x0, y0, x1, y1] = w.extent.0?;
    let m = opts.margin_mm;
    let (x, y, width, height) = (x0 - m, y0 - m, (x1 - x0) + 2.0 * m, (y1 - y0) + 2.0 * m);
    Some(format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{} {} {} {}\" width=\"{}mm\" height=\"{}mm\">\n<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{}/>\n{}</svg>\n",
        num(x),
        num(y),
        num(width),
        num(height),
        num(width),
        num(height),
        num(x),
        num(y),
        num(width),
        num(height),
        paint("fill", opts.background),
        w.out
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_are_trimmed_and_never_negative_zero() {
        assert_eq!(num(1.5), "1.5");
        assert_eq!(num(2.0), "2");
        assert_eq!(num(-0.00001), "0");
        assert_eq!(num(0.12345), "0.1235");
    }

    #[test]
    fn an_empty_scene_draws_nothing() {
        assert!(scene_to_svg(&Scene::default(), &SceneSvgOptions::default()).is_none());
    }

    #[test]
    fn a_line_and_a_half_turn_arc_are_emitted() {
        let mut scene = Scene::default();
        scene.lines.push(LineSegment {
            p0: [0.0, 0.0],
            p1: [2.0, 0.0],
            width: 0.2,
            color: [1.0, 0.0, 0.0, 1.0],
            style: 0,
            _pad: 0,
        });
        scene.arcs.push(Arc {
            center: [0.0, 0.0],
            radius: 1.0,
            start_angle: 0.0,
            end_angle: std::f32::consts::PI,
            width: 0.1,
            color: [0.0, 0.0, 1.0, 1.0],
            _pad: [0.0; 3],
        });
        let svg = scene_to_svg(&scene, &SceneSvgOptions::default()).unwrap();
        assert!(
            svg.contains("<line x1=\"0\" y1=\"0\" x2=\"2\" y2=\"0\""),
            "{svg}"
        );
        assert!(svg.contains("A1 1 0 0 1 -1 0"), "{svg}");
        assert!(svg.contains("stroke=\"#ff0000\""), "{svg}");
    }
}
