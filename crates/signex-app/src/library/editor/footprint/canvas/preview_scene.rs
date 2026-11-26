//! A saved footprint as a world-space [`Scene`], for headless export (#3116).
//!
//! The pads are the PCB canvas's own lowering (`PreviewFootprint::board_pads`,
//! the scene the preview already draws). Silk, fab and courtyard are the
//! graphics `draw_silk_graphics` and `draw_courtyard` stroke onto the editor's
//! canvas, in the same board-palette colours, lowered to scene primitives
//! because a canvas frame cannot be replayed without a window.

use iced::Color;
use signex_gfx::primitive::arc::Arc;
use signex_gfx::primitive::circle::Circle;
use signex_gfx::primitive::line::LineSegment;
use signex_gfx::primitive::polygon::GpuPolygon;
use signex_gfx::primitive::text::{TextHAlign, TextItem, TextVAlign};
use signex_gfx::scene::Scene;
use signex_library::primitive::footprint::{FpGraphic, FpGraphicKind};

use super::PreviewFootprint;
use crate::library::editor::footprint::layers::FpLayer;

/// Stroke of a graphic that stores none (`stroke_width == 0`), mm.
const DEFAULT_STROKE_MM: f32 = 0.1;
/// Stroke of the courtyard outline, mm.
const COURTYARD_STROKE_MM: f32 = 0.05;
/// Pad number ink, as the editor's `draw_pad_number`.
const PAD_NUMBER_INK: [f32; 4] = [0.05, 0.05, 0.05, 1.0];

fn rgba(c: Color) -> [f32; 4] {
    [c.r, c.g, c.b, c.a]
}

fn p(v: [f64; 2]) -> [f32; 2] {
    [v[0] as f32, v[1] as f32]
}

fn segment(a: [f32; 2], b: [f32; 2], width: f32, color: [f32; 4]) -> LineSegment {
    LineSegment {
        p0: a,
        p1: b,
        width,
        color,
        style: 0,
        _pad: 0,
    }
}

/// Closed outline of `points` as line segments.
fn outline(scene: &mut Scene, points: &[[f32; 2]], width: f32, color: [f32; 4]) {
    for (i, a) in points.iter().enumerate() {
        scene
            .lines
            .push(segment(*a, points[(i + 1) % points.len()], width, color));
    }
}

fn text(content: &str, at: [f32; 2], em_mm: f32, color: [f32; 4]) -> TextItem {
    TextItem {
        content: content.to_string(),
        position: at,
        // `size_mm` is a cap height; the canvas draws the stored size as the em.
        size_mm: em_mm * signex_types::font::mm_per_em(None),
        color,
        bold: false,
        italic: false,
        family: None,
        rotation: 0.0,
        h_align: TextHAlign::Left,
        v_align: TextVAlign::Top,
    }
}

fn graphics(scene: &mut Scene, items: &[FpGraphic], layer: FpLayer) {
    let base = rgba(layer.color());
    // Shapes first, text last, as the canvas orders them.
    for text_pass in [false, true] {
        for g in items {
            if matches!(g.kind, FpGraphicKind::Text { .. }) != text_pass {
                continue;
            }
            let width = if g.stroke_width > 0.0 {
                g.stroke_width as f32
            } else {
                DEFAULT_STROKE_MM
            };
            match &g.kind {
                FpGraphicKind::Line { from, to } => {
                    scene.lines.push(segment(p(*from), p(*to), width, base));
                }
                FpGraphicKind::Rectangle { from, to } => {
                    let (a, b) = (p(*from), p(*to));
                    let corners = [a, [b[0], a[1]], b, [a[0], b[1]]];
                    outline(scene, &corners, width, base);
                }
                FpGraphicKind::Circle { center, radius } => scene.circles.push(Circle {
                    center: p(*center),
                    radius: *radius as f32,
                    stroke_width: width,
                    color: base,
                }),
                FpGraphicKind::Arc {
                    center,
                    radius,
                    start_deg,
                    end_deg,
                } => {
                    let start = (*start_deg as f32).to_radians();
                    let sweep = FpGraphicKind::arc_sweep_deg(*start_deg, *end_deg) as f32;
                    scene.arcs.push(Arc {
                        center: p(*center),
                        radius: *radius as f32,
                        start_angle: start,
                        end_angle: start + sweep.to_radians(),
                        width,
                        color: base,
                        _pad: [0.0; 3],
                    });
                }
                FpGraphicKind::Polygon { vertices } if vertices.len() >= 2 => {
                    let points: Vec<[f32; 2]> = vertices.iter().map(|v| p(*v)).collect();
                    if g.filled {
                        scene.polygons.push(GpuPolygon {
                            vertices: points.clone(),
                            holes: Vec::new(),
                            fill_color: [base[0], base[1], base[2], 0.55],
                            stroke_color: None,
                            stroke_width: 0.0,
                        });
                    }
                    outline(scene, &points, width, base);
                }
                FpGraphicKind::Polygon { .. } => {}
                FpGraphicKind::Text {
                    position,
                    content,
                    size,
                    frame,
                } => {
                    let at = p(*position);
                    let mut em = *size as f32;
                    if let Some((w, h)) = frame {
                        let far = [at[0] + *w, at[1] + *h];
                        let corners = [at, [far[0], at[1]], far, [at[0], far[1]]];
                        outline(scene, &corners, width, base);
                        em = em.min(h.abs());
                    }
                    scene.texts.push(text(content, at, em, base));
                }
            }
        }
    }
}

impl PreviewFootprint {
    /// Everything the Components-panel preview draws, front and back, as one
    /// scene: fab, silk, courtyard, pads, then the pad numbers.
    pub fn scene(&self) -> Scene {
        let mut scene = self.board_pads().clone();
        let input = self.layer_input();
        graphics(&mut scene, self.fab_b(), FpLayer::BFab);
        graphics(&mut scene, input.silk_b, FpLayer::BSilks);
        graphics(&mut scene, self.fab_f(), FpLayer::FFab);
        graphics(&mut scene, input.silk_f, FpLayer::FSilks);

        let courtyard = rgba(super::super::layers::LayerPalette::Board.courtyard_colour());
        if let Some(points) = input.courtyard_outline.filter(|o| o.len() >= 3) {
            let points: Vec<[f32; 2]> = points.iter().map(|v| p([v.0, v.1])).collect();
            outline(&mut scene, &points, COURTYARD_STROKE_MM, courtyard);
        } else if let Some(c) = input.courtyard_rect {
            let (a, b) = (
                [c.min_x as f32, c.min_y as f32],
                [c.max_x as f32, c.max_y as f32],
            );
            let corners = [a, [b[0], a[1]], b, [a[0], b[1]]];
            outline(&mut scene, &corners, COURTYARD_STROKE_MM, courtyard);
        }

        for pad in self.pads().iter().filter(|pad| !pad.number.is_empty()) {
            let em = (pad.size_mm.0.min(pad.size_mm.1) as f32 * 0.5).clamp(0.3, 1.0);
            let mut label = text(
                &pad.number,
                [pad.position_mm.0 as f32, pad.position_mm.1 as f32],
                em,
                PAD_NUMBER_INK,
            );
            label.h_align = TextHAlign::Center;
            label.v_align = TextVAlign::Center;
            scene.texts.push(label);
        }
        scene
    }
}
