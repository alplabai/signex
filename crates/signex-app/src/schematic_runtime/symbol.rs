//! Placed-symbol geometry for the schematic canvas: the world box used for
//! hit-test, selection and culling, and the draw inputs, both taken from the
//! one lowering in `signex_types::schematic::lower_symbol`.

use super::snapshot::{ArcStroke, push_arc};
use super::*;
use signex_types::schematic::{SymbolPrimitive, arc_polyline, lower_symbol, primitives_bounds};

/// Half extents of the box a symbol gets when its library graphics are
/// missing or empty.
const FALLBACK_HALF_W_MM: f64 = 4.0;
const FALLBACK_HALF_H_MM: f64 = 2.8;

/// World primitives of `symbol` and the box enclosing them. A symbol whose
/// library entry is missing or draws nothing gets no primitives and the
/// fallback box.
///
/// Lowered on demand: the canvas only calls this when its content cache is
/// rebuilt (camera change or a `RenderInvalidation` refresh), and hit-test on
/// a click. If big sheets make that rebuild slow, memoise the result in
/// `SchematicRenderCache`, refreshed on `SYMBOLS | LIB_SYMBOLS`.
pub(super) fn symbol_geometry(
    snapshot: &SchematicRenderSnapshot,
    symbol: &Symbol,
) -> (Vec<SymbolPrimitive>, Aabb) {
    let primitives = snapshot
        .lib_symbols
        .get(&symbol.lib_id)
        .map(|lib| lower_symbol(lib, symbol))
        .unwrap_or_default();
    match primitives_bounds(&primitives) {
        Some(bbox) => (primitives, bbox),
        None => (Vec::new(), fallback_box(symbol)),
    }
}

/// Hit-test / selection box of `symbol`.
pub(super) fn symbol_bounds(snapshot: &SchematicRenderSnapshot, symbol: &Symbol) -> Aabb {
    symbol_geometry(snapshot, symbol).1
}

fn fallback_box(symbol: &Symbol) -> Aabb {
    Aabb::new(
        symbol.position.x - FALLBACK_HALF_W_MM,
        symbol.position.y - FALLBACK_HALF_H_MM,
        symbol.position.x + FALLBACK_HALF_W_MM,
        symbol.position.y + FALLBACK_HALF_H_MM,
    )
}

/// Library ids of placed symbols that will be drawn as the placeholder box
/// because their library entry is missing or draws nothing. Sorted, one entry
/// per id. The caller reports them when a sheet loads, not on every redraw.
pub(crate) fn symbols_without_geometry(snapshot: &SchematicRenderSnapshot) -> Vec<String> {
    let mut ids: Vec<String> = snapshot
        .symbols
        .iter()
        .filter(|symbol| {
            snapshot
                .lib_symbols
                .get(&symbol.lib_id)
                .map(|lib| primitives_bounds(&lower_symbol(lib, symbol)).is_none())
                .unwrap_or(true)
        })
        .map(|symbol| symbol.lib_id.clone())
        .collect();
    ids.sort();
    ids.dedup();
    ids
}

/// Colours and render id shared by every primitive of one symbol.
pub(super) struct SymbolStyle {
    pub id: u64,
    pub stroke: Color,
    pub fill: Color,
    pub pin: Color,
    pub scale: f32,
    /// Face the symbol's library text draws in (the sheet default).
    pub family: Option<std::sync::Arc<str>>,
}

/// The renderer input lists a symbol draws into.
pub(super) struct SymbolSink<'a> {
    pub wires: &'a mut Vec<WireInput>,
    pub arcs: &'a mut Vec<ArcInput>,
    pub polygons: &'a mut Vec<PolygonInput>,
    pub texts: &'a mut Vec<TextInput>,
}

impl SymbolStyle {
    fn stroke_mm(&self, width: f64) -> f32 {
        (width as f32).max(stroke_world_mm(
            signex_types::schematic::SCHEMATIC_RENDER_SYMBOL_BODY_STROKE_PX,
            self.scale,
        ))
    }

    fn fill_rgba(&self, fill: FillType) -> [f32; 4] {
        match fill {
            FillType::None => [0.0, 0.0, 0.0, 0.0],
            FillType::Outline => to_rgba(self.stroke),
            FillType::Background => to_rgba(self.fill),
        }
    }
}

pub(super) fn push_fallback_box(bbox: &Aabb, style: &SymbolStyle, sink: &mut SymbolSink<'_>) {
    sink.polygons.push(PolygonInput {
        vertices: vec![
            [bbox.min_x as f32, bbox.min_y as f32],
            [bbox.max_x as f32, bbox.min_y as f32],
            [bbox.max_x as f32, bbox.max_y as f32],
            [bbox.min_x as f32, bbox.max_y as f32],
        ],
        fill_color: to_rgba(style.fill),
        stroke_color: Some(to_rgba(style.stroke)),
        stroke_width_mm: style.stroke_mm(0.0),
    });
}

pub(super) fn push_primitives(
    primitives: &[SymbolPrimitive],
    style: &SymbolStyle,
    sink: &mut SymbolSink<'_>,
) {
    let stroke = to_rgba(style.stroke);
    let mut next_id = style.id;
    let mut segment = |sink: &mut SymbolSink<'_>, p0: Point, p1: Point, width_mm, color| {
        sink.wires.push(WireInput {
            id: next_id,
            p0: [p0.x as f32, p0.y as f32],
            p1: [p1.x as f32, p1.y as f32],
            width_mm,
            explicit_color: Some(color),
        });
        next_id = next_id.wrapping_add(1);
    };
    for primitive in primitives {
        match primitive {
            SymbolPrimitive::Path {
                points,
                closed,
                width,
                fill,
            } => {
                if *closed || *fill != FillType::None {
                    sink.polygons.push(PolygonInput {
                        vertices: points.iter().map(|p| [p.x as f32, p.y as f32]).collect(),
                        fill_color: style.fill_rgba(*fill),
                        stroke_color: Some(stroke),
                        stroke_width_mm: style.stroke_mm(*width),
                    });
                } else {
                    for pair in points.windows(2) {
                        segment(sink, pair[0], pair[1], style.stroke_mm(*width), stroke);
                    }
                }
            }
            SymbolPrimitive::Circle {
                center,
                radius,
                width,
                fill,
            } => sink.polygons.push(PolygonInput {
                vertices: circle_vertices([center.x, center.y], *radius as f32, 40),
                fill_color: style.fill_rgba(*fill),
                stroke_color: Some(stroke),
                stroke_width_mm: style.stroke_mm(*width),
            }),
            SymbolPrimitive::Arc {
                start,
                mid,
                end,
                width,
                fill,
            } => {
                if *fill != FillType::None {
                    sink.polygons.push(PolygonInput {
                        vertices: arc_polyline(*start, *mid, *end)
                            .iter()
                            .map(|p| [p.x as f32, p.y as f32])
                            .collect(),
                        fill_color: style.fill_rgba(*fill),
                        stroke_color: None,
                        stroke_width_mm: 0.0,
                    });
                }
                push_arc(
                    [*start, *mid, *end],
                    ArcStroke {
                        id: style.id,
                        width_mm: style.stroke_mm(*width),
                        color: stroke,
                    },
                    style.scale,
                    sink.arcs,
                    sink.wires,
                );
            }
            SymbolPrimitive::Text {
                text,
                position,
                rotation_deg,
                size,
                h_align,
                v_align,
            } => sink.texts.push(TextInput {
                content: text.clone(),
                position: [position.x as f32, position.y as f32],
                size_mm: *size as f32,
                color: stroke,
                bold: false,
                italic: false,
                family: style.family.clone(),
                rotation_rad: rotation_deg.to_radians() as f32,
                h_align: *h_align,
                v_align: *v_align,
            }),
            SymbolPrimitive::Pin { tip, body } => {
                segment(sink, *tip, *body, style.stroke_mm(0.0), to_rgba(style.pin));
            }
        }
    }
}

#[cfg(test)]
#[path = "symbol_tests.rs"]
mod tests;
