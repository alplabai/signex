//! The generated footprint features (mockup row 124): a silkscreen outline
//! on the part's body clipped at the pads, a pin 1 dot and an assembly
//! outline. See `docs/FOOTPRINT_SKETCH.md`, "Generated features".

use signex_library::primitive::footprint::{Footprint, FpGraphic, FpGraphicKind, Pad, PadShape};

/// Line width of the generated assembly outline, in mm (the stock parts' fab
/// line).
const ASSEMBLY_LINE_WIDTH_MM: f64 = 0.1;
/// A clipped piece shorter than this is not drawn, in mm.
const MIN_PIECE_MM: f64 = 1e-6;

/// An axis-aligned box, `[min_x, min_y, max_x, max_y]`, in mm.
type Extent = [f64; 4];

/// Rebuild the generated strokes from the footprint's settings, pads and
/// body. A feature that is off, or has no body to follow, makes no strokes.
pub fn bake_generated(fp: &mut Footprint) {
    let silk = silk_strokes(fp);
    let assembly = assembly_strokes(fp);
    fp.generated.silk = silk;
    fp.generated.assembly = assembly;
}

/// The part's body: the 3D body outline, else the extent of the hand-drawn
/// front fab graphics. `None` when neither gives an area.
pub fn body_extent(fp: &Footprint) -> Option<Extent> {
    let outline = fp.body_3d.outline.as_ref().map(|o| o.points.clone());
    let points = outline
        .filter(|points| !points.is_empty())
        .unwrap_or_else(|| {
            fp.fab_f
                .iter()
                .flat_map(|g| graphic_points(&g.kind))
                .collect()
        });
    extent_of(points).filter(|[x0, y0, x1, y1]| x1 > x0 && y1 > y0)
}

fn graphic_points(kind: &FpGraphicKind) -> Vec<[f64; 2]> {
    match kind {
        FpGraphicKind::Line { from, to } | FpGraphicKind::Rectangle { from, to } => {
            vec![*from, *to]
        }
        FpGraphicKind::Circle { center, radius } | FpGraphicKind::Arc { center, radius, .. } => {
            vec![
                [center[0] - radius, center[1] - radius],
                [center[0] + radius, center[1] + radius],
            ]
        }
        FpGraphicKind::Polygon { vertices } => vertices.clone(),
        FpGraphicKind::Text { .. } => Vec::new(),
    }
}

fn extent_of(points: impl IntoIterator<Item = [f64; 2]>) -> Option<Extent> {
    points.into_iter().fold(None, |extent, [x, y]| {
        Some(match extent {
            Some([x0, y0, x1, y1]) => [x0.min(x), y0.min(y), x1.max(x), y1.max(y)],
            None => [x, y, x, y],
        })
    })
}

fn assembly_strokes(fp: &Footprint) -> Vec<FpGraphic> {
    if !fp.generated.assembly_outline {
        return Vec::new();
    }
    let Some([x0, y0, x1, y1]) = body_extent(fp) else {
        return Vec::new();
    };
    [
        ([x0, y0], [x1, y0]),
        ([x1, y0], [x1, y1]),
        ([x1, y1], [x0, y1]),
        ([x0, y1], [x0, y0]),
    ]
    .into_iter()
    .map(|(from, to)| line(from, to, ASSEMBLY_LINE_WIDTH_MM))
    .collect()
}

fn line(from: [f64; 2], to: [f64; 2], width: f64) -> FpGraphic {
    FpGraphic::drawn(FpGraphicKind::Line { from, to }, width, false)
}

fn silk_strokes(fp: &Footprint) -> Vec<FpGraphic> {
    let settings = &fp.generated;
    let (width, clearance) = (settings.silk_line_width_mm, settings.pad_clearance_mm);
    if !settings.silk_outline || width <= 0.0 || clearance < 0.0 {
        return Vec::new();
    }
    let mut strokes = body_extent(fp).map_or_else(Vec::new, |body| {
        clipped_outline(body, &fp.pads, clearance + width / 2.0, width)
    });
    if settings.pin_one_marker {
        strokes.extend(pin_one_dot(&fp.pads, clearance, width));
    }
    strokes
}

/// The body rectangle's edges with the part inside each pad's keep-away box
/// removed; `keep_away` is measured from the stroke's centre line.
fn clipped_outline(body: Extent, pads: &[Pad], keep_away: f64, width: f64) -> Vec<FpGraphic> {
    let [x0, y0, x1, y1] = body;
    let boxes: Vec<Extent> = pads
        .iter()
        .map(|pad| grown(pad_extent(pad), keep_away))
        .collect();
    let mut strokes = Vec::new();
    for y in [y0, y1] {
        let blocked = boxes
            .iter()
            .filter(|b| b[1] < y && y < b[3])
            .map(|b| (b[0], b[2]));
        for (from, to) in open_spans(x0, x1, blocked) {
            strokes.push(line([from, y], [to, y], width));
        }
    }
    for x in [x0, x1] {
        let blocked = boxes
            .iter()
            .filter(|b| b[0] < x && x < b[2])
            .map(|b| (b[1], b[3]));
        for (from, to) in open_spans(y0, y1, blocked) {
            strokes.push(line([x, from], [x, to], width));
        }
    }
    strokes
}

fn grown([x0, y0, x1, y1]: Extent, by: f64) -> Extent {
    [x0 - by, y0 - by, x1 + by, y1 + by]
}

/// What is left of `start..end` after removing the `blocked` spans.
fn open_spans(start: f64, end: f64, blocked: impl Iterator<Item = (f64, f64)>) -> Vec<(f64, f64)> {
    let mut blocked: Vec<(f64, f64)> = blocked.collect();
    blocked.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut spans = Vec::new();
    let mut cursor = start;
    for (from, to) in blocked {
        if from.min(end) - cursor > MIN_PIECE_MM {
            spans.push((cursor, from.min(end)));
        }
        cursor = cursor.max(to);
    }
    if end - cursor > MIN_PIECE_MM {
        spans.push((cursor, end));
    }
    spans
}

/// The copper's extent, turned with the pad and moved by its copper offset.
fn pad_extent(pad: &Pad) -> Extent {
    let (sin, cos) = pad.rotation.to_radians().sin_cos();
    let turn = |[x, y]: [f64; 2]| [x * cos - y * sin, x * sin + y * cos];
    let offset = turn([
        pad.copper_offset_x_mm.unwrap_or(0.0),
        pad.copper_offset_y_mm.unwrap_or(0.0),
    ]);
    let (hx, hy) = (pad.size[0] / 2.0, pad.size[1] / 2.0);
    let mut corners = vec![[hx, hy], [-hx, hy], [-hx, -hy], [hx, -hy]];
    if let PadShape::Custom(outline) = &pad.shape {
        corners.extend(outline.points.iter().copied());
    }
    let centre = [pad.position[0] + offset[0], pad.position[1] + offset[1]];
    let placed = corners.into_iter().map(|corner| {
        let [x, y] = turn(corner);
        [centre[0] + x, centre[1] + y]
    });
    extent_of(placed).unwrap_or([centre[0], centre[1], centre[0], centre[1]])
}

/// A solid dot on the outer side of pad 1, `clearance` from its copper.
fn pin_one_dot(pads: &[Pad], clearance: f64, width: f64) -> Option<FpGraphic> {
    let pad = pads.iter().find(|pad| pad.number == "1")?;
    let [px0, py0, px1, py1] = pad_extent(pad);
    let all = extent_of(pads.iter().flat_map(|p| {
        let [x0, y0, x1, y1] = pad_extent(p);
        [[x0, y0], [x1, y1]]
    }))?;
    let (dx, dy) = (
        (px0 + px1 - all[0] - all[2]) / 2.0,
        (py0 + py1 - all[1] - all[3]) / 2.0,
    );
    // The dot's outer edge is one line width from its centre.
    let gap = clearance + width;
    let center = if dx.abs() >= dy.abs() {
        let x = if dx > 0.0 { px1 + gap } else { px0 - gap };
        [x, (py0 + py1) / 2.0]
    } else {
        let y = if dy > 0.0 { py1 + gap } else { py0 - gap };
        [(px0 + px1) / 2.0, y]
    };
    let circle = FpGraphicKind::Circle {
        center,
        radius: width / 2.0,
    };
    Some(FpGraphic::drawn(circle, width, true))
}

#[cfg(test)]
#[path = "generated_tests.rs"]
mod tests;
