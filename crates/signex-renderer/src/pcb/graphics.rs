//! Drawings on the board (#2387): a footprint's silkscreen, fab, assembly,
//! courtyard and 3D body outlines, and the board's own drawings and texts
//! (a text frame's box and its lines, #2424), placed by `pcb-geom`, in
//! their layer's board palette colour. Rules: `docs/RENDERING_RULES.md`,
//! "pcb-graphics".
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: IPC-2612-1, wgpu/WGSL public docs.

use pcb_geom::{GraphicShape, PlacedGraphic, PointNm, TextPlacement, nm_to_mm};
use signex_gfx::primitive::circle::Circle;
use signex_gfx::primitive::polygon::GpuPolygon;
use signex_gfx::scene::Scene;
use signex_types::format::parse_pcb_layer;
use signex_types::pcb::{
    BoardGraphic, BoardText, Footprint, FpGraphic, PCB_DEFAULT_GRAPHIC_WIDTH_MM,
    PCB_DEFAULT_TEXT_SIZE_MM, PcbBoard,
};
use signex_types::pcb_graphic::BoardGraphicKind;
use signex_types::pcb_layer::PcbLayer;
use signex_types::schematic::HAlign;

use super::board_features::{line, push_loop};
use super::board_paint::BoardObject;
use super::emit::{dim_if, mm_with_floor, point_to_xy};
use super::footprint_text::FootprintTextInput;
use super::layer_view::PcbLayerView;
use crate::theme::ResolvedTheme;

/// The largest turn one chord of a drawn arc spans, degrees.
const ARC_CHORD_DEG: f32 = 6.0;

/// A drawing's geometry in board mm.
#[derive(Clone, Debug, PartialEq)]
pub enum GraphicShapeInput {
    Line {
        p0: [f32; 2],
        p1: [f32; 2],
    },
    /// Degrees in the board's axes, the sense a pad turns.
    Arc {
        center: [f32; 2],
        radius_mm: f32,
        start_deg: f32,
        sweep_deg: f32,
    },
    Circle {
        center: [f32; 2],
        radius_mm: f32,
    },
    /// A closed outline.
    Polygon {
        points: Vec<[f32; 2]>,
    },
}

/// One drawing as the board draws it.
#[derive(Clone, Debug, PartialEq)]
pub struct GraphicInput {
    pub shape: GraphicShapeInput,
    /// The layer it is drawn on, and in the colour of.
    pub layer: PcbLayer,
    /// Stroke width; a drawing saved with none takes
    /// `PCB_DEFAULT_GRAPHIC_WIDTH_MM`.
    pub width_mm: f32,
    pub filled: bool,
    /// On a copper layer off the active one, drawn faint.
    pub dimmed: bool,
}

/// What a set of drawings lowers to: strokes and areas for the scene, texts
/// for the board text layer.
#[derive(Debug, Default)]
pub(super) struct Drawings {
    pub(super) graphics: Vec<GraphicInput>,
    pub(super) texts: Vec<FootprintTextInput>,
}

/// The layer named `token` when `view` shows it, and whether it is dimmed:
/// a copper layer other than the active copper layer, as copper is. A layer
/// this build does not know has no colour and is not drawn.
fn shown_layer(token: &str, view: Option<&PcbLayerView>) -> Option<(PcbLayer, bool)> {
    let layer = parse_pcb_layer(token)?;
    if view.is_some_and(|v| !v.is_visible(layer)) {
        return None;
    }
    let dimmed = view
        .and_then(PcbLayerView::active_copper)
        .zip(layer.copper())
        .is_some_and(|(active, copper)| active != copper);
    Some((layer, dimmed))
}

/// The drawings of `footprint` that `view` shows. A drawing `pcb-geom`
/// refuses (non-finite or beyond 1 km) is not drawn.
pub(super) fn footprint_drawings(
    footprint: &Footprint,
    view: Option<&PcbLayerView>,
    out: &mut Drawings,
) {
    for graphic in &footprint.graphics {
        let Some((layer, dimmed)) = shown_layer(&graphic.layer, view) else {
            continue;
        };
        let Ok(Some(placed)) = pcb_geom::footprint_graphic_placement(footprint, graphic) else {
            continue;
        };
        match &placed.shape {
            GraphicShape::Text(at) if shows(view, BoardObject::Texts) => {
                out.texts.extend(text_graphic(graphic, at, layer));
            }
            GraphicShape::Text(_) => {}
            _ => out.graphics.extend(graphic_input(&placed, layer, dimmed)),
        }
    }
}

/// Whether `view` draws objects of `kind` (View Options, #2363).
fn shows(view: Option<&PcbLayerView>, kind: BoardObject) -> bool {
    view.is_none_or(|v| v.shows_object(kind))
}

/// The board's own drawings and texts that `view` shows.
pub(super) fn board_drawings(board: &PcbBoard, view: Option<&PcbLayerView>, out: &mut Drawings) {
    let texts = shows(view, BoardObject::Texts);
    for graphic in &board.graphics {
        let Some((layer, dimmed)) = shown_layer(&graphic.layer, view) else {
            continue;
        };
        // A text frame is a text: hiding Texts drops its box too.
        if !texts && graphic.kind() == Some(BoardGraphicKind::TextFrame) {
            continue;
        }
        if let Ok(Some(placed)) = pcb_geom::board_graphic_placement(graphic) {
            out.graphics.extend(graphic_input(&placed, layer, dimmed));
        }
        out.texts.extend(frame_lines(graphic, layer));
    }
    for text in board.texts.iter().filter(|_| texts) {
        let Some((layer, _)) = shown_layer(&text.layer, view) else {
            continue;
        };
        if let Some(input) = board_text(text, layer) {
            out.texts.push(input);
        }
    }
    free_drawings(board, view, out);
}

/// The free arcs off copper and the dimensions (#2373); a copper arc is
/// drawn as copper with the tracks.
fn free_drawings(board: &PcbBoard, view: Option<&PcbLayerView>, out: &mut Drawings) {
    for arc in board
        .free
        .arcs
        .iter()
        .filter(|a| shows(view, BoardObject::Arcs) && !pcb_geom::arc_is_copper(a))
    {
        let (Some((layer, dimmed)), Some(sweep)) = (shown_layer(&arc.layer, view), arc.sweep())
        else {
            continue;
        };
        out.graphics.push(GraphicInput {
            shape: GraphicShapeInput::Arc {
                center: point_to_xy(sweep.center),
                radius_mm: sweep.radius as f32,
                start_deg: sweep.start_angle.to_degrees() as f32,
                sweep_deg: sweep.sweep.to_degrees() as f32,
            },
            layer,
            width_mm: mm_with_floor(arc.width, PCB_DEFAULT_GRAPHIC_WIDTH_MM, 0.0),
            filled: false,
            dimmed,
        });
    }
    let dimensions = shows(view, BoardObject::Dimensions);
    for dimension in board.free.dimensions.iter().filter(|_| dimensions) {
        let (Some((layer, dimmed)), Some(drawing)) =
            (shown_layer(&dimension.layer, view), dimension.drawing())
        else {
            continue;
        };
        let width_mm = mm_with_floor(dimension.line_width, PCB_DEFAULT_GRAPHIC_WIDTH_MM, 0.0);
        out.graphics
            .extend(drawing.strokes.iter().map(|(a, b)| GraphicInput {
                shape: GraphicShapeInput::Line {
                    p0: point_to_xy(*a),
                    p1: point_to_xy(*b),
                },
                layer,
                width_mm,
                filled: false,
                dimmed,
            }));
        out.texts.push(FootprintTextInput {
            content: dimension.value_text(),
            position: point_to_xy(drawing.label_at),
            rotation_deg: drawing.label_rotation_deg as f32,
            size_mm: text_size_mm(dimension.text_height),
            mirrored: false,
            layer,
            justify: HAlign::Center,
            designator: false,
        });
    }
}

fn xy(p: PointNm) -> [f32; 2] {
    point_to_xy(p.to_mm())
}

fn graphic_input(placed: &PlacedGraphic, layer: PcbLayer, dimmed: bool) -> Option<GraphicInput> {
    let shape = match &placed.shape {
        GraphicShape::Line { a, b } => GraphicShapeInput::Line {
            p0: xy(*a),
            p1: xy(*b),
        },
        GraphicShape::Arc {
            center,
            radius,
            start_deg,
            sweep_deg,
        } => GraphicShapeInput::Arc {
            center: xy(*center),
            radius_mm: nm_to_mm(*radius) as f32,
            start_deg: *start_deg as f32,
            sweep_deg: *sweep_deg as f32,
        },
        GraphicShape::Circle { center, radius } => GraphicShapeInput::Circle {
            center: xy(*center),
            radius_mm: nm_to_mm(*radius) as f32,
        },
        GraphicShape::Polygon { points } => GraphicShapeInput::Polygon {
            points: points.iter().map(|&p| xy(p)).collect(),
        },
        GraphicShape::Text(_) => return None,
    };
    Some(GraphicInput {
        shape,
        layer,
        width_mm: mm_with_floor(nm_to_mm(placed.width), PCB_DEFAULT_GRAPHIC_WIDTH_MM, 0.0),
        filled: placed.filled,
        dimmed,
    })
}

fn text_size_mm(size: f64) -> f32 {
    if size.is_finite() && size > 0.0 {
        size as f32
    } else {
        PCB_DEFAULT_TEXT_SIZE_MM as f32
    }
}

/// A footprint's text drawing, drawn as its designator is (#2368).
fn text_graphic(
    graphic: &FpGraphic,
    at: &TextPlacement,
    layer: PcbLayer,
) -> Option<FootprintTextInput> {
    if graphic.text.trim().is_empty() {
        return None;
    }
    Some(FootprintTextInput {
        content: graphic.text.clone(),
        position: xy(at.anchor),
        rotation_deg: at.rotation_deg as f32,
        size_mm: text_size_mm(graphic.font_size),
        mirrored: at.mirrored,
        layer,
        justify: HAlign::Center,
        designator: false,
    })
}

/// A text frame's lines (#2424), each drawn as a board text is; none for
/// another drawing or one `pcb-geom` refuses.
fn frame_lines(graphic: &BoardGraphic, layer: PcbLayer) -> Vec<FootprintTextInput> {
    let size_mm = pcb_geom::text_frame_size_mm(graphic) as f32;
    pcb_geom::text_frame_lines(graphic)
        .unwrap_or_default()
        .into_iter()
        .map(|line| FootprintTextInput {
            content: line.text,
            position: xy(line.placement.anchor),
            rotation_deg: line.placement.rotation_deg as f32,
            size_mm,
            mirrored: line.placement.mirrored,
            layer,
            justify: HAlign::Center,
            designator: false,
        })
        .collect()
}

fn board_text(text: &BoardText, layer: PcbLayer) -> Option<FootprintTextInput> {
    if text.text.trim().is_empty() {
        return None;
    }
    let at = pcb_geom::board_text_placement(text).ok()?;
    Some(FootprintTextInput {
        content: text.text.clone(),
        position: xy(at.anchor),
        rotation_deg: at.rotation_deg as f32,
        size_mm: text_size_mm(text.font_size),
        mirrored: at.mirrored,
        layer,
        justify: HAlign::Center,
        designator: false,
    })
}

/// Append `graphics` to the overlay buckets of `scene`, so a silkscreen
/// line draws over the copper under it.
pub(super) fn emit(graphics: &[GraphicInput], theme: &ResolvedTheme, scene: &mut Scene) {
    for graphic in graphics {
        let color = dim_if(theme.board_layer_color(graphic.layer), graphic.dimmed);
        let width = graphic.width_mm;
        match &graphic.shape {
            GraphicShapeInput::Line { p0, p1 } => {
                scene.overlay_lines.push(line(*p0, *p1, color, width));
            }
            GraphicShapeInput::Arc {
                center,
                radius_mm,
                start_deg,
                sweep_deg,
            } => {
                let points = arc_points(*center, *radius_mm, *start_deg, *sweep_deg);
                for pair in points.windows(2) {
                    scene
                        .overlay_lines
                        .push(line(pair[0], pair[1], color, width));
                }
            }
            GraphicShapeInput::Circle { center, radius_mm } => {
                // A ring's stroke is centred on its radius; a disc reaches
                // the stroke's outer edge.
                let circle = if graphic.filled {
                    Circle {
                        center: *center,
                        radius: radius_mm + width * 0.5,
                        stroke_width: 0.0,
                        color,
                    }
                } else {
                    Circle {
                        center: *center,
                        radius: *radius_mm,
                        stroke_width: width,
                        color,
                    }
                };
                scene.overlay_circles.push(circle);
            }
            GraphicShapeInput::Polygon { points } if graphic.filled && points.len() >= 3 => {
                scene.overlay_polygons.push(GpuPolygon {
                    vertices: points.clone(),
                    holes: Vec::new(),
                    fill_color: color,
                    stroke_color: Some(color),
                    stroke_width: width,
                });
            }
            GraphicShapeInput::Polygon { points } if points.len() >= 2 => {
                push_loop(scene, points, color, width);
            }
            GraphicShapeInput::Polygon { .. } => {}
        }
    }
}

/// The points along an arc, at most `ARC_CHORD_DEG` apart, both ends
/// included.
pub(super) fn arc_points(
    center: [f32; 2],
    radius: f32,
    start_deg: f32,
    sweep_deg: f32,
) -> Vec<[f32; 2]> {
    let chords = (sweep_deg.abs() / ARC_CHORD_DEG).ceil().max(1.0) as usize;
    (0..=chords)
        .map(|i| {
            let deg = start_deg + sweep_deg * i as f32 / chords as f32;
            let (sin, cos) = deg.to_radians().sin_cos();
            [center[0] + radius * cos, center[1] + radius * sin]
        })
        .collect()
}

#[cfg(test)]
#[path = "graphics_tests.rs"]
mod tests;
