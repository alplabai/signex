//! The bake preview on the canvas (mockup row 190): while Bake preview is open, what
//! Bake writes, drawn in its layer colours, with the sketch faint on top. A
//! veil in the ground colour pushes back what is under it. A board draws its
//! bake (outline, cutouts, mounting holes, keepouts, V-scores) in the board
//! theme's `BoardEdge`, `BoardOpening`, `Keepout` and `BoardArea` slots; a
//! footprint its pads, silk and courtyard as Pads mode draws them, plus the
//! mask, paste, pour, keepout and cutout fields. Rules:
//! `docs/FOOTPRINT_SKETCH.md`, "Leaving sketch mode", and
//! `docs/PCB_SKETCH.md`, "The way out".

use iced::widget::canvas::{self, LineDash, Path, Stroke};
use iced::{Color, Point};
use signex_gfx::style::ColorSlot;
use signex_library::primitive::footprint::{LayerId, Polygon};
use signex_renderer::theme::ResolvedTheme;
use signex_sketch::SketchData;
use signex_sketch::entity::EntityKind;
use signex_sketch::id::SketchEntityId;
use signex_sketch::solver::state::{circle_radius, point_xy};
use signex_types::pcb::BoardGraphic;
use signex_types::pcb_graphic::BoardGraphicKind;
use signex_types::pcb_layer::{CopperLayer, PcbLayer, TechnicalLayer};
use signex_types::pcb_sketch::BakedBoard;
use signex_types::sketch::SketchHost;
use signex_types::sketch::board_attr::SketchBoardItems;

use super::super::super::{FootprintCanvas, FootprintCanvasState};
use super::super::cursor_ink::CursorInk;
use super::super::layers::{LayerInput, draw_courtyard, draw_pads, draw_silk};
use super::super::pad::PadLabels;
use crate::library::editor::footprint::layers::LayerPalette;
use crate::library::editor::footprint::state::{EditorMode, FootprintEditorState};
use crate::pcb_canvas::layer_style::board_layer_colour;
use crate::styles::TypeScale;

/// How much of the ground colour the veil lays over the canvas.
const VEIL_ALPHA: f32 = 0.6;
/// The sketch over the result: there, but quieter than the result.
const FAINT_ALPHA: f32 = 0.35;
/// Fill of a baked region, against its layer colour.
const REGION_FILL_ALPHA: f32 = 0.35;
/// Fill of a keepout, which only forbids.
const KEEPOUT_FILL_ALPHA: f32 = 0.15;
/// Outline stroke of baked geometry, in pixels.
const EDGE_WIDTH: f32 = 1.5;
/// Stroke of the faint sketch, in pixels.
const FAINT_WIDTH: f32 = 1.0;
/// Segments of an arc of the faint sketch.
const ARC_STEPS: usize = 24;
/// Dashes of a V-score and of construction geometry, in pixels.
const DASHES: &[f32] = &[6.0, 4.0];
/// Radius of a component anchor's ring and reach of its cross, in pixels.
const ANCHOR_RING: f32 = 5.0;
const ANCHOR_ARM: f32 = 9.0;

impl FootprintCanvas<'_> {
    /// The bake preview, over everything else; nothing while it is closed.
    pub(in crate::library::editor::footprint::canvas) fn draw_bake_preview(
        &self,
        frame: &mut canvas::Frame,
        cstate: &FootprintCanvasState,
    ) {
        if !self.state.sketch_exit.preview || self.state.mode != EditorMode::Sketch {
            return;
        }
        let Some(sketch) = self.sketch else {
            return;
        };
        frame.fill_rectangle(
            Point::ORIGIN,
            frame.size(),
            Color {
                a: VEIL_ALPHA,
                ..self.bg_color
            },
        );
        match self.state.sketch_host() {
            // A refused board bake draws nothing; the card says why.
            SketchHost::Board => {
                let exit = &self.state.sketch_exit;
                if let Some(baked) = exit.board_result.as_deref() {
                    draw_board_bake(frame, cstate, baked);
                }
                if let Some(items) = exit.board_items.as_deref() {
                    draw_board_items(frame, cstate, items);
                }
            }
            SketchHost::Footprint => self.draw_footprint_bake(frame, cstate, sketch),
        }
        let ink = CursorInk::sketch().mark;
        draw_faint_sketch(
            frame,
            cstate,
            sketch,
            self.state,
            ink.scale_alpha(FAINT_ALPHA),
        );
    }

    /// The footprint's bake: the baked pads, silk and courtyard the editor
    /// holds, and the fields the canvas does not draw otherwise, baked from
    /// the same solve.
    fn draw_footprint_bake(
        &self,
        frame: &mut canvas::Frame,
        cstate: &FootprintCanvasState,
        sketch: &SketchData,
    ) {
        let input = LayerInput {
            pads: &self.state.pads,
            silk_f: self.silk_f,
            silk_b: self.silk_b,
            visibility: self.state.layer_visibility,
            courtyard_outline: self.state.courtyard_outline(),
            courtyard_rect: self.state.courtyard_mm,
            // The preview shows the result, not the selection.
            selected_pad: None,
            selected_pads_extra: &[],
            selected_silk_f: None,
            pad_labels: PadLabels::Editor,
            palette: LayerPalette::Editor,
        };
        draw_silk(frame, cstate, &input);
        draw_courtyard(frame, cstate, &input);
        if let Some(solve) = self.state.last_solve.as_ref() {
            for (polygon, layer) in footprint_regions(sketch, solve) {
                let colour = board_layer_colour(layer);
                let alpha = match layer {
                    PcbLayer::Technical(
                        TechnicalLayer::KeepoutTop | TechnicalLayer::KeepoutBottom,
                    ) => KEEPOUT_FILL_ALPHA,
                    _ => REGION_FILL_ALPHA,
                };
                draw_region(
                    frame,
                    cstate,
                    polygon.points.iter().map(|p| (p[0], p[1])),
                    colour,
                    alpha,
                );
            }
        }
        draw_pads(frame, cstate, &input);
    }
}

/// The mask, paste, pour, keepout and cutout regions a bake of `sketch` at
/// `solve` writes, with the layer each is drawn in. A field whose bake
/// fails is left out; the preview card lists the bake's warnings.
fn footprint_regions(
    sketch: &SketchData,
    solve: &signex_sketch::solver::FullSolveOutput,
) -> Vec<(Polygon, PcbLayer)> {
    use TechnicalLayer as T;
    let params = &solve.params;
    let mut warnings = Vec::new();
    let mut out = Vec::new();
    let mut mask = Vec::new();
    if signex_bake::bake_mask_openings(sketch, solve, &mut mask, &mut warnings).is_ok() {
        out.extend(mask.into_iter().map(|m| {
            let layer = sided(&m.layer, T::SolderMaskTop, T::SolderMaskBottom);
            (m.boundary, layer)
        }));
    }
    let mut paste = Vec::new();
    if signex_bake::bake_paste_apertures(sketch, solve, &mut paste, &mut warnings).is_ok() {
        out.extend(paste.into_iter().map(|p| {
            let layer = sided(&p.layer, T::PasteTop, T::PasteBottom);
            (p.boundary, layer)
        }));
    }
    let mut pours = Vec::new();
    if signex_bake::bake_pours(sketch, solve, params, &mut pours, &mut warnings).is_ok() {
        out.extend(pours.into_iter().map(|p| {
            let copper = if is_bottom(&p.layer) {
                CopperLayer::Bottom
            } else {
                CopperLayer::Top
            };
            (p.boundary, PcbLayer::Copper(copper))
        }));
    }
    let mut keepouts = Vec::new();
    if signex_bake::bake_keepouts(sketch, solve, &mut keepouts, &mut warnings).is_ok() {
        out.extend(keepouts.into_iter().map(|k| {
            let layer = sided(&k.layer, T::KeepoutTop, T::KeepoutBottom);
            (k.boundary, layer)
        }));
    }
    let mut cutouts = Vec::new();
    if signex_bake::bake_cutouts(sketch, solve, params, &mut cutouts, &mut warnings).is_ok() {
        out.extend(
            cutouts
                .into_iter()
                .map(|c| (c.boundary, PcbLayer::Technical(T::EdgeCuts))),
        );
    }
    out
}

/// A baked footprint names its bottom layers "Bottom …"
/// (`signex_bake::layer_label`).
fn is_bottom(layer: &LayerId) -> bool {
    layer.as_str().starts_with("Bottom")
}

fn sided(layer: &LayerId, top: TechnicalLayer, bottom: TechnicalLayer) -> PcbLayer {
    PcbLayer::Technical(if is_bottom(layer) { bottom } else { top })
}

/// The board's bake in the board theme's slots, as the PCB canvas draws a
/// board's features (`docs/PCB_SKETCH.md`, "Drawing the board").
fn draw_board_bake(frame: &mut canvas::Frame, cstate: &FootprintCanvasState, baked: &BakedBoard) {
    let theme = ResolvedTheme::from_canvas_colors(crate::styles::board_canvas_colors());
    let slot = |slot| {
        let [r, g, b, a] = theme.color(slot);
        Color { r, g, b, a }
    };
    let edge = slot(ColorSlot::BoardEdge);
    let opening = slot(ColorSlot::BoardOpening);
    let keepout = slot(ColorSlot::Keepout);
    let solid = |colour| Stroke::default().with_width(EDGE_WIDTH).with_color(colour);
    if let Some(path) = baked
        .outline
        .as_deref()
        .and_then(|outline| polygon_path(cstate, outline.iter().map(|p| (p.x, p.y))))
    {
        frame.fill(&path, slot(ColorSlot::BoardArea));
        frame.stroke(&path, solid(edge));
    }
    for item in &baked.keepouts {
        let points = item.boundary.iter().map(|p| (p.x, p.y));
        draw_region(frame, cstate, points, keepout, KEEPOUT_FILL_ALPHA);
    }
    for cutout in &baked.cutouts {
        if let Some(path) = polygon_path(cstate, cutout.boundary.iter().map(|p| (p.x, p.y))) {
            frame.fill(&path, opening);
            frame.stroke(&path, solid(edge));
        }
    }
    for hole in &baked.holes {
        let radius = (hole.drill * 0.5) as f32 * cstate.scale;
        let path = Path::circle(
            cstate.world_to_screen((hole.position.x, hole.position.y)),
            radius,
        );
        frame.fill(&path, opening);
        frame.stroke(&path, solid(edge));
    }
    for score in &baked.v_scores {
        let path = Path::line(
            cstate.world_to_screen((score.from.x, score.from.y)),
            cstate.world_to_screen((score.to.x, score.to.y)),
        );
        frame.stroke(&path, dashed(edge));
    }
}

/// The pours, mechanical drawings and component anchors of a board bake
/// (decision 2): pours filled in their copper colour, drawings in their
/// mechanical layer's colour, an anchor as a cross in a ring with the
/// component's designator (mockup row 179). A slot is a cutout and is drawn
/// with them.
fn draw_board_items(
    frame: &mut canvas::Frame,
    cstate: &FootprintCanvasState,
    items: &SketchBoardItems,
) {
    for zone in &items.zones {
        let Some(layer) = zone.pcb_layer() else {
            continue;
        };
        let points = zone.outline.iter().map(|p| (p.x, p.y));
        let colour = board_layer_colour(layer);
        draw_region(frame, cstate, points, colour, REGION_FILL_ALPHA);
    }
    for drawing in &items.graphics {
        let Some(layer) = drawing.pcb_layer() else {
            continue;
        };
        let stroke = Stroke::default()
            .with_width(EDGE_WIDTH)
            .with_color(board_layer_colour(layer));
        if let Some(path) = graphic_path(cstate, drawing) {
            frame.stroke(&path, stroke);
        }
    }
    let theme = ResolvedTheme::from_canvas_colors(crate::styles::board_canvas_colors());
    let [r, g, b, a] = theme.color(ColorSlot::Selection);
    let mark = Color { r, g, b, a };
    for anchor in &items.anchors {
        let at = cstate.world_to_screen((anchor.position.x, anchor.position.y));
        let ring = Stroke::default().with_width(EDGE_WIDTH).with_color(mark);
        frame.stroke(&Path::circle(at, ANCHOR_RING), ring);
        frame.stroke(
            &Path::new(|cross| {
                cross.move_to(Point::new(at.x - ANCHOR_ARM, at.y));
                cross.line_to(Point::new(at.x + ANCHOR_ARM, at.y));
                cross.move_to(Point::new(at.x, at.y - ANCHOR_ARM));
                cross.line_to(Point::new(at.x, at.y + ANCHOR_ARM));
            }),
            ring,
        );
        frame.fill_text(canvas::Text {
            content: anchor.reference.clone(),
            position: Point::new(at.x - ANCHOR_ARM, at.y - ANCHOR_ARM),
            color: mark,
            size: iced::Pixels(TypeScale::DEFAULT.caption),
            align_x: iced::alignment::Horizontal::Right.into(),
            align_y: iced::alignment::Vertical::Bottom,
            ..canvas::Text::default()
        });
    }
}

/// A mechanical drawing's line, arc or circle on screen.
fn graphic_path(cstate: &FootprintCanvasState, drawing: &BoardGraphic) -> Option<Path> {
    let at = |p: signex_types::pcb::Point| (p.x, p.y);
    match drawing.kind()? {
        BoardGraphicKind::Line => {
            let (from, to) = (drawing.start?, drawing.end?);
            Some(Path::line(
                cstate.world_to_screen(at(from)),
                cstate.world_to_screen(at(to)),
            ))
        }
        BoardGraphicKind::Arc => Some(arc_path(
            cstate,
            at(drawing.center?),
            at(drawing.start?),
            at(drawing.end?),
            true,
        )),
        BoardGraphicKind::Circle => Some(Path::circle(
            cstate.world_to_screen(at(drawing.center?)),
            drawing.radius as f32 * cstate.scale,
        )),
        BoardGraphicKind::Rectangle | BoardGraphicKind::TextFrame => None,
    }
}

fn dashed(colour: Color) -> Stroke<'static> {
    Stroke {
        line_dash: LineDash {
            segments: DASHES,
            offset: 0,
        },
        ..Stroke::default().with_width(EDGE_WIDTH).with_color(colour)
    }
}

/// A closed polygon through `points`; `None` under three corners.
fn polygon_path(
    cstate: &FootprintCanvasState,
    points: impl IntoIterator<Item = (f64, f64)>,
) -> Option<Path> {
    let corners: Vec<Point> = points
        .into_iter()
        .map(|p| cstate.world_to_screen(p))
        .collect();
    let (first, rest) = corners.split_first()?;
    if rest.len() < 2 {
        return None;
    }
    Some(Path::new(|b| {
        b.move_to(*first);
        for &corner in rest {
            b.line_to(corner);
        }
        b.close();
    }))
}

/// A region in `colour`: filled at `fill_alpha`, outlined in full.
fn draw_region(
    frame: &mut canvas::Frame,
    cstate: &FootprintCanvasState,
    points: impl IntoIterator<Item = (f64, f64)>,
    colour: Color,
    fill_alpha: f32,
) {
    if let Some(path) = polygon_path(cstate, points) {
        frame.fill(&path, colour.scale_alpha(fill_alpha));
        frame.stroke(
            &path,
            Stroke::default().with_width(EDGE_WIDTH).with_color(colour),
        );
    }
}

/// The sketch's lines, arcs and circles in `ink`, at their solved places
/// (construction and centerlines dashed).
fn draw_faint_sketch(
    frame: &mut canvas::Frame,
    cstate: &FootprintCanvasState,
    sketch: &SketchData,
    state: &FootprintEditorState,
    ink: Color,
) {
    let at = |id: SketchEntityId| -> Option<(f64, f64)> {
        if let Some(solve) = state.last_solve.as_ref()
            && let Some(p) = point_xy(id, &solve.result.state, &solve.result.index, sketch)
        {
            return Some(p);
        }
        sketch
            .entities
            .iter()
            .find(|e| e.id == id)
            .and_then(|e| match e.kind {
                EntityKind::Point { x, y } => Some((x, y)),
                _ => None,
            })
    };
    for entity in &sketch.entities {
        let stroke = if entity.bake_skipped() {
            Stroke {
                line_dash: LineDash {
                    segments: DASHES,
                    offset: 0,
                },
                ..Stroke::default().with_width(FAINT_WIDTH).with_color(ink)
            }
        } else {
            Stroke::default().with_width(FAINT_WIDTH).with_color(ink)
        };
        let path = match entity.kind {
            EntityKind::Point { .. } => None,
            EntityKind::Line { start, end } => at(start)
                .zip(at(end))
                .map(|(a, b)| Path::line(cstate.world_to_screen(a), cstate.world_to_screen(b))),
            EntityKind::Circle { center, radius } => at(center).map(|c| {
                let radius = state
                    .last_solve
                    .as_ref()
                    .and_then(|s| circle_radius(entity.id, &s.result.state, &s.result.index))
                    .unwrap_or(radius);
                Path::circle(cstate.world_to_screen(c), radius as f32 * cstate.scale)
            }),
            EntityKind::Arc {
                center,
                start,
                end,
                sweep_ccw,
            } => match (at(center), at(start), at(end)) {
                (Some(c), Some(s), Some(e)) => Some(arc_path(cstate, c, s, e, sweep_ccw)),
                _ => None,
            },
        };
        if let Some(path) = path {
            frame.stroke(&path, stroke);
        }
    }
}

/// An arc about `c` from `s` to `e`, counter-clockwise when `ccw`, as a
/// polyline of [`ARC_STEPS`] segments.
fn arc_path(
    cstate: &FootprintCanvasState,
    c: (f64, f64),
    s: (f64, f64),
    e: (f64, f64),
    ccw: bool,
) -> Path {
    let radius = (s.0 - c.0).hypot(s.1 - c.1);
    let from = (s.1 - c.1).atan2(s.0 - c.0);
    let to = (e.1 - c.1).atan2(e.0 - c.0);
    let tau = std::f64::consts::TAU;
    let sweep = if ccw {
        (to - from).rem_euclid(tau)
    } else {
        -(from - to).rem_euclid(tau)
    };
    Path::new(|b| {
        b.move_to(cstate.world_to_screen(s));
        for i in 1..=ARC_STEPS {
            let theta = from + sweep * i as f64 / ARC_STEPS as f64;
            b.line_to(
                cstate.world_to_screen((c.0 + radius * theta.cos(), c.1 + radius * theta.sin())),
            );
        }
    })
}
