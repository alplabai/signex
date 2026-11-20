//! Where the sketch canvas puts each constraint glyph, dimension value,
//! pattern glyph and pattern instance. One pure layout, read by the draw
//! pass and by the hit test, so a click lands on what is drawn.

use std::collections::HashMap;

use iced::{Point, Rectangle, Size};
use signex_sketch::SketchData;
use signex_sketch::array::{Array, ArrayId, ArrayKind};
use signex_sketch::constraint::{Constraint, ConstraintKind};
use signex_sketch::entity::EntityKind;
use signex_sketch::id::{ConstraintId, SketchEntityId};

use super::FootprintCanvasState;
use crate::library::editor::footprint::sketch_mode::value_edit;
use crate::library::editor::footprint::state::FootprintEditorState;
use crate::library::editor::footprint::state::canvas_text::SketchCanvasText;
use crate::library::editor::footprint::state::sketch_overlay::{PatternField, ValueTarget};

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;

/// Text size of every glyph and value on the drawing.
pub(crate) const GLYPH_TEXT: f32 = TS.body;
/// Average advance of a glyph character, in ems; sizes a label before it
/// is drawn (the canvas cannot measure text).
const ADVANCE_EM: f32 = 0.62;
/// Height of a value or pattern label, in text heights.
const LABEL_HEIGHT_EM: f32 = 1.7;
/// Horizontal padding inside a label, in text heights.
const LABEL_PAD_EM: f32 = 0.6;
/// How far a pattern glyph sits left of and below its source, in text
/// heights; further glyphs of the same source stack below it.
const PATTERN_OFFSET_EM: f32 = 2.2;
/// Radius of an instance slot's click target, in text heights.
const INSTANCE_RADIUS_EM: f32 = 0.8;

/// What a glyph stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GlyphTarget {
    Constraint(ConstraintId),
    Pattern(ArrayId),
    PatternValue(ArrayId, PatternField),
    Instance { array: ArrayId, i: u32, j: u32 },
}

/// How a glyph is drawn.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum GlyphFace {
    /// A constraint's letter, turned to follow its line when it has one.
    Letter {
        text: &'static str,
        rotation: Option<f32>,
    },
    /// A value on the drawing (`name · value`), or the text being typed.
    Value {
        text: String,
        editing: bool,
        invalid: bool,
    },
    /// A pattern's glyph ("L×4").
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
    /// and patterns first, then instance slots, then letters.
    fn hit_rank(&self) -> u8 {
        match self.face {
            GlyphFace::Value { .. } | GlyphFace::Pattern { .. } => 0,
            GlyphFace::Instance { .. } => 1,
            GlyphFace::Letter { .. } => 2,
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
fn label_rect(anchor: Point, text: &str) -> Rectangle {
    let width =
        text.chars().count() as f32 * GLYPH_TEXT * ADVANCE_EM + 2.0 * GLYPH_TEXT * LABEL_PAD_EM;
    let height = GLYPH_TEXT * LABEL_HEIGHT_EM;
    Rectangle::new(
        Point::new(anchor.x - width / 2.0, anchor.y - height / 2.0),
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

/// A constraint's letter, the points its glyph sits among, and the line
/// whose direction the letter follows.
struct ConstraintShape {
    letter: &'static str,
    points: Vec<SketchEntityId>,
    primary_line: Option<(SketchEntityId, SketchEntityId)>,
}

/// The [`ConstraintShape`] of `kind`. The letters are the shipped set.
fn constraint_shape(kind: &ConstraintKind, l: &Lookup<'_>) -> ConstraintShape {
    let (letter, points, primary_line) = constraint_parts(kind, l);
    ConstraintShape {
        letter,
        points,
        primary_line,
    }
}

type Parts = (
    &'static str,
    Vec<SketchEntityId>,
    Option<(SketchEntityId, SketchEntityId)>,
);

fn constraint_parts(kind: &ConstraintKind, l: &Lookup<'_>) -> Parts {
    use ConstraintKind::*;
    let with = |mut head: Vec<SketchEntityId>, tail: Vec<SketchEntityId>| {
        head.extend(tail);
        head
    };
    match kind {
        Coincident { p1, p2 } => ("=", vec![*p1, *p2], Some((*p1, *p2))),
        PointOnLine { point, line } => (
            "|",
            with(vec![*point], l.line_points(*line)),
            l.line_ends(*line),
        ),
        Horizontal { line } => ("H", l.line_points(*line), l.line_ends(*line)),
        Vertical { line } => ("V", l.line_points(*line), l.line_ends(*line)),
        Parallel { l1, l2 } => (
            "//",
            with(l.line_points(*l1), l.line_points(*l2)),
            l.line_ends(*l1),
        ),
        Perpendicular { l1, l2 } => (
            "L",
            with(l.line_points(*l1), l.line_points(*l2)),
            l.line_ends(*l1),
        ),
        DistancePtPt { p1, p2, .. } => ("D", vec![*p1, *p2], None),
        DistancePtLine { point, line, .. } => ("d", with(vec![*point], l.line_points(*line)), None),
        DistancePtCircle { point, circle, .. } => {
            let centre = l.circle_center(*circle).or_else(|| l.arc_center(*circle));
            (
                "\u{29bf}",
                with(vec![*point], centre.into_iter().collect()),
                None,
            )
        }
        Fixed { point } => ("\u{1F512}", vec![*point], None),
        PointOnArc { point, arc } => {
            let refs = match l.kind(*arc) {
                Some(EntityKind::Arc {
                    center, start, end, ..
                }) => vec![*center, *start, *end],
                _ => Vec::new(),
            };
            ("\u{2192}", with(vec![*point], refs), None)
        }
        Angle { l1, l2, .. } => ("A", with(l.line_points(*l1), l.line_points(*l2)), None),
        EqualLength { l1, l2 } => (
            "=L",
            with(l.line_points(*l1), l.line_points(*l2)),
            l.line_ends(*l1),
        ),
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
            ("=R", centres, None)
        }
        TangentLineArc { line, arc } => (
            "T",
            with(
                l.line_points(*line),
                l.arc_center(*arc).into_iter().collect(),
            ),
            l.line_ends(*line),
        ),
        TangentArcArc { a1, a2, .. } => (
            "TT",
            [*a1, *a2].iter().filter_map(|a| l.arc_center(*a)).collect(),
            None,
        ),
        SymmetricAboutLine { p1, p2, line } => (
            "\u{29C7}",
            with(vec![*p1, *p2], l.line_points(*line)),
            l.line_ends(*line),
        ),
        SymmetricAboutPoint { p1, p2, center } => {
            ("\u{29C7}", vec![*p1, *p2, *center], Some((*p1, *p2)))
        }
        Midpoint { point, line } => (
            "M",
            with(vec![*point], l.line_points(*line)),
            l.line_ends(*line),
        ),
    }
}

/// The glyph of one constraint: a value for a dimension, its letter
/// otherwise. `None` when none of its points can be placed.
fn constraint_glyph(
    c: &Constraint,
    l: &Lookup<'_>,
    params: &HashMap<String, f64>,
    cstate: &FootprintCanvasState,
) -> Option<Glyph> {
    let ConstraintShape {
        letter,
        points,
        primary_line,
    } = constraint_shape(&c.kind, l);
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
    let overlay = &l.state.sketch_overlay;
    if let Some((name, value)) =
        value_edit::dimension_label(&c.kind, l.sketch, params, &l.state.canvas_text)
    {
        let edit = overlay
            .value_edit
            .as_ref()
            .filter(|e| e.target == ValueTarget::Constraint(c.id));
        let text = match (edit, name) {
            (Some(e), _) => e.buffer.clone(),
            (None, Some(name)) => format!("{name} · {value}"),
            (None, None) => value,
        };
        let rect = label_rect(anchor, &text);
        return Some(Glyph {
            target,
            face: GlyphFace::Value {
                text,
                editing: edit.is_some(),
                invalid: edit.is_some_and(|e| e.invalid),
            },
            anchor,
            rect,
        });
    }
    let rotation = primary_line.and_then(|(a, b)| {
        let a = cstate.world_to_screen(l.point(a)?);
        let b = cstate.world_to_screen(l.point(b)?);
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        (dx.hypot(dy) > 1.0).then(|| dy.atan2(dx))
    });
    let rect = if rotation.is_some() {
        // A turned letter is drawn about its anchor.
        Rectangle::new(
            Point::new(anchor.x - GLYPH_TEXT, anchor.y - GLYPH_TEXT),
            Size::new(2.0 * GLYPH_TEXT, 2.0 * GLYPH_TEXT),
        )
    } else {
        // An upright letter is drawn from a small nudge right of and above
        // its anchor.
        let nudge = GLYPH_TEXT * 0.55;
        Rectangle::new(
            Point::new(anchor.x + nudge, anchor.y - nudge),
            Size::new(
                letter.chars().count() as f32 * GLYPH_TEXT * ADVANCE_EM + 2.0,
                GLYPH_TEXT * 1.3,
            ),
        )
    };
    Some(Glyph {
        target,
        face: GlyphFace::Letter {
            text: letter,
            rotation,
        },
        anchor,
        rect,
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
    let anchor = Point::new(base.x - offset, base.y + offset + stack_index as f32 * row);
    let selected = l.state.sketch_overlay.selected_pattern == Some(array.id);
    let text = value_edit::pattern_glyph_text(l.sketch, &array.kind, text_set);
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
        let source_expr = field.expr(&array.kind).unwrap_or_default();
        let value_anchor = Point::new(anchor.x, anchor.y + (k + 1) as f32 * row);
        let edit = l
            .state
            .sketch_overlay
            .value_edit
            .as_ref()
            .filter(|e| e.target == ValueTarget::Pattern(array.id, field));
        let text = match edit {
            Some(e) => e.buffer.clone(),
            None => format!(
                "{} · {}",
                pattern_value_name(field, source_expr, l.sketch, text_set),
                value_edit::pattern_value_text(l.sketch, field, source_expr, text_set)
            ),
        };
        out.push(Glyph {
            target: GlyphTarget::PatternValue(array.id, field),
            rect: label_rect(value_anchor, &text),
            face: GlyphFace::Value {
                text,
                editing: edit.is_some(),
                invalid: edit.is_some_and(|e| e.invalid),
            },
            anchor: value_anchor,
        });
    }
    instance_glyphs(array, source_at, l, cstate, out);
}

/// The instance slots of a selected grid or polar pattern, the source's
/// own slot left out (it is the source pad, edited as such).
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
    let center = match &array.kind {
        ArrayKind::Polar { center, .. } => l.anchor(*center),
        _ => None,
    };
    let Some(layout) = value_edit::array_layout(l.sketch, &array.kind, center) else {
        return;
    };
    let r = GLYPH_TEXT * INSTANCE_RADIUS_EM;
    for (i, j) in layout.slots().into_iter().filter(|&slot| slot != (0, 0)) {
        let at = cstate.world_to_screen(layout.position(source_at, i, j));
        out.push(Glyph {
            target: GlyphTarget::Instance {
                array: array.id,
                i,
                j,
            },
            face: GlyphFace::Instance {
                suppressed: array.kind.is_instance_suppressed(i, j),
            },
            anchor: at,
            rect: Rectangle::new(Point::new(at.x - r, at.y - r), Size::new(2.0 * r, 2.0 * r)),
        });
    }
}

/// Every glyph of `sketch` as the canvas camera shows it: constraints in
/// sketch order, then each pattern with, while selected, its values and
/// instance slots.
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
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_sketch::constraint::DimTarget;
    use signex_sketch::entity::Entity;
    use signex_sketch::plane::{Plane, PlaneId, PlaneKind};

    fn words() -> SketchCanvasText {
        SketchCanvasText {
            unit_mm: "mm".into(),
            unit_deg: "°".into(),
            value_unresolved: "?".into(),
            pattern_linear: "L".into(),
            ..SketchCanvasText::default()
        }
    }

    /// Two points 0.7 mm apart and a distance between them.
    fn sketch(target: DimTarget) -> (SketchData, ConstraintId) {
        let plane = PlaneId::new();
        let (p1, p2) = (SketchEntityId::new(), SketchEntityId::new());
        let id = ConstraintId::new();
        let mut parameters = signex_sketch::parameter::ParameterTable::new();
        parameters.insert("pitch", "0.65mm");
        let sketch = SketchData {
            planes: vec![Plane {
                id: plane,
                kind: PlaneKind::BoardTop,
            }],
            entities: vec![
                Entity::new(p1, plane, EntityKind::Point { x: 0.0, y: 0.0 }),
                Entity::new(p2, plane, EntityKind::Point { x: 0.7, y: 0.0 }),
            ],
            constraints: vec![Constraint {
                id,
                kind: ConstraintKind::DistancePtPt { p1, p2, target },
            }],
            parameters,
            ..SketchData::default()
        };
        (sketch, id)
    }

    fn glyphs_of(sketch: &SketchData) -> Vec<Glyph> {
        let mut state = FootprintEditorState::empty();
        state.canvas_text = words();
        let cstate = FootprintCanvasState::with_view(100.0, Point::new(50.0, 50.0));
        sketch_glyphs(sketch, &state, &cstate)
    }

    #[test]
    fn a_dimension_draws_its_value_and_the_hit_test_finds_it() {
        let (sketch, id) = sketch(DimTarget::Literal(0.7));
        let glyphs = glyphs_of(&sketch);
        let glyph = glyphs
            .iter()
            .find(|g| g.constraint() == Some(id))
            .expect("glyph");
        assert!(
            matches!(&glyph.face, GlyphFace::Value { text, .. } if text == "0.70 mm"),
            "{:?}",
            glyph.face
        );
        // The value sits between the two points: x = 50 + 0.35 * 100.
        assert!((glyph.anchor.x - 85.0).abs() < 1e-3, "{:?}", glyph.anchor);
        let hit = glyph_at(&glyphs, glyph.anchor).expect("a click on the value hits it");
        assert_eq!(hit.constraint(), Some(id));
        assert!(glyph_at(&glyphs, Point::new(500.0, 500.0)).is_none());
    }

    #[test]
    fn a_dimension_bound_to_a_parameter_draws_name_and_value() {
        let (sketch, id) = sketch(DimTarget::Expr("pitch".into()));
        let glyphs = glyphs_of(&sketch);
        let glyph = glyphs
            .iter()
            .find(|g| g.constraint() == Some(id))
            .expect("glyph");
        assert!(
            matches!(&glyph.face, GlyphFace::Value { text, .. } if text == "pitch · 0.65 mm"),
            "{:?}",
            glyph.face
        );
    }
}
