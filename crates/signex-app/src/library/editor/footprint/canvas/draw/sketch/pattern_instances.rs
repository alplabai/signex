//! The instances of a pattern, drawn at rest (mockup row 179e, mockup row
//! 191): a hole pattern shows every hole it bakes, dashed, at the bake's own
//! slots (`signex_bake::pattern_slots`), so an edited spacing moves them. A
//! slot the suppression list holds back keeps its place and draws faint and
//! struck through (B5), apart from the holes that bake. A linear pattern
//! draws its spacing and count as dimensions on the drawing, named by the
//! parameters they are bound to.

use iced::Point;
use iced::widget::canvas::{self, LineDash, Path, Stroke};
use signex_bake::PatternSlot;
use signex_sketch::SketchData;
use signex_sketch::array::{Array, ArrayKind};
use signex_sketch::entity::EntityKind;
use signex_sketch::id::SketchEntityId;
use signex_sketch::solver::FullSolveOutput;
use signex_sketch::solver::state::point_xy;

use super::SELECTION;
use super::arrows::draw_arrow_head;
use super::sketch_ink::SketchInk;
use crate::library::editor::footprint::canvas::FootprintCanvasState;
use crate::library::editor::footprint::canvas::glyph_layout::GLYPH_TEXT;
use crate::library::editor::footprint::sketch_mode::value_edit::{
    MAX_DRAWN_SLOTS, bound_parameter, pattern_value_text,
};
use crate::library::editor::footprint::state::FootprintEditorState;
use crate::library::editor::footprint::state::canvas_text::SketchCanvasText;
use crate::library::editor::footprint::state::sketch_overlay::PatternField;

const STROKE_PX: f32 = 1.2;
const DASH: &[f32] = &[4.0, 3.0];
const INK_ALPHA: f32 = 0.7;
/// A suppressed slot: fainter, with a finer dash.
const SUPPRESSED_ALPHA: f32 = 0.35;
const SUPPRESSED_DASH: &[f32] = &[2.0, 3.0];
/// How far a linear pattern's spacing dimension stands off its instances,
/// past the source's own extent, in text heights.
const DIMENSION_OFFSET_EM: f32 = 1.6;
/// How far an extension line runs past its dimension line, in text heights.
const EXTENSION_OVERSHOOT_EM: f32 = 0.3;
/// Arrowhead length of the spacing dimension, in text heights.
const ARROW_HEAD_EM: f32 = 0.4;
/// Width of a dimension and its extension lines, in px.
const DIMENSION_PX: f32 = 1.0;

/// Where a pattern places its source: the source's extent and every slot.
pub(in crate::library::editor::footprint::canvas) struct SourceSlots {
    /// The source circle's radius, mm; `None` for any other source.
    pub radius: Option<f64>,
    /// Every slot the bake places or the suppression list holds back, the
    /// source's own `(0, 0)` first, each placing the source's anchor.
    pub slots: Vec<PatternSlot>,
}

/// The point a source entity is placed by: a Point itself, a Line's
/// midpoint, a Circle's or Arc's centre.
fn source_anchor(
    sketch: &SketchData,
    solve: &FullSolveOutput,
    source: SketchEntityId,
) -> Option<(f64, f64)> {
    let at = |id| point_xy(id, &solve.result.state, &solve.result.index, sketch);
    match sketch.entities.iter().find(|e| e.id == source)?.kind {
        EntityKind::Point { .. } => at(source),
        EntityKind::Line { start, end } => {
            let (a, b) = (at(start)?, at(end)?);
            Some(((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0))
        }
        EntityKind::Circle { center, .. } | EntityKind::Arc { center, .. } => at(center),
    }
}

/// The slots of `array`, from the bake's own walk. `None` when the array
/// does not expand or its source has no place.
pub(in crate::library::editor::footprint::canvas) fn source_slots(
    array: &Array,
    sketch: &SketchData,
    solve: &FullSolveOutput,
) -> Option<SourceSlots> {
    let source = array.kind.source();
    let radius = sketch
        .entities
        .iter()
        .find(|e| e.id == source)
        .and_then(|e| match e.kind {
            EntityKind::Circle { radius, .. } => Some(radius),
            _ => None,
        });
    let at = source_anchor(sketch, solve, source)?;
    let slots = signex_bake::pattern_slots(sketch, solve, array.id, at, MAX_DRAWN_SLOTS)?;
    Some(SourceSlots { radius, slots })
}

/// One further instance of a pattern of circles, as the drawing shows it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::library::editor::footprint::canvas) struct InstanceCircle {
    pub at: (f64, f64),
    pub radius: f64,
    /// On the suppression list: the slot stays, the bake leaves it out.
    pub suppressed: bool,
}

/// Every further instance `array` of a Circle source places, suppressed
/// ones included; the source itself is left out. Empty for any other
/// source (a pad pattern draws as baked pads).
pub(in crate::library::editor::footprint::canvas) fn instance_slots(
    array: &Array,
    sketch: &SketchData,
    solve: &FullSolveOutput,
) -> Vec<InstanceCircle> {
    let Some(SourceSlots {
        radius: Some(radius),
        slots,
    }) = source_slots(array, sketch, solve)
    else {
        return Vec::new();
    };
    slots
        .into_iter()
        .filter(|slot| (slot.i, slot.j) != (0, 0))
        .map(|slot| InstanceCircle {
            at: slot.at,
            radius,
            suppressed: slot.suppressed,
        })
        .collect()
}

/// A linear pattern's count and spacing as dimensions on the drawing
/// (mockup row 191): the spacing spans the source and the first further
/// instance, the count sits past the last one. A value bound to a
/// parameter is named by it ("pitch = 0.65 mm").
#[derive(Debug, Clone, PartialEq)]
pub(in crate::library::editor::footprint::canvas) struct LinearDimensions {
    /// The source's slot and the next one, mm.
    pub span: ((f64, f64), (f64, f64)),
    /// The last slot, mm.
    pub last: (f64, f64),
    /// How far the source reaches from its anchor, mm (a circle's radius).
    pub clearance_mm: f64,
    pub spacing: String,
    pub count: String,
}

/// A pattern value as a dimension shows it: `name = value` when bound to
/// a parameter, else the value.
fn bound_text(
    sketch: &SketchData,
    field: PatternField,
    kind: &ArrayKind,
    text: &SketchCanvasText,
) -> String {
    let source = field.expr(kind).unwrap_or_default();
    let value = pattern_value_text(sketch, field, source, text);
    match bound_parameter(source, sketch) {
        Some(param) => format!("{param} = {value}"),
        None => value,
    }
}

/// The dimensions of `array` when it is a linear pattern of at least two
/// instances; `None` otherwise.
pub(in crate::library::editor::footprint::canvas) fn linear_dimensions(
    array: &Array,
    sketch: &SketchData,
    solve: &FullSolveOutput,
    text: &SketchCanvasText,
) -> Option<LinearDimensions> {
    if !matches!(array.kind, ArrayKind::Linear { .. }) {
        return None;
    }
    let SourceSlots { radius, slots } = source_slots(array, sketch, solve)?;
    let (first, next, last) = (slots.first()?, slots.get(1)?, slots.last()?);
    let step = (next.at.0 - first.at.0, next.at.1 - first.at.1);
    let spacing = [
        (PatternField::SpacingX, step.0),
        (PatternField::SpacingY, step.1),
    ]
    .into_iter()
    .filter(|(_, d)| d.abs() > 1e-9)
    .map(|(field, _)| bound_text(sketch, field, &array.kind, text))
    .collect::<Vec<_>>()
    .join(", ");
    let count_expr = PatternField::Count.expr(&array.kind).unwrap_or_default();
    let count = bound_text(sketch, PatternField::Count, &array.kind, text);
    let count = match bound_parameter(count_expr, sketch) {
        Some(_) => count,
        None => format!("\u{00D7}{count}"),
    };
    Some(LinearDimensions {
        span: (first.at, next.at),
        last: last.at,
        clearance_mm: radius.unwrap_or(0.0),
        spacing,
        count,
    })
}

pub(in crate::library::editor::footprint::canvas::draw) fn draw_pattern_instances(
    frame: &mut canvas::Frame,
    cstate: &FootprintCanvasState,
    sketch: &SketchData,
    state: &FootprintEditorState,
) {
    let Some(solve) = state.last_solve.as_ref() else {
        return;
    };
    let ring = |alpha: f32, dash: &'static [f32]| Stroke {
        line_dash: LineDash {
            segments: dash,
            offset: 0,
        },
        ..Stroke::default()
            .with_width(STROKE_PX)
            .with_color(iced::Color {
                a: alpha,
                ..SELECTION
            })
    };
    let kept = ring(INK_ALPHA, DASH);
    let held_back = ring(SUPPRESSED_ALPHA, SUPPRESSED_DASH);
    for array in &sketch.arrays {
        for slot in instance_slots(array, sketch, solve) {
            let c = cstate.world_to_screen(slot.at);
            let r = slot.radius as f32 * cstate.scale;
            if !slot.suppressed {
                frame.stroke(&Path::circle(c, r), kept);
                continue;
            }
            frame.stroke(&Path::circle(c, r), held_back);
            let d = r * std::f32::consts::FRAC_1_SQRT_2;
            let strike = Stroke {
                line_dash: LineDash::default(),
                ..held_back
            };
            frame.stroke(
                &Path::line(Point::new(c.x - d, c.y + d), Point::new(c.x + d, c.y - d)),
                strike,
            );
        }
        // A selected pattern shows its values as glyphs (`glyph_layout`).
        if state.sketch_overlay.selected_pattern != Some(array.id)
            && let Some(dims) = linear_dimensions(array, sketch, solve, &state.canvas_text)
        {
            draw_linear_dimensions(frame, cstate, &dims);
        }
    }
}

/// The spacing dimension beside the first two instances and the count
/// past the last one.
fn draw_linear_dimensions(
    frame: &mut canvas::Frame,
    cstate: &FootprintCanvasState,
    dims: &LinearDimensions,
) {
    let ink = SketchInk::board();
    let a = cstate.world_to_screen(dims.span.0);
    let b = cstate.world_to_screen(dims.span.1);
    let length = a.distance(b);
    if length < 1.0 {
        return;
    }
    let dir = ((b.x - a.x) / length, (b.y - a.y) / length);
    let normal = (-dir.1, dir.0);
    let clearance = dims.clearance_mm as f32 * cstate.scale;
    let off = clearance + GLYPH_TEXT * DIMENSION_OFFSET_EM;
    let shift = |p: Point, by: f32| Point::new(p.x + normal.0 * by, p.y + normal.1 * by);
    let stroke = Stroke::default()
        .with_width(DIMENSION_PX)
        .with_color(ink.dimension);
    let reach = off + GLYPH_TEXT * EXTENSION_OVERSHOOT_EM;
    for p in [a, b] {
        frame.stroke(&Path::line(shift(p, clearance), shift(p, reach)), stroke);
    }
    let (da, db) = (shift(a, off), shift(b, off));
    frame.stroke(&Path::line(da, db), stroke);
    draw_arrow_head(frame, da, db, GLYPH_TEXT * ARROW_HEAD_EM, stroke);
    draw_arrow_head(frame, db, da, GLYPH_TEXT * ARROW_HEAD_EM, stroke);
    let mid = Point::new((da.x + db.x) / 2.0, (da.y + db.y) / 2.0);
    dimension_text(frame, shift(mid, GLYPH_TEXT), &dims.spacing, ink.label);
    let last = cstate.world_to_screen(dims.last);
    let count_at = Point::new(last.x + dir.0 * off, last.y + dir.1 * off);
    dimension_text(frame, count_at, &dims.count, ink.label);
}

fn dimension_text(frame: &mut canvas::Frame, at: Point, content: &str, color: iced::Color) {
    frame.fill_text(canvas::Text {
        content: content.to_string(),
        position: at,
        color,
        size: iced::Pixels(GLYPH_TEXT),
        align_x: iced::alignment::Horizontal::Center.into(),
        align_y: iced::alignment::Vertical::Center,
        ..canvas::Text::default()
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_sketch::Solver;
    use signex_sketch::array::{ArrayId, NumberingScheme};
    use signex_sketch::entity::Entity;
    use signex_sketch::plane::{Plane, PlaneId, PlaneKind};

    /// The instances that draw as plain ghosts: where `array` of a Circle
    /// source places its other circles, with their radius, suppressed
    /// slots left out.
    fn instance_circles(
        array: &Array,
        sketch: &SketchData,
        solve: &FullSolveOutput,
    ) -> Vec<((f64, f64), f64)> {
        instance_slots(array, sketch, solve)
            .into_iter()
            .filter(|c| !c.suppressed)
            .map(|c| (c.at, c.radius))
            .collect()
    }

    fn circle_grid() -> (SketchData, Array) {
        let (mut sketch, hole) = hole_sketch();
        let array = Array {
            id: ArrayId::new(),
            kind: ArrayKind::Grid {
                source: hole,
                nx_expr: "2".into(),
                ny_expr: "2".into(),
                dx_expr: "10mm".into(),
                dy_expr: "5mm".into(),
                depopulation: None,
            },
            numbering: NumberingScheme::default(),
        };
        sketch.arrays.push(array.clone());
        (sketch, array)
    }

    /// Mockup row 179e: a 2 x 2 hole pattern draws its three other holes
    /// at the spacing, each with the source's radius.
    #[test]
    fn a_circle_pattern_places_its_other_instances() {
        let (sketch, array) = circle_grid();
        let params = signex_sketch::parameter::resolve(&sketch.parameters).expect("resolves");
        let solve = Solver::default().solve(&sketch, &params).expect("solves");
        let circles = instance_circles(&array, &sketch, &solve);
        let mut centres: Vec<(f64, f64)> = circles.iter().map(|(at, _)| *at).collect();
        centres.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
        let want = [(1.0, 7.0), (11.0, 2.0), (11.0, 7.0)];
        assert_eq!(centres.len(), want.len(), "{centres:?}");
        for (got, want) in centres.iter().zip(want) {
            assert!(
                (got.0 - want.0).abs() < 1e-6 && (got.1 - want.1).abs() < 1e-6,
                "{got:?} is not {want:?}"
            );
        }
        assert!(circles.iter().all(|(_, r)| (*r - 0.5).abs() < 1e-9));
    }

    /// A Circle of radius 0.5 centred on (1, 2), and its id.
    fn hole_sketch() -> (SketchData, SketchEntityId) {
        let plane = PlaneId::new();
        let mut sketch = SketchData::default();
        sketch.planes.push(Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        });
        let (center, hole) = (SketchEntityId::new(), SketchEntityId::new());
        sketch.entities.push(Entity::new(
            center,
            plane,
            EntityKind::Point { x: 1.0, y: 2.0 },
        ));
        sketch.entities.push(Entity::new(
            hole,
            plane,
            EntityKind::Circle {
                center,
                radius: 0.5,
            },
        ));
        (sketch, hole)
    }

    fn solved(sketch: &SketchData) -> FullSolveOutput {
        let params = signex_sketch::parameter::resolve(&sketch.parameters).expect("resolves");
        Solver::default().solve(sketch, &params).expect("solves")
    }

    fn words() -> SketchCanvasText {
        SketchCanvasText {
            unit_mm: "mm".into(),
            value_unresolved: "?".into(),
            ..SketchCanvasText::default()
        }
    }

    fn linear(sketch: &mut SketchData, source: SketchEntityId, exprs: [&str; 3]) -> Array {
        let [count, dx, dy] = exprs;
        let array = Array {
            id: ArrayId::new(),
            kind: ArrayKind::Linear {
                source,
                count_expr: count.into(),
                dx_expr: dx.into(),
                dy_expr: dy.into(),
            },
            numbering: NumberingScheme::default(),
        };
        sketch.arrays.push(array.clone());
        array
    }

    /// Mockup row 191 (B5): a suppressed instance keeps its slot on the
    /// drawing, marked suppressed, and is not among the plain ghosts.
    #[test]
    fn a_suppressed_instance_keeps_its_slot_and_draws_as_suppressed() {
        let (mut sketch, mut array) = circle_grid();
        assert!(array.kind.set_instance_suppressed(1, 0, true));
        sketch.arrays = vec![array.clone()];
        let solve = solved(&sketch);

        let slots = instance_slots(&array, &sketch, &solve);
        assert_eq!(slots.len(), 3, "the suppressed slot stays: {slots:?}");
        let held_back: Vec<&InstanceCircle> = slots.iter().filter(|s| s.suppressed).collect();
        assert_eq!(held_back.len(), 1, "{slots:?}");
        let at = held_back[0].at;
        assert!(
            (at.0 - 11.0).abs() < 1e-6 && (at.1 - 2.0).abs() < 1e-6,
            "{at:?}"
        );
        assert_eq!(
            instance_circles(&array, &sketch, &solve).len(),
            2,
            "only the kept instances draw as plain ghosts"
        );
    }

    /// Mockup row 191: a linear pattern bound to `rows` and `pitch` draws
    /// its spacing between the first two instances and its count past the
    /// last, each named by its parameter.
    #[test]
    fn a_linear_pattern_draws_count_and_spacing_bound_to_parameters() {
        let (mut sketch, hole) = hole_sketch();
        sketch.parameters.insert("rows", "4");
        sketch.parameters.insert("pitch", "0.65mm");
        let array = linear(&mut sketch, hole, ["rows", "0mm", "pitch"]);
        let solve = solved(&sketch);

        let dims = linear_dimensions(&array, &sketch, &solve, &words()).expect("dimensions");
        assert_eq!(dims.spacing, "pitch = 0.65 mm");
        assert_eq!(dims.count, "rows = 4");
        let ((x0, y0), (x1, y1)) = dims.span;
        assert!(
            (x0 - 1.0).abs() < 1e-6 && (y0 - 2.0).abs() < 1e-6,
            "{dims:?}"
        );
        assert!(
            (x1 - 1.0).abs() < 1e-6 && (y1 - 2.65).abs() < 1e-6,
            "{dims:?}"
        );
        assert!((dims.last.1 - (2.0 + 3.0 * 0.65)).abs() < 1e-6, "{dims:?}");
        assert!((dims.clearance_mm - 0.5).abs() < 1e-9);
    }

    /// A literal count and spacing show their values; the count keeps its
    /// times sign.
    #[test]
    fn a_literal_linear_pattern_shows_its_values() {
        let (mut sketch, hole) = hole_sketch();
        let array = linear(&mut sketch, hole, ["3", "2mm", "0mm"]);
        let solve = solved(&sketch);

        let dims = linear_dimensions(&array, &sketch, &solve, &words()).expect("dimensions");
        assert_eq!(dims.spacing, "2.00 mm");
        assert_eq!(dims.count, "\u{00D7}3");
    }

    #[test]
    fn a_grid_pattern_draws_no_linear_dimensions() {
        let (sketch, array) = circle_grid();
        let solve = solved(&sketch);
        assert_eq!(linear_dimensions(&array, &sketch, &solve, &words()), None);
    }
}
