//! The line art of a project preview, built from the model. Pure: no IO,
//! no iced; `update` calls it after a save.
//!
//! A board gives its outline, its top and bottom copper (tracks, pads, vias)
//! and its silkscreen; a sheet gives its wires, symbol bodies and power
//! ports. The art is capped at [`MAX_SEGMENTS`] line segments, dropping the
//! smallest primitives first, then laid out in a view box whose longer side
//! is [`VIEW_BOX_SIDE`].

use pcb_geom::{
    GraphicShape, PointNm, Shape, board_graphic_placement, footprint_graphic_placement, layer_bit,
    nm_to_mm, pad_layers, pad_shape,
};
use signex_renderer::pcb::BOARD_OUTLINE;
use signex_types::pcb::{Footprint, Pad, PcbBoard};
use signex_types::pcb_layer::{CopperLayer, PcbLayer, TechnicalLayer};
use signex_types::schematic::{
    Point, SchematicDefaults, SchematicSheet, SymbolPrimitive, arc_polyline, lower_placed_symbol,
};

use super::{Geometry, Ink, Primitive, Thumbnail, ThumbnailKind, VIEW_BOX_SIDE};

/// Most line segments a thumbnail keeps; a circle counts as
/// [`CIRCLE_COST`]. A few thousand keep a big board cheap to draw at tile
/// size.
pub const MAX_SEGMENTS: usize = 3000;

/// Segments a circle counts as against [`MAX_SEGMENTS`].
pub const CIRCLE_COST: usize = 8;

/// Largest turn, degrees, of one chord of a sampled board arc.
const ARC_CHORD_DEG: f64 = 15.0;

/// Shape of a primitive in world mm, before the view box.
#[derive(Debug, Clone)]
enum WorldGeometry {
    Polyline { points: Vec<[f64; 2]>, closed: bool },
    Circle { center: [f64; 2], radius: f64 },
}

#[derive(Debug, Clone)]
struct WorldPrimitive {
    ink: Ink,
    filled: bool,
    geometry: WorldGeometry,
}

impl WorldPrimitive {
    fn line(ink: Ink, a: [f64; 2], b: [f64; 2]) -> Self {
        Self::polyline(ink, vec![a, b], false)
    }

    fn polyline(ink: Ink, points: Vec<[f64; 2]>, closed: bool) -> Self {
        Self {
            ink,
            filled: false,
            geometry: WorldGeometry::Polyline { points, closed },
        }
    }

    fn circle(ink: Ink, center: [f64; 2], radius: f64) -> Self {
        Self {
            ink,
            filled: false,
            geometry: WorldGeometry::Circle { center, radius },
        }
    }

    fn filled(mut self) -> Self {
        self.filled = true;
        self
    }

    /// Line segments this primitive costs against the cap.
    fn cost(&self) -> usize {
        match &self.geometry {
            WorldGeometry::Polyline { points, closed } => {
                points.len().saturating_sub(1) + usize::from(*closed && points.len() > 2)
            }
            WorldGeometry::Circle { .. } => CIRCLE_COST,
        }
    }

    /// `[min_x, min_y, max_x, max_y]`.
    fn bounds(&self) -> Option<[f64; 4]> {
        match &self.geometry {
            WorldGeometry::Polyline { points, .. } => points_bounds(points),
            WorldGeometry::Circle { center, radius } => Some([
                center[0] - radius,
                center[1] - radius,
                center[0] + radius,
                center[1] + radius,
            ]),
        }
    }

    /// The diagonal of its box: what "smallest" means for the cap.
    fn extent(&self) -> f64 {
        self.bounds()
            .map(|[x0, y0, x1, y1]| (x1 - x0).hypot(y1 - y0))
            .unwrap_or(0.0)
    }

    fn is_drawable(&self) -> bool {
        match &self.geometry {
            WorldGeometry::Polyline { points, .. } => {
                points.len() >= 2 && points.iter().flatten().all(|v| v.is_finite())
            }
            WorldGeometry::Circle { center, radius } => {
                center.iter().all(|v| v.is_finite()) && radius.is_finite() && *radius > 0.0
            }
        }
    }
}

fn xy(point: Point) -> [f64; 2] {
    [point.x, point.y]
}

fn nm_xy(point: PointNm) -> [f64; 2] {
    [nm_to_mm(point.x), nm_to_mm(point.y)]
}

fn points_bounds(points: &[[f64; 2]]) -> Option<[f64; 4]> {
    points.iter().fold(None, |acc, [x, y]| {
        Some(match acc {
            None => [*x, *y, *x, *y],
            Some([x0, y0, x1, y1]) => [x0.min(*x), y0.min(*y), x1.max(*x), y1.max(*y)],
        })
    })
}

// ─── Board ────────────────────────────────────────────────────────

/// The preview of `board`.
pub fn board(board: &PcbBoard) -> Thumbnail {
    let mut art = Vec::new();
    if board.outline.len() >= 3 {
        let ring = board.outline.iter().copied().map(xy).collect();
        art.push(WorldPrimitive::polyline(Ink::BoardOutline, ring, true));
    }
    for segment in &board.segments {
        if let Some(ink) = segment.pcb_layer().and_then(copper_ink) {
            art.push(WorldPrimitive::line(
                ink,
                xy(segment.start),
                xy(segment.end),
            ));
        }
    }
    for via in &board.vias {
        art.push(
            WorldPrimitive::circle(Ink::MultiLayer, xy(via.position), via.diameter / 2.0).filled(),
        );
    }
    for footprint in &board.footprints {
        footprint_art(footprint, &mut art);
    }
    for graphic in &board.graphics {
        let Some(ink) = graphic.pcb_layer().and_then(drawing_ink) else {
            continue;
        };
        if let Ok(Some(placed)) = board_graphic_placement(graphic) {
            push_placed(&mut art, ink, &placed.shape);
        }
    }
    finish(ThumbnailKind::Board, art)
}

fn footprint_art(footprint: &Footprint, art: &mut Vec<WorldPrimitive>) {
    art.extend(
        footprint
            .pads
            .iter()
            .filter_map(|pad| pad_art(footprint, pad)),
    );
    for graphic in &footprint.graphics {
        let Some(ink) = graphic.pcb_layer().and_then(drawing_ink) else {
            continue;
        };
        if let Ok(Some(placed)) = footprint_graphic_placement(footprint, graphic) {
            push_placed(art, ink, &placed.shape);
        }
    }
}

/// The ink of copper on `layer`: top or bottom; inner copper is not drawn.
fn copper_ink(layer: PcbLayer) -> Option<Ink> {
    match layer {
        PcbLayer::Copper(CopperLayer::Top) => Some(Ink::TopCopper),
        PcbLayer::Copper(CopperLayer::Bottom) => Some(Ink::BottomCopper),
        _ => None,
    }
}

/// The ink of a drawing on `layer`: the board edge, silkscreen, or none.
fn drawing_ink(layer: PcbLayer) -> Option<Ink> {
    match layer {
        _ if layer == BOARD_OUTLINE => Some(Ink::BoardOutline),
        PcbLayer::Technical(TechnicalLayer::SilkscreenTop) => Some(Ink::TopSilkscreen),
        PcbLayer::Technical(TechnicalLayer::SilkscreenBottom) => Some(Ink::BottomSilkscreen),
        _ => None,
    }
}

/// A pad as a filled area: on both faces (through-hole) it is Multi-Layer,
/// else the face it is on. A pad only on inner copper is not drawn.
fn pad_art(footprint: &Footprint, pad: &Pad) -> Option<WorldPrimitive> {
    let mask = pad_layers(footprint, pad);
    let top = mask & (1 << layer_bit(CopperLayer::Top)) != 0;
    let bottom = mask & (1 << layer_bit(CopperLayer::Bottom)) != 0;
    let ink = match (top, bottom) {
        (true, true) => Ink::MultiLayer,
        (true, false) => Ink::TopCopper,
        (false, true) => Ink::BottomCopper,
        (false, false) => return None,
    };
    let shape = pad_shape(footprint, pad).ok()?;
    Some(WorldPrimitive {
        ink,
        filled: true,
        geometry: shape_outline(&shape),
    })
}

/// A copper shape's outline: a disc stays a circle, a capsule and a
/// (round-)rectangle become their corner box, a polygon its ring.
fn shape_outline(shape: &Shape) -> WorldGeometry {
    match shape {
        Shape::Disc { center, radius } => WorldGeometry::Circle {
            center: nm_xy(*center),
            radius: nm_to_mm(*radius),
        },
        Shape::Capsule { a, b, radius } => capsule_box(nm_xy(*a), nm_xy(*b), nm_to_mm(*radius)),
        Shape::Rect {
            center,
            half_w,
            half_h,
            rotation_deg,
        }
        | Shape::RoundRect {
            center,
            half_w,
            half_h,
            rotation_deg,
            ..
        } => turned_box(
            nm_xy(*center),
            nm_to_mm(*half_w),
            nm_to_mm(*half_h),
            *rotation_deg,
        ),
        Shape::Polygon { ring, .. } => WorldGeometry::Polyline {
            points: ring.iter().copied().map(nm_xy).collect(),
            closed: true,
        },
    }
}

/// The rectangle `2 half_w` by `2 half_h` about `center`, turned
/// `rotation_deg` counter-clockwise in the model's axes.
fn turned_box(center: [f64; 2], half_w: f64, half_h: f64, rotation_deg: f64) -> WorldGeometry {
    let (sin, cos) = rotation_deg.to_radians().sin_cos();
    let corner = |x: f64, y: f64| [center[0] + x * cos - y * sin, center[1] + x * sin + y * cos];
    WorldGeometry::Polyline {
        points: vec![
            corner(-half_w, -half_h),
            corner(half_w, -half_h),
            corner(half_w, half_h),
            corner(-half_w, half_h),
        ],
        closed: true,
    }
}

/// The box around the capsule `a`..`b` of `radius`.
fn capsule_box(a: [f64; 2], b: [f64; 2], radius: f64) -> WorldGeometry {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let length = dx.hypot(dy);
    if length <= f64::EPSILON {
        return WorldGeometry::Circle { center: a, radius };
    }
    let (nx, ny) = (-dy / length * radius, dx / length * radius);
    WorldGeometry::Polyline {
        points: vec![
            [a[0] + nx, a[1] + ny],
            [b[0] + nx, b[1] + ny],
            [b[0] - nx, b[1] - ny],
            [a[0] - nx, a[1] - ny],
        ],
        closed: true,
    }
}

/// A placed drawing as line art; text is left out (it is noise at tile
/// size).
fn push_placed(art: &mut Vec<WorldPrimitive>, ink: Ink, shape: &GraphicShape) {
    match shape {
        GraphicShape::Line { a, b } => art.push(WorldPrimitive::line(ink, nm_xy(*a), nm_xy(*b))),
        GraphicShape::Arc {
            center,
            radius,
            start_deg,
            sweep_deg,
        } => art.push(WorldPrimitive::polyline(
            ink,
            sampled_arc(nm_xy(*center), nm_to_mm(*radius), *start_deg, *sweep_deg),
            false,
        )),
        GraphicShape::Circle { center, radius } => {
            art.push(WorldPrimitive::circle(
                ink,
                nm_xy(*center),
                nm_to_mm(*radius),
            ));
        }
        GraphicShape::Polygon { points } => art.push(WorldPrimitive::polyline(
            ink,
            points.iter().copied().map(nm_xy).collect(),
            true,
        )),
        GraphicShape::Text(_) => {}
    }
}

/// Points along the arc about `center` from `start_deg` through `sweep_deg`.
fn sampled_arc(center: [f64; 2], radius: f64, start_deg: f64, sweep_deg: f64) -> Vec<[f64; 2]> {
    let steps = (sweep_deg.abs() / ARC_CHORD_DEG).ceil().clamp(1.0, 64.0) as usize;
    (0..=steps)
        .map(|i| {
            let angle = (start_deg + sweep_deg * i as f64 / steps as f64).to_radians();
            [
                center[0] + radius * angle.cos(),
                center[1] + radius * angle.sin(),
            ]
        })
        .collect()
}

// ─── Sheet ────────────────────────────────────────────────────────

/// The preview of the root sheet `sheet`: wires, symbol bodies and pins,
/// power ports in their own ink.
pub fn sheet(sheet: &SchematicSheet) -> Thumbnail {
    let mut art = Vec::new();
    for wire in &sheet.wires {
        art.push(WorldPrimitive::line(
            Ink::Wire,
            xy(wire.start),
            xy(wire.end),
        ));
    }
    for symbol in &sheet.symbols {
        let ink = if symbol.is_power {
            Ink::PowerPort
        } else {
            Ink::SymbolBody
        };
        let lowered =
            lower_placed_symbol(sheet.body_of(symbol), symbol, &SchematicDefaults::BUILT_IN);
        art.extend(lowered.iter().filter_map(|p| symbol_art(ink, p)));
    }
    finish(ThumbnailKind::Sheet, art)
}

fn symbol_art(ink: Ink, primitive: &SymbolPrimitive) -> Option<WorldPrimitive> {
    Some(match primitive {
        SymbolPrimitive::Path { points, closed, .. } => {
            WorldPrimitive::polyline(ink, points.iter().copied().map(xy).collect(), *closed)
        }
        SymbolPrimitive::Circle { center, radius, .. } => {
            WorldPrimitive::circle(ink, xy(*center), *radius)
        }
        SymbolPrimitive::Arc {
            start, mid, end, ..
        } => WorldPrimitive::polyline(
            ink,
            arc_polyline(*start, *mid, *end)
                .into_iter()
                .map(xy)
                .collect(),
            false,
        ),
        SymbolPrimitive::Pin { tip, body, .. } => WorldPrimitive::line(ink, xy(*tip), xy(*body)),
        SymbolPrimitive::Text { .. } => return None,
    })
}

// ─── Cap and view box ─────────────────────────────────────────────

/// Drop the smallest primitives until the rest cost at most `budget`
/// segments; the kept ones keep their order.
fn cap(art: &mut Vec<WorldPrimitive>, budget: usize) {
    let total: usize = art.iter().map(WorldPrimitive::cost).sum();
    if total <= budget {
        return;
    }
    let mut by_size: Vec<usize> = (0..art.len()).collect();
    // Stable: equal sizes keep model order.
    by_size.sort_by(|&a, &b| art[b].extent().total_cmp(&art[a].extent()));
    let mut keep = vec![false; art.len()];
    let mut spent = 0;
    for index in by_size {
        let cost = art[index].cost();
        if spent + cost <= budget {
            keep[index] = true;
            spent += cost;
        }
    }
    let mut index = 0;
    art.retain(|_| {
        let kept = keep[index];
        index += 1;
        kept
    });
}

/// Cap `art`, put it in paint order and lay it out in the view box.
fn finish(kind: ThumbnailKind, mut art: Vec<WorldPrimitive>) -> Thumbnail {
    art.retain(WorldPrimitive::is_drawable);
    cap(&mut art, MAX_SEGMENTS);
    art.sort_by_key(|p| p.ink.draw_order());
    let bounds = art.iter().filter_map(WorldPrimitive::bounds).reduce(
        |[a0, b0, c0, d0], [a1, b1, c1, d1]| [a0.min(a1), b0.min(b1), c0.max(c1), d0.max(d1)],
    );
    let Some([min_x, min_y, max_x, max_y]) = bounds else {
        return Thumbnail {
            kind,
            view_box: [VIEW_BOX_SIDE, VIEW_BOX_SIDE],
            primitives: Vec::new(),
        };
    };
    let (width, height) = (max_x - min_x, max_y - min_y);
    let longest = width.max(height);
    let scale = if longest > 0.0 {
        f64::from(VIEW_BOX_SIDE) / longest
    } else {
        1.0
    };
    let place = |[x, y]: [f64; 2]| [unit((x - min_x) * scale), unit((y - min_y) * scale)];
    let primitives = art
        .into_iter()
        .map(|p| Primitive {
            ink: p.ink,
            filled: p.filled,
            geometry: match p.geometry {
                WorldGeometry::Polyline { points, closed } => Geometry::Polyline {
                    points: points.into_iter().map(place).collect(),
                    closed,
                },
                WorldGeometry::Circle { center, radius } => Geometry::Circle {
                    center: place(center),
                    radius: unit(radius * scale),
                },
            },
        })
        .collect();
    Thumbnail {
        kind,
        view_box: [unit(width * scale).max(1.0), unit(height * scale).max(1.0)],
        primitives,
    }
}

/// A view-box coordinate, to a tenth of a unit: finer than a tile pixel,
/// and it keeps the cache file small.
fn unit(value: f64) -> f32 {
    ((value * 10.0).round() / 10.0) as f32
}
