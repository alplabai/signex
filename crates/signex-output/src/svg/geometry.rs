//! Low-level SVG path/shape primitives.
//!
//! Rectangle, circle, and three-point-arc path builders plus the
//! `tiny_skia` path conversion used by the rasterizer. These are the
//! geometry helpers the per-element emitters share.
//!
//! Extracted verbatim from the SVG exporter (`svg/mod.rs`); pure code
//! motion, zero behaviour change.

use super::*;
use tiny_skia::PathBuilder;

pub(super) fn path_to_tiny_skia(commands: &[SvgPathCommand]) -> Option<tiny_skia::Path> {
    let mut pb = PathBuilder::new();
    for c in commands {
        match c {
            SvgPathCommand::MoveTo(p) => pb.move_to(p.x, p.y),
            SvgPathCommand::LineTo(p) => pb.line_to(p.x, p.y),
            SvgPathCommand::CubicTo(c1, c2, p) => pb.cubic_to(c1.x, c1.y, c2.x, c2.y, p.x, p.y),
            SvgPathCommand::Close => pb.close(),
        }
    }
    pb.finish()
}

pub(super) fn rect_path(x: f32, y: f32, w: f32, h: f32, style: SvgStyle) -> SvgElement {
    SvgElement::Path {
        commands: vec![
            SvgPathCommand::MoveTo(pt(x, y)),
            SvgPathCommand::LineTo(pt(x + w, y)),
            SvgPathCommand::LineTo(pt(x + w, y + h)),
            SvgPathCommand::LineTo(pt(x, y + h)),
            SvgPathCommand::Close,
        ],
        style,
    }
}

pub(super) fn circle_path(cx: f32, cy: f32, r: f32, style: SvgStyle) -> SvgElement {
    let k = 0.552_284_8_f32 * r;
    SvgElement::Path {
        commands: vec![
            SvgPathCommand::MoveTo(pt(cx + r, cy)),
            SvgPathCommand::CubicTo(pt(cx + r, cy + k), pt(cx + k, cy + r), pt(cx, cy + r)),
            SvgPathCommand::CubicTo(pt(cx - k, cy + r), pt(cx - r, cy + k), pt(cx - r, cy)),
            SvgPathCommand::CubicTo(pt(cx - r, cy - k), pt(cx - k, cy - r), pt(cx, cy - r)),
            SvgPathCommand::CubicTo(pt(cx + k, cy - r), pt(cx + r, cy - k), pt(cx + r, cy)),
            SvgPathCommand::Close,
        ],
        style,
    }
}

pub(super) fn arc_path_commands(start: SvgPoint, mid: SvgPoint, end: SvgPoint) -> Vec<SvgPathCommand> {
    if let Some((cx, cy, r)) = circle_from_three_points(start, mid, end) {
        let start_a = (start.y - cy).atan2(start.x - cx) as f64;
        let mid_a = (mid.y - cy).atan2(mid.x - cx) as f64;
        let end_a = (end.y - cy).atan2(end.x - cx) as f64;
        let (from, to) = arc_sweep(start_a, mid_a, end_a);
        let sweep = to - from;
        let seg_count = ((sweep.abs() / (std::f64::consts::FRAC_PI_2)).ceil() as usize).max(1);
        let step = sweep / seg_count as f64;

        let mut cmds = Vec::with_capacity(seg_count + 1);
        cmds.push(SvgPathCommand::MoveTo(start));

        for i in 0..seg_count {
            let a0 = from + step * i as f64;
            let a1 = from + step * (i + 1) as f64;
            let k = (4.0 / 3.0) * ((a1 - a0) / 4.0).tan();

            let p0 = (
                cx as f64 + r as f64 * a0.cos(),
                cy as f64 + r as f64 * a0.sin(),
            );
            let p3 = (
                cx as f64 + r as f64 * a1.cos(),
                cy as f64 + r as f64 * a1.sin(),
            );
            let c1 = (
                p0.0 - k * r as f64 * a0.sin(),
                p0.1 + k * r as f64 * a0.cos(),
            );
            let c2 = (
                p3.0 + k * r as f64 * -a1.sin(),
                p3.1 + k * r as f64 * a1.cos(),
            );

            cmds.push(SvgPathCommand::CubicTo(
                pt(c1.0 as f32, c1.1 as f32),
                pt(c2.0 as f32, c2.1 as f32),
                pt(p3.0 as f32, p3.1 as f32),
            ));
        }

        cmds
    } else {
        vec![SvgPathCommand::MoveTo(start), SvgPathCommand::LineTo(end)]
    }
}

fn circle_from_three_points(a: SvgPoint, b: SvgPoint, c: SvgPoint) -> Option<(f32, f32, f32)> {
    let (ax, ay) = (a.x as f64, a.y as f64);
    let (bx, by) = (b.x as f64, b.y as f64);
    let (cx, cy) = (c.x as f64, c.y as f64);

    let d = 2.0 * (ax * (by - cy) + bx * (cy - ay) + cx * (ay - by));
    if d.abs() < 1e-12 {
        return None;
    }

    let ux = ((ax * ax + ay * ay) * (by - cy)
        + (bx * bx + by * by) * (cy - ay)
        + (cx * cx + cy * cy) * (ay - by))
        / d;
    let uy = ((ax * ax + ay * ay) * (cx - bx)
        + (bx * bx + by * by) * (ax - cx)
        + (cx * cx + cy * cy) * (bx - ax))
        / d;
    let r = ((ax - ux) * (ax - ux) + (ay - uy) * (ay - uy)).sqrt();

    Some((ux as f32, uy as f32, r as f32))
}

fn arc_sweep(start: f64, mid: f64, end: f64) -> (f64, f64) {
    use std::f64::consts::TAU;

    let norm = |a: f64| -> f64 {
        let mut t = a % TAU;
        if t < 0.0 {
            t += TAU;
        }
        t
    };

    let ccw_dist = |a: f64, b: f64| -> f64 {
        let d = b - a;
        if d < 0.0 { d + TAU } else { d }
    };

    let s = norm(start);
    let m = norm(mid);
    let e = norm(end);

    let s_to_m = ccw_dist(s, m);
    let s_to_e = ccw_dist(s, e);
    if s_to_m <= s_to_e {
        (start, start + s_to_e)
    } else {
        (start, start - (TAU - s_to_e))
    }
}
