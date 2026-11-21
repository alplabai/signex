//! Board drawings and texts as areas, for the silkscreen and for shapes
//! drawn on a solder mask layer. Same geometry as the flat view's strokes.

use pcb_geom::{
    PointNm, Polygon, TextPlacement, mm_to_nm, nm_to_mm, text_stroke_width_mm, text_strokes,
};

use super::board_xy;
use super::copper::{capsule, circle};
use super::region::polygon;
use crate::pcb::{FootprintTextInput, GraphicInput, GraphicShapeInput, arc_points};

/// The thinnest line drawn, mm: a drawing saved with no width still shows.
const MIN_STROKE_MM: f32 = 0.02;
/// Points round a drawn circle.
const CIRCLE_SEGMENTS: usize = 32;

fn stroke(a: [f32; 2], b: [f32; 2], radius: f32) -> Option<Polygon> {
    polygon(&capsule(board_xy(a), board_xy(b), radius), &[])
}

/// The area `graphic` covers, in the 3D frame.
pub(super) fn graphic_polygons(graphic: &GraphicInput) -> Vec<Polygon> {
    let half = graphic.width_mm.max(MIN_STROKE_MM) * 0.5;
    match &graphic.shape {
        GraphicShapeInput::Line { p0, p1 } => stroke(*p0, *p1, half).into_iter().collect(),
        GraphicShapeInput::Arc {
            center,
            radius_mm,
            start_deg,
            sweep_deg,
        } => arc_points(*center, *radius_mm, *start_deg, *sweep_deg)
            .windows(2)
            .filter_map(|pair| stroke(pair[0], pair[1], half))
            .collect(),
        GraphicShapeInput::Circle { center, radius_mm } => {
            let center = board_xy(*center);
            let outer = circle(center, radius_mm + half, CIRCLE_SEGMENTS);
            let inner = radius_mm - half;
            let holes = if graphic.filled || inner <= 0.0 {
                Vec::new()
            } else {
                vec![circle(center, inner, CIRCLE_SEGMENTS)]
            };
            polygon(&outer, &holes).into_iter().collect()
        }
        GraphicShapeInput::Polygon { points } => {
            let mut out: Vec<Polygon> = (0..points.len())
                .filter(|_| points.len() >= 2)
                .filter_map(|i| stroke(points[i], points[(i + 1) % points.len()], half))
                .collect();
            if graphic.filled {
                let ring: Vec<[f32; 2]> = points.iter().copied().map(board_xy).collect();
                out.extend(polygon(&ring, &[]));
            }
            out
        }
    }
}

/// The strokes of `text` as areas, in the 3D frame; none for a text the
/// geometry refuses.
pub(super) fn text_polygons(text: &FootprintTextInput) -> Vec<Polygon> {
    let (Ok(x), Ok(y)) = (
        mm_to_nm(f64::from(text.position[0]), "text.x"),
        mm_to_nm(f64::from(text.position[1]), "text.y"),
    ) else {
        return Vec::new();
    };
    let at = TextPlacement {
        anchor: PointNm::new(x, y),
        rotation_deg: f64::from(text.rotation_deg),
        mirrored: text.mirrored,
    };
    let size = f64::from(text.size_mm);
    let Ok(strokes) = text_strokes(&text.content, &at, size, text.justify) else {
        return Vec::new();
    };
    let half = (text_stroke_width_mm(size) * 0.5) as f32;
    let mm = |p: &PointNm| [nm_to_mm(p.x) as f32, nm_to_mm(p.y) as f32];
    let mut out = Vec::new();
    for line in &strokes {
        if let [dot] = line.as_slice() {
            out.extend(stroke(mm(dot), mm(dot), half));
        }
        for pair in line.windows(2) {
            out.extend(stroke(mm(&pair[0]), mm(&pair[1]), half));
        }
    }
    out
}
