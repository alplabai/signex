//! The instances of a pattern, drawn at rest (mockup row 179e, mockup row
//! 191): a hole pattern shows every hole it bakes, dashed, at the bake's own
//! slots (`signex_bake::pattern_slots`), so an edited spacing moves them. A
//! slot the suppression list holds back keeps its place and draws faint and
//! struck through (B5), apart from the holes that bake. A linear pattern
//! draws its spacing and count as `name · value` pills on dimension lines,
//! the text its selected value glyphs show; a diagonal step draws its X
//! and Y spacing as two dimensions.

use iced::widget::canvas::{self, LineDash, Path, Stroke};
use iced::{Point, Vector};
use signex_bake::PatternSlot;
use signex_sketch::SketchData;
use signex_sketch::array::{Array, ArrayId, ArrayKind};
use signex_sketch::entity::EntityKind;
use signex_sketch::id::SketchEntityId;
use signex_sketch::solver::FullSolveOutput;
use signex_sketch::solver::state::point_xy;

use super::SELECTION;
use super::dimension_labels::{LabelInk, dimension_stroke, draw_value};
use crate::library::editor::footprint::canvas::FootprintCanvasState;
use crate::library::editor::footprint::canvas::glyph_layout::{
    GLYPH_TEXT, Glyph, GlyphFace, GlyphTarget, label_rect, pattern_value_label,
};
use crate::library::editor::footprint::sketch_mode::value_edit::MAX_DRAWN_SLOTS;
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
/// The gap between the last instance and the count pill, in text heights.
const COUNT_GAP_EM: f32 = 0.6;

/// Where a pattern places its source: the source's anchor and extent, and
/// every slot.
pub(in crate::library::editor::footprint::canvas) struct SourceSlots {
    /// The point the source is placed by, mm.
    pub anchor: (f64, f64),
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
    let anchor = source_anchor(sketch, solve, source)?;
    let slots = signex_bake::pattern_slots(sketch, solve, array.id, anchor, MAX_DRAWN_SLOTS)?;
    Some(SourceSlots {
        anchor,
        radius,
        slots,
    })
}

/// One further instance of a pattern of circles, as the drawing shows it.
#[derive(Debug, Clone, Copy, PartialEq)]
struct InstanceCircle {
    at: (f64, f64),
    radius: f64,
    /// On the suppression list: the slot stays, the bake leaves it out.
    suppressed: bool,
}

/// Every further instance a pattern of a Circle source places, suppressed
/// ones included; the source itself is left out. Empty for any other
/// source (a pad pattern draws as baked pads).
fn instance_slots(found: &SourceSlots) -> Vec<InstanceCircle> {
    let Some(radius) = found.radius else {
        return Vec::new();
    };
    found
        .slots
        .iter()
        .filter(|slot| (slot.i, slot.j) != (0, 0))
        .map(|slot| InstanceCircle {
            at: slot.at,
            radius,
            suppressed: slot.suppressed,
        })
        .collect()
}

/// One spacing dimension of a linear pattern, between two points, mm.
#[derive(Debug, Clone, PartialEq)]
struct SpacingDimension {
    field: PatternField,
    from: (f64, f64),
    to: (f64, f64),
    text: String,
}

/// A linear pattern's count and spacing as dimensions on the drawing
/// (mockup row 191): the spacing spans the source and the next instance,
/// its X and Y steps apart; the count sits past the last instance.
#[derive(Debug, Clone, PartialEq)]
struct LinearDimensions {
    array: ArrayId,
    /// The X step, then the Y step; a zero step draws none.
    spacing: Vec<SpacingDimension>,
    /// The source's slot and the next one, mm.
    step: ((f64, f64), (f64, f64)),
    /// The last slot, mm.
    last: (f64, f64),
    /// How far the source reaches from its anchor, mm (a circle's radius).
    clearance_mm: f64,
    count: String,
}

/// The dimensions of `array` when it is a linear pattern of at least two
/// instances; `None` otherwise.
fn linear_dimensions(
    array: &Array,
    sketch: &SketchData,
    found: &SourceSlots,
    text: &SketchCanvasText,
) -> Option<LinearDimensions> {
    if !matches!(array.kind, ArrayKind::Linear { .. }) {
        return None;
    }
    let first = found.slots.first()?.at;
    let next = found.slots.get(1)?.at;
    let corner = (next.0, first.1);
    let spacing = [
        (PatternField::SpacingX, first, corner),
        (PatternField::SpacingY, corner, next),
    ]
    .into_iter()
    .filter(|(_, from, to)| (to.0 - from.0).hypot(to.1 - from.1) > 1e-9)
    .map(|(field, from, to)| SpacingDimension {
        field,
        from,
        to,
        text: pattern_value_label(field, &array.kind, sketch, text),
    })
    .collect();
    Some(LinearDimensions {
        array: array.id,
        spacing,
        step: (first, next),
        last: found.slots.last()?.at,
        clearance_mm: found.radius.unwrap_or(0.0),
        count: pattern_value_label(PatternField::Count, &array.kind, sketch, text),
    })
}

pub(in crate::library::editor::footprint::canvas::draw) fn draw_pattern_instances(
    frame: &mut canvas::Frame,
    cstate: &FootprintCanvasState,
    sketch: &SketchData,
    state: &FootprintEditorState,
    conflict_colour: iced::Color,
) {
    let Some(solve) = state.last_solve.as_ref() else {
        return;
    };
    let ink = LabelInk::new(conflict_colour);
    for array in &sketch.arrays {
        let Some(found) = source_slots(array, sketch, solve) else {
            continue;
        };
        for slot in instance_slots(&found) {
            draw_instance_circle(frame, cstate, slot);
        }
        // A selected pattern shows its values as glyphs (`glyph_layout`).
        if state.sketch_overlay.selected_pattern != Some(array.id)
            && let Some(dims) = linear_dimensions(array, sketch, &found, &state.canvas_text)
        {
            draw_linear_dimensions(frame, &lay_out_linear(&dims, cstate), &ink);
        }
    }
}

/// The screen sides a linear pattern's spacing dimensions stand off its
/// instances on, one unit normal per dimension; empty for any other
/// pattern. The pattern glyph keeps to the other side (`glyph_layout`).
pub(in crate::library::editor::footprint::canvas) fn spacing_sides(
    array: &Array,
    sketch: &SketchData,
    state: &FootprintEditorState,
    cstate: &FootprintCanvasState,
) -> Vec<Vector> {
    let Some(solve) = state.last_solve.as_ref() else {
        return Vec::new();
    };
    source_slots(array, sketch, solve)
        .and_then(|found| linear_dimensions(array, sketch, &found, &state.canvas_text))
        .map(|dims| lay_out_linear(&dims, cstate).sides)
        .unwrap_or_default()
}

/// A kept instance as a dashed ring; a suppressed one fainter, finer
/// dashed and struck through.
fn draw_instance_circle(
    frame: &mut canvas::Frame,
    cstate: &FootprintCanvasState,
    slot: InstanceCircle,
) {
    let (alpha, dash) = if slot.suppressed {
        (SUPPRESSED_ALPHA, SUPPRESSED_DASH)
    } else {
        (INK_ALPHA, DASH)
    };
    let ring = Stroke {
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
    let c = cstate.world_to_screen(slot.at);
    let r = slot.radius as f32 * cstate.scale;
    frame.stroke(&Path::circle(c, r), ring);
    if slot.suppressed {
        let d = r * std::f32::consts::FRAC_1_SQRT_2;
        let strike = Stroke {
            line_dash: LineDash::default(),
            ..ring
        };
        frame.stroke(
            &Path::line(Point::new(c.x - d, c.y + d), Point::new(c.x + d, c.y - d)),
            strike,
        );
    }
}

/// The unit direction from `a` to `b` on screen and its normal; `None`
/// when they are under a pixel apart.
fn axes(a: Point, b: Point) -> Option<(Vector, Vector)> {
    let length = a.distance(b);
    (length >= 1.0).then(|| {
        let dir = Vector::new((b.x - a.x) / length, (b.y - a.y) / length);
        (dir, Vector::new(-dir.y, dir.x))
    })
}

/// A value pill at `at`, on the dimension line `span` when it has one.
fn value_glyph(target: GlyphTarget, text: &str, at: Point, span: Option<(Point, Point)>) -> Glyph {
    Glyph {
        target,
        face: GlyphFace::Value {
            text: text.to_string(),
            editing: false,
            invalid: false,
            reference: false,
            span,
        },
        anchor: at,
        rect: label_rect(at, text),
    }
}

/// A linear pattern's dimensions on screen: the extension lines, the value
/// pills, and the side each spacing dimension stands off on.
struct LinearLayout {
    extensions: Vec<(Point, Point)>,
    pills: Vec<Glyph>,
    sides: Vec<Vector>,
}

/// Each spacing dimension beside its instances, its pill on the line, and
/// the count pill past the last instance.
fn lay_out_linear(dims: &LinearDimensions, cstate: &FootprintCanvasState) -> LinearLayout {
    let clearance = dims.clearance_mm as f32 * cstate.scale;
    let off = clearance + GLYPH_TEXT * DIMENSION_OFFSET_EM;
    let reach = off + GLYPH_TEXT * EXTENSION_OVERSHOOT_EM;
    let mut layout = LinearLayout {
        extensions: Vec::new(),
        pills: Vec::new(),
        sides: Vec::new(),
    };
    for dim in &dims.spacing {
        let a = cstate.world_to_screen(dim.from);
        let b = cstate.world_to_screen(dim.to);
        let Some((_, normal)) = axes(a, b) else {
            continue;
        };
        let shift = |p: Point, by: f32| p + normal * by;
        for p in [a, b] {
            layout
                .extensions
                .push((shift(p, clearance), shift(p, reach)));
        }
        let span = (shift(a, off), shift(b, off));
        let mid = Point::new((span.0.x + span.1.x) / 2.0, (span.0.y + span.1.y) / 2.0);
        let target = GlyphTarget::PatternValue(dims.array, dim.field);
        layout
            .pills
            .push(value_glyph(target, &dim.text, mid, Some(span)));
        layout.sides.push(normal);
    }
    let step = (
        cstate.world_to_screen(dims.step.0),
        cstate.world_to_screen(dims.step.1),
    );
    if let Some((dir, _)) = axes(step.0, step.1) {
        let size = label_rect(Point::ORIGIN, &dims.count).size();
        let half = (dir.x.abs() * size.width + dir.y.abs() * size.height) / 2.0;
        let gap = clearance + GLYPH_TEXT * COUNT_GAP_EM + half;
        let at = cstate.world_to_screen(dims.last) + dir * gap;
        let target = GlyphTarget::PatternValue(dims.array, PatternField::Count);
        layout
            .pills
            .push(value_glyph(target, &dims.count, at, None));
    }
    layout
}

fn draw_linear_dimensions(frame: &mut canvas::Frame, layout: &LinearLayout, ink: &LabelInk) {
    let extension = dimension_stroke(ink, false, false);
    for &(from, to) in &layout.extensions {
        frame.stroke(&Path::line(from, to), extension);
    }
    for pill in &layout.pills {
        draw_value(frame, pill, ink, false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_sketch::Solver;
    use signex_sketch::array::NumberingScheme;
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
        source_slots(array, sketch, solve)
            .map(|found| instance_slots(&found))
            .unwrap_or_default()
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

    /// The en-US canvas words the labels below read.
    fn words() -> SketchCanvasText {
        SketchCanvasText {
            unit_mm: "mm".into(),
            value_unresolved: "?".into(),
            pattern_count: "count".into(),
            pattern_spacing_x: "spacing X".into(),
            pattern_spacing_y: "spacing Y".into(),
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

    fn dimensions(array: &Array, sketch: &SketchData) -> Option<LinearDimensions> {
        let found = source_slots(array, sketch, &solved(sketch)).expect("slots");
        linear_dimensions(array, sketch, &found, &words())
    }

    fn near(a: (f64, f64), b: (f64, f64)) -> bool {
        (a.0 - b.0).abs() < 1e-6 && (a.1 - b.1).abs() < 1e-6
    }

    /// Mockup row 191 (B5): a suppressed instance keeps its slot on the
    /// drawing, marked suppressed, and is not among the plain ghosts.
    #[test]
    fn a_suppressed_instance_keeps_its_slot_and_draws_as_suppressed() {
        let (mut sketch, mut array) = circle_grid();
        assert!(array.kind.set_instance_suppressed(1, 0, true));
        sketch.arrays = vec![array.clone()];
        let solve = solved(&sketch);

        let found = source_slots(&array, &sketch, &solve).expect("slots");
        let slots = instance_slots(&found);
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
    /// last, as `name · value` pills naming the parameters, the text the
    /// selected pattern's glyphs show (DECISIONS 2026-11-20).
    #[test]
    fn a_linear_pattern_draws_count_and_spacing_bound_to_parameters() {
        let (mut sketch, hole) = hole_sketch();
        sketch.parameters.insert("rows", "4");
        sketch.parameters.insert("pitch", "0.65mm");
        let array = linear(&mut sketch, hole, ["rows", "0mm", "pitch"]);

        let dims = dimensions(&array, &sketch).expect("dimensions");
        assert_eq!(dims.spacing.len(), 1, "a zero X step draws none: {dims:?}");
        let spacing = &dims.spacing[0];
        assert_eq!(spacing.field, PatternField::SpacingY);
        assert_eq!(spacing.text, "spacing Y = pitch · 0.65 mm");
        assert_eq!(dims.count, "count = rows · 4");
        assert!(near(spacing.from, (1.0, 2.0)), "{dims:?}");
        assert!(near(spacing.to, (1.0, 2.65)), "{dims:?}");
        assert!((dims.last.1 - (2.0 + 3.0 * 0.65)).abs() < 1e-6, "{dims:?}");
        assert!((dims.clearance_mm - 0.5).abs() < 1e-9);
    }

    /// A literal count and spacing show their field and value.
    #[test]
    fn a_literal_linear_pattern_shows_its_values() {
        let (mut sketch, hole) = hole_sketch();
        let array = linear(&mut sketch, hole, ["3", "2mm", "0mm"]);

        let dims = dimensions(&array, &sketch).expect("dimensions");
        assert_eq!(dims.spacing.len(), 1, "{dims:?}");
        assert_eq!(dims.spacing[0].text, "spacing X · 2.00 mm");
        assert_eq!(dims.count, "count · 3");
    }

    /// A diagonal step draws its X and Y spacing as two dimensions, each
    /// named by its own field and as long as the value it shows.
    #[test]
    fn a_diagonal_linear_pattern_draws_its_x_and_y_spacing_apart() {
        let (mut sketch, hole) = hole_sketch();
        let array = linear(&mut sketch, hole, ["2", "2mm", "1mm"]);

        let dims = dimensions(&array, &sketch).expect("dimensions");
        assert_eq!(dims.spacing.len(), 2, "{dims:?}");
        let (x, y) = (&dims.spacing[0], &dims.spacing[1]);
        assert_eq!(x.text, "spacing X · 2.00 mm");
        assert!(near(x.from, (1.0, 2.0)) && near(x.to, (3.0, 2.0)), "{x:?}");
        assert_eq!(y.text, "spacing Y · 1.00 mm");
        assert!(near(y.from, (3.0, 2.0)) && near(y.to, (3.0, 3.0)), "{y:?}");
    }

    #[test]
    fn a_grid_pattern_draws_no_linear_dimensions() {
        let (sketch, array) = circle_grid();
        assert_eq!(dimensions(&array, &sketch), None);
    }

    /// #3009, mockup row 126: at the default zoom a linear pattern's glyph
    /// ("L×4") clears its spacing and count pills, and selecting the
    /// pattern does not move it.
    #[test]
    fn a_linear_pattern_glyph_clears_its_dimension_pills() {
        use crate::library::editor::footprint::canvas::glyph_layout::sketch_glyphs;
        let (mut sketch, hole) = hole_sketch();
        let array = linear(&mut sketch, hole, ["4", "1.27mm", "0mm"]);
        let mut state = FootprintEditorState::empty();
        state.canvas_text = words();
        state.last_solve = Some(solved(&sketch));
        let cstate = FootprintCanvasState::with_view(30.0, Point::new(200.0, 200.0));
        let glyph_of = |state: &FootprintEditorState| {
            sketch_glyphs(&sketch, state, &cstate)
                .into_iter()
                .find(|g| g.target == GlyphTarget::Pattern(array.id))
                .expect("the pattern glyph")
        };
        let glyph = glyph_of(&state);
        let dims = dimensions(&array, &sketch).expect("dimensions");
        let layout = lay_out_linear(&dims, &cstate);
        assert_eq!(layout.pills.len(), 2, "spacing X and count: {dims:?}");
        assert_eq!(layout.sides, vec![Vector::new(0.0, 1.0)], "spacing below");
        for pill in &layout.pills {
            assert!(
                glyph.rect.intersection(&pill.rect).is_none(),
                "{glyph:?} covers {pill:?}"
            );
        }
        assert!(
            glyph.anchor.y < 200.0 + 2.0 * 30.0,
            "above the row: {glyph:?}"
        );

        state.sketch_overlay.selected_pattern = Some(array.id);
        assert_eq!(glyph_of(&state).anchor, glyph.anchor, "the glyph stays put");
    }

    /// #3009 on the app path: selecting the pattern through
    /// `Signex::update` keeps its glyph where it was at rest.
    #[test]
    fn selecting_a_linear_pattern_in_the_app_keeps_its_glyph_in_place() {
        use std::path::PathBuf;

        use crate::app::{FootprintDocument, Message, Signex, TabInfo, TabKind};
        use crate::library::editor::footprint::EditorMode;
        use crate::library::editor::footprint::canvas::glyph_layout::sketch_glyphs;
        use crate::library::editor::footprint::sketch_mode::SketchCanvasMsg;
        use crate::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
        use signex_library::{Footprint, FootprintFile};

        let (mut sketch, hole) = hole_sketch();
        let array = linear(&mut sketch, hole, ["4", "1.27mm", "0mm"]);
        let solve = solved(&sketch);
        let mut fp = Footprint::empty("pattern");
        fp.sketch = Some(sketch);
        let path = PathBuf::from("pattern-glyph.snxfpt");
        let mut editor = FootprintDocument::new(path.clone(), FootprintFile::from_footprint(fp));
        editor.state.mode = EditorMode::Sketch;
        editor.state.last_solve = Some(solve);
        let (mut app, _task) = Signex::new();
        app.document_state
            .footprint_editors
            .insert(path.clone(), editor);
        app.document_state.tabs.push(TabInfo {
            title: "pattern".into(),
            path: path.clone(),
            project_id: None,
            kind: TabKind::FootprintEditor(path.clone()),
        });
        app.document_state.active_tab = 0;
        let cstate = FootprintCanvasState::with_view(30.0, Point::new(200.0, 200.0));
        let glyph = |app: &Signex| {
            let editor = &app.document_state.footprint_editors[&path];
            let sketch = editor.primitive().sketch.as_ref().expect("the sketch");
            sketch_glyphs(sketch, &editor.state, &cstate)
                .into_iter()
                .find(|g| g.target == GlyphTarget::Pattern(array.id))
                .expect("the pattern glyph")
        };
        let at_rest = glyph(&app);
        drop(
            app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
                path: path.clone(),
                msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchCanvas(
                    SketchCanvasMsg::PatternSelected(Some(array.id)),
                )),
            })),
        );
        let selected = glyph(&app);
        assert!(
            matches!(selected.face, GlyphFace::Pattern { selected: true, .. }),
            "{selected:?}"
        );
        assert_eq!(selected.anchor, at_rest.anchor, "the glyph stays put");
    }

    /// With no solve there are no spacing sides, so the glyph keeps its
    /// place left of and below the source.
    #[test]
    fn without_a_solve_a_pattern_has_no_spacing_sides() {
        let (mut sketch, hole) = hole_sketch();
        let array = linear(&mut sketch, hole, ["4", "1.27mm", "0mm"]);
        let state = FootprintEditorState::empty();
        let cstate = FootprintCanvasState::with_view(30.0, Point::new(200.0, 200.0));
        assert!(spacing_sides(&array, &sketch, &state, &cstate).is_empty());
    }
}
