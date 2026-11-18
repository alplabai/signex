//! The marks of an assembly drawing, gathered from the board: drawings and
//! texts on the drawing's layers, each part's designator and the outline.

use pcb_geom::{GraphicShape, PlacedGraphic, PointNm, nm_to_mm};
use signex_types::format::parse_pcb_layer;
use signex_types::pcb::{PCB_DEFAULT_TEXT_SIZE_MM, PcbBoard, Point};
use signex_types::pcb_layer::{BoardSide, PcbLayer, TechnicalLayer};
use signex_types::property::PcbProperty;

use super::{AssemblyInk, AssemblyMark, ink_of};

/// The designator's property key.
const DESIGNATOR_KEY: &str = "Reference";
/// The stroke of the board outline, mm.
const OUTLINE_WIDTH_MM: f64 = 0.1;

/// A text size, else the PCB default.
fn text_size(size: f64) -> f64 {
    if size.is_finite() && size > 0.0 {
        size
    } else {
        PCB_DEFAULT_TEXT_SIZE_MM
    }
}

/// Every drawing and text of the board and its footprints on a layer the
/// `side` drawing draws in `ink`, and every dimension for the Dimensions
/// ink. The outline's Edge Cuts drawings are left to [`outline`].
pub(super) fn layer_marks(
    board: &PcbBoard,
    side: BoardSide,
    ink: AssemblyInk,
    out: &mut Vec<AssemblyMark>,
) {
    let wanted = |layer: &str| ink_of(side, layer) == Some(ink);
    footprint_drawings(board, ink, &wanted, out);
    board_drawings(board, ink, &wanted, out);
    if ink == AssemblyInk::Dimension {
        dimensions(board, &wanted, out);
    }
}

/// The footprints' drawings and texts on a `wanted` layer, placed.
fn footprint_drawings(
    board: &PcbBoard,
    ink: AssemblyInk,
    wanted: &dyn Fn(&str) -> bool,
    out: &mut Vec<AssemblyMark>,
) {
    for fp in &board.footprints {
        for graphic in fp.graphics.iter().filter(|g| wanted(&g.layer)) {
            if let Ok(Some(placed)) = pcb_geom::footprint_graphic_placement(fp, graphic) {
                let text = (graphic.text.as_str(), text_size(graphic.font_size));
                placed_marks(&placed, ink, text, out);
            }
        }
    }
}

/// The board's own drawings, text frames and texts on a `wanted` layer.
fn board_drawings(
    board: &PcbBoard,
    ink: AssemblyInk,
    wanted: &dyn Fn(&str) -> bool,
    out: &mut Vec<AssemblyMark>,
) {
    for graphic in board.graphics.iter().filter(|g| wanted(&g.layer)) {
        if let Ok(Some(placed)) = pcb_geom::board_graphic_placement(graphic) {
            placed_marks(&placed, ink, ("", 0.0), out);
        }
        let size = pcb_geom::text_frame_size_mm(graphic);
        for line in pcb_geom::text_frame_lines(graphic).unwrap_or_default() {
            out.push(AssemblyMark::Text {
                at: line.placement.anchor.to_mm(),
                rotation_deg: line.placement.rotation_deg,
                size_mm: size,
                text: line.text,
                ink,
            });
        }
    }
    for text in board.texts.iter().filter(|t| wanted(&t.layer)) {
        if let Ok(placement) = pcb_geom::board_text_placement(text) {
            out.push(AssemblyMark::Text {
                at: placement.anchor.to_mm(),
                rotation_deg: placement.rotation_deg,
                size_mm: text_size(text.font_size),
                text: text.text.clone(),
                ink,
            });
        }
    }
}

/// Each linear dimension on a `wanted` layer: its strokes and its value.
fn dimensions(board: &PcbBoard, wanted: &dyn Fn(&str) -> bool, out: &mut Vec<AssemblyMark>) {
    for dimension in board.free.dimensions.iter().filter(|d| wanted(&d.layer)) {
        let Some(drawing) = dimension.drawing() else {
            continue;
        };
        for (a, b) in &drawing.strokes {
            out.push(AssemblyMark::Stroke {
                points: vec![*a, *b],
                closed: false,
                width_mm: dimension.line_width,
                ink: AssemblyInk::Dimension,
            });
        }
        out.push(AssemblyMark::Text {
            at: drawing.label_at,
            rotation_deg: drawing.label_rotation_deg,
            size_mm: text_size(dimension.text_height),
            text: dimension.value_text(),
            ink: AssemblyInk::Dimension,
        });
    }
}

/// The designator of each part on `side`: its own designator text when it
/// sits on the side's Designator layer, else the designator at the part's
/// origin, turned with the part and kept upright.
pub(super) fn designators(board: &PcbBoard, side: BoardSide, out: &mut Vec<AssemblyMark>) {
    let layer = match side {
        BoardSide::Top => TechnicalLayer::DesignatorTop,
        BoardSide::Bottom => TechnicalLayer::DesignatorBottom,
    };
    let on_layer = |p: &&PcbProperty| {
        p.layer.as_deref().and_then(parse_pcb_layer) == Some(PcbLayer::Technical(layer))
    };
    for fp in &board.footprints {
        let fp_side = parse_pcb_layer(&fp.layer).and_then(PcbLayer::side);
        if fp_side.unwrap_or(BoardSide::Top) != side || fp.reference.trim().is_empty() {
            continue;
        }
        let own = fp
            .properties
            .iter()
            .filter(|p| p.key == DESIGNATOR_KEY)
            .find(on_layer);
        let (property, size) = match own {
            Some(own) if own.hidden => continue,
            Some(own) => (own.clone(), own.font_size),
            None => (
                PcbProperty {
                    key: DESIGNATOR_KEY.to_string(),
                    position: Some(Point::new(0.0, 0.0)),
                    ..PcbProperty::default()
                },
                None,
            ),
        };
        let Ok(placed) = pcb_geom::property_text_placement(fp, &property) else {
            continue;
        };
        let turn = if own.is_some() {
            placed.rotation_deg
        } else {
            upright(placed.rotation_deg)
        };
        out.push(AssemblyMark::Text {
            at: placed.anchor.to_mm(),
            rotation_deg: turn,
            size_mm: text_size(size.unwrap_or(0.0)),
            text: fp.reference.clone(),
            ink: AssemblyInk::Designator,
        });
    }
}

/// `degrees` turned into `-90 < degrees <= 90`, so a text never reads
/// upside down.
fn upright(degrees: f64) -> f64 {
    let d = degrees.rem_euclid(360.0);
    if d > 270.0 {
        d - 360.0
    } else if d > 90.0 {
        d - 180.0
    } else {
        d
    }
}

/// The board outline and the Edge Cuts drawings, the footprints' (a slot
/// or a cutout) as well as the board's.
pub(super) fn outline(board: &PcbBoard, side: BoardSide, out: &mut Vec<AssemblyMark>) {
    let ring: Vec<Point> = board
        .outline
        .iter()
        .copied()
        .filter(|p| p.x.is_finite() && p.y.is_finite())
        .collect();
    if ring.len() >= 3 {
        out.push(AssemblyMark::Stroke {
            points: ring,
            closed: true,
            width_mm: OUTLINE_WIDTH_MM,
            ink: AssemblyInk::Outline,
        });
    }
    let wanted = |layer: &str| ink_of(side, layer) == Some(AssemblyInk::Outline);
    footprint_drawings(board, AssemblyInk::Outline, &wanted, out);
    board_drawings(board, AssemblyInk::Outline, &wanted, out);
}

/// The marks of one placed drawing; `text` is a text drawing's string and
/// size in mm.
fn placed_marks(
    placed: &PlacedGraphic,
    ink: AssemblyInk,
    text: (&str, f64),
    out: &mut Vec<AssemblyMark>,
) {
    let mm = |p: &PointNm| p.to_mm();
    let width_mm = nm_to_mm(placed.width);
    let outline = |points: Vec<Point>, closed: bool| AssemblyMark::Stroke {
        points,
        closed,
        width_mm,
        ink,
    };
    let mark = match &placed.shape {
        GraphicShape::Line { a, b } => outline(vec![mm(a), mm(b)], false),
        GraphicShape::Arc {
            center,
            radius,
            start_deg,
            sweep_deg,
        } => outline(arc_points(*center, *radius, *start_deg, *sweep_deg), false),
        GraphicShape::Circle { center, radius } => {
            let mut ring = arc_points(*center, *radius, 0.0, 360.0);
            ring.pop();
            closed_shape(ring, placed.filled, width_mm, ink)
        }
        GraphicShape::Polygon { points } => closed_shape(
            points.iter().map(mm).collect(),
            placed.filled,
            width_mm,
            ink,
        ),
        GraphicShape::Text(placement) if !text.0.trim().is_empty() => AssemblyMark::Text {
            at: placement.anchor.to_mm(),
            rotation_deg: placement.rotation_deg,
            size_mm: text.1,
            text: text.0.to_string(),
            ink,
        },
        GraphicShape::Text(_) => return,
    };
    out.push(mark);
}

fn closed_shape(ring: Vec<Point>, filled: bool, width_mm: f64, ink: AssemblyInk) -> AssemblyMark {
    if filled {
        AssemblyMark::Fill {
            rings: vec![ring],
            ink,
        }
    } else {
        AssemblyMark::Stroke {
            points: ring,
            closed: true,
            width_mm,
            ink,
        }
    }
}

/// Points along an arc in the model's axes, about one every 10 degrees.
fn arc_points(center: PointNm, radius: i64, start_deg: f64, sweep_deg: f64) -> Vec<Point> {
    let steps = ((sweep_deg.abs() / 10.0).ceil() as usize).clamp(2, 72);
    let (cx, cy, r) = (nm_to_mm(center.x), nm_to_mm(center.y), nm_to_mm(radius));
    (0..=steps)
        .map(|i| {
            let t = (start_deg + sweep_deg * i as f64 / steps as f64).to_radians();
            Point::new(cx + r * t.cos(), cy + r * t.sin())
        })
        .collect()
}
