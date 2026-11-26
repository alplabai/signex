//! Where the sketch canvas puts each constraint mark, dimension value,
//! pattern glyph, mirror glyph and pattern instance. One pure layout, read
//! by the draw pass and by the hit test, so a click lands on what is drawn.
//! The marks are the mockups' (mockup row 187, mockup row 188): H, V, ∥, ⊥, =, ◡, ⇔, ⌖, a dot
//! for coincident; a dimension shows its value instead, and a reference
//! (driven) dimension the value it measures.

use std::collections::HashMap;

use iced::{Point, Rectangle, Size, Vector};
use signex_sketch::SketchData;
use signex_sketch::array::{Array, ArrayId, ArrayKind};
use signex_sketch::constraint::{Constraint, ConstraintKind};
use signex_sketch::entity::EntityKind;
use signex_sketch::id::{ConstraintId, SketchEntityId};

use super::FootprintCanvasState;
use super::draw::spacing_sides;
use crate::library::editor::footprint::sketch_mode::value_edit;
use crate::library::editor::footprint::state::FootprintEditorState;
use crate::library::editor::footprint::state::canvas_text::SketchCanvasText;
use crate::library::editor::footprint::state::sketch_overlay::{PatternField, ValueTarget};

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;

/// Text size of every value and pattern glyph on the drawing.
pub(crate) const GLYPH_TEXT: f32 = TS.body;
/// Text size of a constraint mark (H, V, ∥ …): a step under the values
/// beside it, as the mockups' marks sit under their labels.
pub(crate) const MARK_TEXT: f32 = TS.section;
/// How far a mark sits right of and above its anchor: half a value's text
/// height, so it clears the point it belongs to.
const MARK_NUDGE: f32 = GLYPH_TEXT * 0.5;
/// Height of a mark's pill, in mark text heights; a one-character pill is
/// square.
const MARK_HEIGHT_EM: f32 = 1.45;
/// Extra pill width per character past the first, in mark text heights.
const MARK_EXTRA_EM: f32 = 0.55;
/// Average advance of a glyph character, in ems; sizes a label before it
/// is drawn (the canvas cannot measure text).
const ADVANCE_EM: f32 = 0.62;
/// Height of a value or pattern label, in text heights.
const LABEL_HEIGHT_EM: f32 = 1.7;
/// Horizontal padding inside a label, in text heights.
const LABEL_PAD_EM: f32 = 0.6;
/// How far a pattern glyph sits left of and below its source, in text
/// heights; further glyphs of the same source stack below it. A mirror
/// glyph sits the same distance right of and above its line.
const PATTERN_OFFSET_EM: f32 = 2.2;
/// Radius of an instance slot's click target, in text heights.
const INSTANCE_RADIUS_EM: f32 = 0.8;

/// The coincident mark: drawn as a dot, not a pill.
pub(crate) const COINCIDENT_MARK: &str = "\u{25CF}";
/// The point-on-line and point-on-arc mark.
const POINT_ON_MARK: &str = "\u{2503}\u{00B7}";
/// The distance mark (a dimension draws its value instead).
const DISTANCE_MARK: &str = "\u{2194}";
/// The tangent mark.
const TANGENT_MARK: &str = "\u{25E1}";
/// The symmetric mark.
const SYMMETRIC_MARK: &str = "\u{21D4}";

/// What a glyph stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GlyphTarget {
    Constraint(ConstraintId),
    /// A pattern, or a pattern mirror (its id is a pattern-object id).
    Pattern(ArrayId),
    PatternValue(ArrayId, PatternField),
    Instance {
        array: ArrayId,
        i: u32,
        j: u32,
    },
}

/// How a glyph is drawn.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum GlyphFace {
    /// A relation's mark in a small upright pill; the coincident mark is
    /// a dot.
    Mark { text: &'static str },
    /// A value on the drawing (`name · value`), or the text being typed.
    Value {
        text: String,
        editing: bool,
        invalid: bool,
        /// A reference (driven) dimension: dashed, in the reference ink,
        /// showing the value it measures.
        reference: bool,
        /// The dimension line, from one measured end to the other, in
        /// canvas pixels; `None` for an angle and a pattern value.
        span: Option<(Point, Point)>,
    },
    /// A pattern's glyph ("L×4", "P×4 of L×4"), or a mirror's ("⇔ L×4").
    Pattern { text: String, selected: bool },
    /// One instance slot of the selected pattern.
    Instance { suppressed: bool },
}

/// One glyph: what it is, how it draws, and where.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Glyph {
    pub target: GlyphTarget,
    pub face: GlyphFace,
    /// The point the glyph belongs to, in canvas pixels.
    pub anchor: Point,
    /// What a click must hit, in canvas pixels.
    pub rect: Rectangle,
}

impl Glyph {
    /// The constraint this glyph is, if it is one.
    pub(crate) fn constraint(&self) -> Option<ConstraintId> {
        match self.target {
            GlyphTarget::Constraint(id) => Some(id),
            _ => None,
        }
    }

    /// How a hit on this glyph ranks against an overlapping one: values
    /// and patterns first, then instance slots, then marks.
    fn hit_rank(&self) -> u8 {
        match self.face {
            GlyphFace::Value { .. } | GlyphFace::Pattern { .. } => 0,
            GlyphFace::Instance { .. } => 1,
            GlyphFace::Mark { .. } => 2,
        }
    }
}

/// The glyph under `p`, if any; overlapping glyphs rank by kind.
pub(crate) fn glyph_at(glyphs: &[Glyph], p: Point) -> Option<&Glyph> {
    glyphs
        .iter()
        .filter(|g| g.rect.contains(p))
        .min_by_key(|g| g.hit_rank())
}

/// A label `text` long, centred on `anchor`.
pub(in crate::library::editor::footprint::canvas) fn label_rect(
    anchor: Point,
    text: &str,
) -> Rectangle {
    let width =
        text.chars().count() as f32 * GLYPH_TEXT * ADVANCE_EM + 2.0 * GLYPH_TEXT * LABEL_PAD_EM;
    let height = GLYPH_TEXT * LABEL_HEIGHT_EM;
    Rectangle::new(
        Point::new(anchor.x - width / 2.0, anchor.y - height / 2.0),
        Size::new(width, height),
    )
}

/// The pill of mark `text`, right of and above `anchor`.
fn mark_rect(anchor: Point, text: &str) -> Rectangle {
    let height = MARK_TEXT * MARK_HEIGHT_EM;
    let extra = text.chars().count().saturating_sub(1) as f32;
    let width = height + extra * MARK_TEXT * MARK_EXTRA_EM;
    Rectangle::new(
        Point::new(anchor.x + MARK_NUDGE, anchor.y - MARK_NUDGE - height / 2.0),
        Size::new(width, height),
    )
}

/// Where sketch points are: the last solve, else as authored.
struct Lookup<'a> {
    sketch: &'a SketchData,
    state: &'a FootprintEditorState,
}

impl Lookup<'_> {
    fn point(&self, id: SketchEntityId) -> Option<(f64, f64)> {
        if let Some(solve) = self.state.last_solve.as_ref()
            && let Some(p) = signex_sketch::solver::state::point_xy(
                id,
                &solve.result.state,
                &solve.result.index,
                self.sketch,
            )
        {
            return Some(p);
        }
        match self.kind(id)? {
            EntityKind::Point { x, y } => Some((*x, *y)),
            _ => None,
        }
    }

    fn kind(&self, id: SketchEntityId) -> Option<&EntityKind> {
        self.sketch
            .entities
            .iter()
            .find(|e| e.id == id)
            .map(|e| &e.kind)
    }

    fn line_ends(&self, id: SketchEntityId) -> Option<(SketchEntityId, SketchEntityId)> {
        match self.kind(id)? {
            EntityKind::Line { start, end } => Some((*start, *end)),
            _ => None,
        }
    }

    /// The line's two endpoints, or nothing.
    fn line_points(&self, id: SketchEntityId) -> Vec<SketchEntityId> {
        self.line_ends(id)
            .map(|(s, e)| vec![s, e])
            .unwrap_or_default()
    }

    /// Both lines' endpoints.
    fn two_lines(&self, a: SketchEntityId, b: SketchEntityId) -> Vec<SketchEntityId> {
        let mut points = self.line_points(a);
        points.extend(self.line_points(b));
        points
    }

    fn arc_center(&self, id: SketchEntityId) -> Option<SketchEntityId> {
        match self.kind(id)? {
            EntityKind::Arc { center, .. } => Some(*center),
            _ => None,
        }
    }

    fn circle_center(&self, id: SketchEntityId) -> Option<SketchEntityId> {
        match self.kind(id)? {
            EntityKind::Circle { center, .. } => Some(*center),
            _ => None,
        }
    }

    /// A circle's radius at the last solve, if there is one.
    fn solved_radius(&self, id: SketchEntityId) -> Option<f64> {
        let solve = self.state.last_solve.as_ref()?;
        signex_sketch::solver::state::circle_radius(id, &solve.result.state, &solve.result.index)
    }

    /// Where an entity is drawn from: a point itself, a line's midpoint,
    /// a circle's or arc's centre.
    fn anchor(&self, id: SketchEntityId) -> Option<(f64, f64)> {
        match self.kind(id)? {
            EntityKind::Point { .. } => self.point(id),
            EntityKind::Line { start, end } => {
                let (a, b) = (self.point(*start)?, self.point(*end)?);
                Some(((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0))
            }
            EntityKind::Circle { center, .. } | EntityKind::Arc { center, .. } => {
                self.point(*center)
            }
        }
    }
}

/// `head` followed by `tail`.
fn with(mut head: Vec<SketchEntityId>, tail: Vec<SketchEntityId>) -> Vec<SketchEntityId> {
    head.extend(tail);
    head
}

/// A constraint's mark and the points its glyph sits among.
fn constraint_parts(kind: &ConstraintKind, l: &Lookup<'_>) -> (&'static str, Vec<SketchEntityId>) {
    use ConstraintKind::*;
    match kind {
        Coincident { p1, p2 } => (COINCIDENT_MARK, vec![*p1, *p2]),
        PointOnLine { point, line } => (POINT_ON_MARK, with(vec![*point], l.line_points(*line))),
        PointOnArc { point, arc } => {
            let refs = match l.kind(*arc) {
                Some(EntityKind::Arc {
                    center, start, end, ..
                }) => vec![*center, *start, *end],
                _ => Vec::new(),
            };
            (POINT_ON_MARK, with(vec![*point], refs))
        }
        Horizontal { line } => ("H", l.line_points(*line)),
        Vertical { line } => ("V", l.line_points(*line)),
        Parallel { l1, l2 } => ("\u{2225}", l.two_lines(*l1, *l2)),
        Perpendicular { l1, l2 } => ("\u{22A5}", l.two_lines(*l1, *l2)),
        DistancePtPt { p1, p2, .. } => (DISTANCE_MARK, vec![*p1, *p2]),
        DistancePtLine { point, line, .. } => {
            (DISTANCE_MARK, with(vec![*point], l.line_points(*line)))
        }
        DistancePtCircle { point, circle, .. } => {
            let centre = l.circle_center(*circle).or_else(|| l.arc_center(*circle));
            (
                DISTANCE_MARK,
                with(vec![*point], centre.into_iter().collect()),
            )
        }
        Angle { l1, l2, .. } => ("\u{2220}", l.two_lines(*l1, *l2)),
        Fixed { point } => ("\u{2316}", vec![*point]),
        EqualLength { l1, l2 } => ("=", l.two_lines(*l1, *l2)),
        EqualRadius { e1, e2 } => {
            let arcs: Vec<_> = [*e1, *e2].iter().filter_map(|e| l.arc_center(*e)).collect();
            let centres = if arcs.is_empty() {
                [*e1, *e2]
                    .iter()
                    .filter_map(|e| l.circle_center(*e))
                    .collect()
            } else {
                arcs
            };
            ("=", centres)
        }
        TangentLineArc { line, arc } => (
            TANGENT_MARK,
            with(
                l.line_points(*line),
                l.arc_center(*arc).into_iter().collect(),
            ),
        ),
        TangentArcArc { a1, a2, .. } => (
            TANGENT_MARK,
            [*a1, *a2].iter().filter_map(|a| l.arc_center(*a)).collect(),
        ),
        SymmetricAboutLine { p1, p2, line } => {
            (SYMMETRIC_MARK, with(vec![*p1, *p2], l.line_points(*line)))
        }
        SymmetricAboutPoint { p1, p2, center } => (SYMMETRIC_MARK, vec![*p1, *p2, *center]),
        Midpoint { point, line } => ("M", with(vec![*point], l.line_points(*line))),
    }
}

/// The dimension line of `kind` in world mm (see
/// [`signex_sketch::solver::measure::dimension_span`]). `None` for an
/// angle (and anything not a distance).
fn dimension_span(kind: &ConstraintKind, l: &Lookup<'_>) -> Option<((f64, f64), (f64, f64))> {
    signex_sketch::solver::measure::dimension_span(
        kind,
        l.sketch,
        |id| l.point(id),
        |id| l.solved_radius(id),
    )
}

/// The text of a dimension's value glyph: what is being typed, the value a
/// reference measures, else `name · value` (or the value alone). `None` for
/// a constraint that is not a dimension.
fn value_text(c: &Constraint, l: &Lookup<'_>, params: &HashMap<String, f64>) -> Option<String> {
    let words = &l.state.canvas_text;
    c.kind.dim_target()?;
    if let Some(e) = l.state.sketch_overlay.value_edit.as_ref()
        && e.target == ValueTarget::Constraint(c.id)
    {
        return Some(e.buffer.clone());
    }
    if l.sketch.is_driven(c.id) {
        let solve = l.state.last_solve.as_ref().map(|s| &s.result);
        let measured = signex_sketch::solver::measure::measured_value(l.sketch, solve, c);
        return Some(match measured {
            Some(rad) if matches!(c.kind, ConstraintKind::Angle { .. }) => {
                let shown = l.state.angle_deg_mirrored(rad.to_degrees());
                value_edit::format_angle(shown, &words.unit_deg)
            }
            Some(mm) => value_edit::format_length(mm, &words.unit_mm),
            None => words.value_unresolved.clone(),
        });
    }
    let shown = l.state.shown_constraint(&c.kind);
    let (name, value) = value_edit::dimension_label(&shown, l.sketch, params, words)?;
    Some(match name {
        Some(name) => format!("{name} · {value}"),
        None => value,
    })
}

/// The glyph of one constraint: a value for a dimension, its mark
/// otherwise. `None` when none of its points can be placed.
fn constraint_glyph(
    c: &Constraint,
    l: &Lookup<'_>,
    params: &HashMap<String, f64>,
    cstate: &FootprintCanvasState,
) -> Option<Glyph> {
    let (mark, points) = constraint_parts(&c.kind, l);
    let placed: Vec<(f64, f64)> = points.iter().filter_map(|id| l.point(*id)).collect();
    if placed.is_empty() {
        return None;
    }
    let n = placed.len() as f64;
    let centroid = (
        placed.iter().map(|p| p.0).sum::<f64>() / n,
        placed.iter().map(|p| p.1).sum::<f64>() / n,
    );
    let anchor = cstate.world_to_screen(centroid);
    let target = GlyphTarget::Constraint(c.id);
    let Some(text) = value_text(c, l, params) else {
        return Some(Glyph {
            target,
            face: GlyphFace::Mark { text: mark },
            anchor,
            rect: mark_rect(anchor, mark),
        });
    };
    let edit = l
        .state
        .sketch_overlay
        .value_edit
        .as_ref()
        .filter(|e| e.target == ValueTarget::Constraint(c.id));
    let span = dimension_span(&c.kind, l)
        .map(|(a, b)| (cstate.world_to_screen(a), cstate.world_to_screen(b)));
    Some(Glyph {
        target,
        rect: label_rect(anchor, &text),
        face: GlyphFace::Value {
            text,
            editing: edit.is_some(),
            invalid: edit.is_some_and(|e| e.invalid),
            reference: l.sketch.is_driven(c.id),
            span,
        },
        anchor,
    })
}

/// The name a pattern value shows: the field's word, and the parameter
/// it is bound to ("count = rows").
fn pattern_value_name(
    field: PatternField,
    source: &str,
    sketch: &SketchData,
    text: &SketchCanvasText,
) -> String {
    let word = match field {
        PatternField::Count => &text.pattern_count,
        PatternField::Columns => &text.pattern_columns,
        PatternField::Rows => &text.pattern_rows,
        PatternField::SpacingX => &text.pattern_spacing_x,
        PatternField::SpacingY => &text.pattern_spacing_y,
        PatternField::Sweep => &text.pattern_sweep,
    };
    match value_edit::bound_parameter(source, sketch) {
        Some(param) => format!("{word} = {param}"),
        None => word.clone(),
    }
}

/// The `name · value` label of a pattern value ("count = rows · 4"), as
/// the selected pattern's glyphs and a linear pattern's dimensions at
/// rest both show it.
pub(in crate::library::editor::footprint::canvas) fn pattern_value_label(
    field: PatternField,
    kind: &ArrayKind,
    sketch: &SketchData,
    text: &SketchCanvasText,
) -> String {
    let source = field.expr(kind).unwrap_or_default();
    format!(
        "{} · {}",
        pattern_value_name(field, source, sketch, text),
        value_edit::pattern_value_text(sketch, field, source, text)
    )
}

/// How many pattern links a glyph text follows: a pattern of a mirror of
/// a pattern still reads ("P×4 of ⇔ L×4"); a longer chain stops there.
const GLYPH_LINK_DEPTH: u8 = 2;

/// The glyph text of pattern object `id`: an array's counts, then the
/// pattern it repeats when its source is one ("P×4 of L×4"); a mirror's
/// mark and the pattern it mirrors ("⇔ L×4"). Follows at most `depth`
/// links. `None` when `id` names nothing.
fn pattern_text(
    sketch: &SketchData,
    id: ArrayId,
    words: &SketchCanvasText,
    depth: u8,
) -> Option<String> {
    let linked = |of: ArrayId| match depth.checked_sub(1) {
        Some(next) => pattern_text(sketch, of, words, next),
        None => Some(words.value_unresolved.clone()),
    };
    if let Some(mirror) = sketch.pattern_mirror(id) {
        return Some(format!(
            "{} {}",
            words.pattern_mirror,
            linked(mirror.pattern)?
        ));
    }
    let array = sketch.arrays.iter().find(|a| a.id == id)?;
    let own = value_edit::pattern_glyph_text(sketch, &array.kind, words);
    Some(match sketch.pattern_source_of(id).and_then(linked) {
        Some(source) => format!("{own} {} {source}", words.pattern_of),
        None => own,
    })
}

/// Which way a pattern glyph sits from its source, as a sign per axis:
/// left of and below it, mirrored across the source for each spacing
/// dimension (`sides`, unit normals) that stands off on that side, so the
/// glyph never covers a spacing pill (#3009). Values stack the same way.
fn glyph_side(sides: &[Vector]) -> Vector {
    sides.iter().fold(Vector::new(-1.0, 1.0), |side, n| {
        let along = side.x * n.x + side.y * n.y;
        if along > 0.0 {
            Vector::new(side.x - 2.0 * along * n.x, side.y - 2.0 * along * n.y)
        } else {
            side
        }
    })
}

/// The glyph of one pattern and, while it is selected, its values and
/// instance slots.
fn pattern_glyphs(
    array: &Array,
    stack_index: usize,
    l: &Lookup<'_>,
    cstate: &FootprintCanvasState,
    out: &mut Vec<Glyph>,
) {
    let source = array.kind.source();
    let Some(source_at) = l.anchor(source) else {
        return;
    };
    let text_set = &l.state.canvas_text;
    let row = GLYPH_TEXT * LABEL_HEIGHT_EM + 2.0;
    let base = cstate.world_to_screen(source_at);
    let offset = GLYPH_TEXT * PATTERN_OFFSET_EM;
    let side = glyph_side(&spacing_sides(array, l.sketch, l.state, cstate));
    let anchor = Point::new(
        base.x + side.x * offset,
        base.y + side.y * (offset + stack_index as f32 * row),
    );
    let selected = l.state.sketch_overlay.selected_pattern == Some(array.id);
    let text = pattern_text(l.sketch, array.id, text_set, GLYPH_LINK_DEPTH).unwrap_or_default();
    out.push(Glyph {
        target: GlyphTarget::Pattern(array.id),
        rect: label_rect(anchor, &text),
        face: GlyphFace::Pattern { text, selected },
        anchor,
    });
    if !selected {
        return;
    }
    for (k, &field) in PatternField::of(&array.kind).iter().enumerate() {
        let value_anchor = Point::new(anchor.x, anchor.y + side.y * (k + 1) as f32 * row);
        let edit = l
            .state
            .sketch_overlay
            .value_edit
            .as_ref()
            .filter(|e| e.target == ValueTarget::Pattern(array.id, field));
        let text = match edit {
            Some(e) => e.buffer.clone(),
            None => pattern_value_label(field, &array.kind, l.sketch, text_set),
        };
        out.push(Glyph {
            target: GlyphTarget::PatternValue(array.id, field),
            rect: label_rect(value_anchor, &text),
            face: GlyphFace::Value {
                text,
                editing: edit.is_some(),
                invalid: edit.is_some_and(|e| e.invalid),
                reference: false,
                span: None,
            },
            anchor: value_anchor,
        });
    }
    instance_glyphs(array, source_at, l, cstate, out);
}

/// The glyph of each pattern mirror: the mirror mark and the mirrored
/// pattern's glyph ("⇔ L×4"), right of and above the mirror line. Mirrors
/// of one line stack downwards.
fn mirror_glyphs(l: &Lookup<'_>, cstate: &FootprintCanvasState, out: &mut Vec<Glyph>) {
    let words = &l.state.canvas_text;
    let row = GLYPH_TEXT * LABEL_HEIGHT_EM + 2.0;
    let mut per_line: HashMap<SketchEntityId, usize> = HashMap::new();
    for mirror in &l.sketch.pattern_mirrors {
        let Some(text) = pattern_text(l.sketch, mirror.id, words, GLYPH_LINK_DEPTH) else {
            continue;
        };
        let Some(line_at) = l.anchor(mirror.line) else {
            continue;
        };
        let stacked = per_line.entry(mirror.line).or_default();
        let base = cstate.world_to_screen(line_at);
        let offset = GLYPH_TEXT * PATTERN_OFFSET_EM;
        let anchor = Point::new(base.x + offset, base.y - offset + *stacked as f32 * row);
        *stacked += 1;
        out.push(Glyph {
            target: GlyphTarget::Pattern(mirror.id),
            rect: label_rect(anchor, &text),
            face: GlyphFace::Pattern {
                text,
                selected: l.state.sketch_overlay.selected_pattern == Some(mirror.id),
            },
            anchor,
        });
    }
}

/// The instance slots of a selected grid or polar pattern, the source's
/// own slot left out (it is the source pad, edited as such). The slots
/// come from the bake's own walk (`signex_bake::pattern_slots`), so a ring
/// sits where the bake puts the pad; a slot the mask drops has no ring.
fn instance_glyphs(
    array: &Array,
    source_at: (f64, f64),
    l: &Lookup<'_>,
    cstate: &FootprintCanvasState,
    out: &mut Vec<Glyph>,
) {
    if !array.kind.has_suppression_model() {
        return;
    }
    let Some(solve) = l.state.last_solve.as_ref() else {
        return;
    };
    let Some(slots) = signex_bake::pattern_slots(
        l.sketch,
        solve,
        array.id,
        source_at,
        value_edit::MAX_DRAWN_SLOTS,
    ) else {
        return;
    };
    let r = GLYPH_TEXT * INSTANCE_RADIUS_EM;
    for slot in slots.into_iter().filter(|s| (s.i, s.j) != (0, 0)) {
        let (i, j) = (slot.i, slot.j);
        let at = cstate.world_to_screen(slot.at);
        out.push(Glyph {
            target: GlyphTarget::Instance {
                array: array.id,
                i,
                j,
            },
            face: GlyphFace::Instance {
                suppressed: slot.suppressed,
            },
            anchor: at,
            rect: Rectangle::new(Point::new(at.x - r, at.y - r), Size::new(2.0 * r, 2.0 * r)),
        });
    }
}

/// Every glyph of `sketch` as the canvas camera shows it: constraints in
/// sketch order, then each pattern with, while selected, its values and
/// instance slots, then each pattern mirror.
pub(crate) fn sketch_glyphs(
    sketch: &SketchData,
    state: &FootprintEditorState,
    cstate: &FootprintCanvasState,
) -> Vec<Glyph> {
    let l = Lookup { sketch, state };
    let params = value_edit::resolved_params(sketch, state);
    let mut out: Vec<Glyph> = sketch
        .constraints
        .iter()
        .filter_map(|c| constraint_glyph(c, &l, &params, cstate))
        .collect();
    let mut per_source: HashMap<SketchEntityId, usize> = HashMap::new();
    for array in &sketch.arrays {
        let stacked = per_source.entry(array.kind.source()).or_default();
        pattern_glyphs(array, *stacked, &l, cstate, &mut out);
        *stacked += 1;
    }
    mirror_glyphs(&l, cstate, &mut out);
    out
}

#[cfg(test)]
#[path = "glyph_layout_tests.rs"]
mod tests;
