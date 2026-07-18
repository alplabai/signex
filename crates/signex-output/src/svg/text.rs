//! Text rasterization — glyph outlining, markup runs, and fonts.
//!
//! Rasterizes an `SvgElement::Text` into `tiny_skia` glyph paths:
//! markup-run splitting (sub/superscript, overbar), advance measuring,
//! the embedded font faces, and the rotation-aware outline builder.
//!
//! Extracted verbatim from the SVG exporter (`svg/mod.rs`); pure code
//! motion, zero behaviour change.

use super::*;
use signex_types::markup::{RichSegment, parse_signex_markup};
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Stroke};
use ttf_parser::{Face, GlyphId, OutlineBuilder};

pub(super) fn draw_text_outline(
    pixmap: &mut Pixmap,
    x: f32,
    y: f32,
    size_pt: f32,
    align: SvgTextAlign,
    v_align: SvgTextVAlign,
    rotation_deg: f32,
    fill_rgb: (f32, f32, f32),
    font_alias: &str,
    text: &str,
) {
    let Some(face) = face_for_alias(font_alias) else {
        return;
    };
    let units_per_em = face.units_per_em() as f32;
    if units_per_em <= 0.0 {
        return;
    }
    let runs = markup_runs(text);
    let base_size = size_pt.max(1.0);
    let advance = measure_text_advance_runs(&face, &runs, base_size, units_per_em);

    let start_x = match align {
        SvgTextAlign::Left => x,
        SvgTextAlign::Center => x - advance * 0.5,
        SvgTextAlign::Right => x - advance,
    };

    let base_scale = base_size / units_per_em;
    let asc = face.ascender() as f32 * base_scale;
    let desc = face.descender() as f32 * base_scale;
    let baseline_y = match v_align {
        SvgTextVAlign::Top => y + asc,
        SvgTextVAlign::Center => y + (asc + desc) * 0.5,
        SvgTextVAlign::Bottom => y + desc,
    };

    let mut pen_x = start_x;
    let mut paint = Paint::default();
    paint.set_color(rgb_to_color(fill_rgb.0, fill_rgb.1, fill_rgb.2));

    for run in &runs {
        let run_start = pen_x;
        let run_size = base_size * run.scale;
        let run_scale = run_size / units_per_em;
        let run_baseline = baseline_y + base_size * run.baseline_offset;

        for ch in run.text.chars() {
            if ch == '\n' || ch == '\r' {
                continue;
            }

            if let Some(gid) = face.glyph_index(ch) {
                let mut builder =
                    TinyPathOutlineBuilder::new(pen_x, run_baseline, run_scale, x, y, rotation_deg);
                if face.outline_glyph(gid, &mut builder).is_some()
                    && let Some(path) = builder.finish()
                {
                    pixmap.fill_path(&path, &paint, FillRule::Winding, Default::default(), None);
                }
                pen_x += glyph_advance(&face, gid, run_scale);
            } else {
                pen_x += run_size * 0.5;
            }
        }

        if run.overbar && pen_x > run_start {
            let overbar_y = run_baseline - run_size * 0.78;
            let (x1, y1) = rotate_about(run_start, overbar_y, x, y, rotation_deg);
            let (x2, y2) = rotate_about(pen_x, overbar_y, x, y, rotation_deg);
            let mut pb = PathBuilder::new();
            pb.move_to(x1, y1);
            pb.line_to(x2, y2);
            if let Some(path) = pb.finish() {
                let stroke = Stroke {
                    width: (run_size * 0.08).max(0.5),
                    ..Stroke::default()
                };
                pixmap.stroke_path(&path, &paint, &stroke, Default::default(), None);
            }
        }
    }
}

fn measure_text_advance_runs(
    face: &Face<'_>,
    runs: &[MarkupRun],
    base_size: f32,
    units_per_em: f32,
) -> f32 {
    let mut advance = 0.0_f32;
    for run in runs {
        let run_scale = (base_size * run.scale) / units_per_em;
        for ch in run.text.chars() {
            if ch == '\n' || ch == '\r' {
                continue;
            }
            if let Some(gid) = face.glyph_index(ch) {
                advance += glyph_advance(face, gid, run_scale);
            } else {
                advance += 0.5 * run_scale * face.units_per_em() as f32;
            }
        }
    }
    advance
}

#[derive(Clone)]
struct MarkupRun {
    text: String,
    scale: f32,
    baseline_offset: f32,
    overbar: bool,
}

fn markup_runs(input: &str) -> Vec<MarkupRun> {
    let expanded = normalize_standard_text(input);
    let segments = parse_signex_markup(&expanded);
    if segments.is_empty() {
        return vec![MarkupRun {
            text: expanded,
            scale: 1.0,
            baseline_offset: 0.0,
            overbar: false,
        }];
    }

    segments
        .into_iter()
        .map(|seg| match seg {
            RichSegment::Normal(t) => MarkupRun {
                text: t,
                scale: 1.0,
                baseline_offset: 0.0,
                overbar: false,
            },
            // TODO(v0.x): visual decoration for bold/italic/strike
            RichSegment::Bold(t) | RichSegment::Italic(t) | RichSegment::Strike(t) => MarkupRun {
                text: t,
                scale: 1.0,
                baseline_offset: 0.0,
                overbar: false,
            },
            RichSegment::Overbar(t) => MarkupRun {
                text: t,
                scale: 1.0,
                baseline_offset: 0.0,
                overbar: true,
            },
            RichSegment::Subscript(t) => MarkupRun {
                text: t,
                scale: 0.72,
                baseline_offset: 0.26,
                overbar: false,
            },
            RichSegment::Superscript(t) => MarkupRun {
                text: t,
                scale: 0.72,
                baseline_offset: -0.34,
                overbar: false,
            },
            // Links render as plain label text on the canvas — URL is ignored
            // until link rendering ships in a later phase.
            RichSegment::Link { label, .. } => MarkupRun {
                text: label,
                scale: 1.0,
                baseline_offset: 0.0,
                overbar: false,
            },
        })
        .filter(|run| !run.text.is_empty())
        .collect()
}

fn rotate_about(px: f32, py: f32, ox: f32, oy: f32, rotation_deg: f32) -> (f32, f32) {
    let rad = rotation_deg.to_radians();
    let cos = rad.cos();
    let sin = rad.sin();
    let dx = px - ox;
    let dy = py - oy;
    (ox + dx * cos - dy * sin, oy + dx * sin + dy * cos)
}

fn glyph_advance(face: &Face<'_>, gid: GlyphId, scale: f32) -> f32 {
    face.glyph_hor_advance(gid)
        .map(|v| v as f32 * scale)
        .unwrap_or(0.5 * face.units_per_em() as f32 * scale)
}

fn face_for_alias(alias: &str) -> Option<Face<'static>> {
    match alias {
        "F1" => Face::parse(
            include_bytes!("../../../signex-app/assets/fonts/Roboto-Regular.ttf"),
            0,
        )
        .ok(),
        "F2" => Face::parse(
            include_bytes!("../../../signex-app/assets/fonts/Roboto-Bold.ttf"),
            0,
        )
        .ok(),
        "F3" => Face::parse(
            include_bytes!("../../../signex-app/assets/fonts/Iosevka-Regular.ttf"),
            0,
        )
        .ok(),
        "F4" => Face::parse(
            include_bytes!("../../../signex-app/assets/fonts/Iosevka-Bold.ttf"),
            0,
        )
        .ok(),
        _ => None,
    }
}

struct TinyPathOutlineBuilder {
    pb: PathBuilder,
    pen_x: f32,
    baseline_y: f32,
    scale: f32,
    anchor_x: f32,
    anchor_y: f32,
    rot_cos: f32,
    rot_sin: f32,
}

impl TinyPathOutlineBuilder {
    fn new(
        pen_x: f32,
        baseline_y: f32,
        scale: f32,
        anchor_x: f32,
        anchor_y: f32,
        rotation_deg: f32,
    ) -> Self {
        let rad = rotation_deg.to_radians();
        Self {
            pb: PathBuilder::new(),
            pen_x,
            baseline_y,
            scale,
            anchor_x,
            anchor_y,
            rot_cos: rad.cos(),
            rot_sin: rad.sin(),
        }
    }

    fn finish(self) -> Option<tiny_skia::Path> {
        self.pb.finish()
    }

    fn map_point(&self, x: f32, y: f32) -> (f32, f32) {
        let px = self.pen_x + x * self.scale;
        let py = self.baseline_y - y * self.scale;
        let dx = px - self.anchor_x;
        let dy = py - self.anchor_y;
        let rx = self.anchor_x + dx * self.rot_cos - dy * self.rot_sin;
        let ry = self.anchor_y + dx * self.rot_sin + dy * self.rot_cos;
        (rx, ry)
    }
}

impl OutlineBuilder for TinyPathOutlineBuilder {
    fn move_to(&mut self, x: f32, y: f32) {
        let (mx, my) = self.map_point(x, y);
        self.pb.move_to(mx, my);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let (lx, ly) = self.map_point(x, y);
        self.pb.line_to(lx, ly);
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (cx, cy) = self.map_point(x1, y1);
        let (px, py) = self.map_point(x, y);
        self.pb.quad_to(cx, cy, px, py);
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (c1x, c1y) = self.map_point(x1, y1);
        let (c2x, c2y) = self.map_point(x2, y2);
        let (px, py) = self.map_point(x, y);
        self.pb.cubic_to(c1x, c1y, c2x, c2y, px, py);
    }

    fn close(&mut self) {
        self.pb.close();
    }
}
