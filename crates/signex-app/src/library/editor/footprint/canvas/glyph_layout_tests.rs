//! Tests of `glyph_layout`: values, reference dimensions, marks, pattern
//! and mirror glyphs.

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
        pattern_polar: "P".into(),
        pattern_mirror: "\u{21D4}".into(),
        pattern_of: "of".into(),
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

/// The value glyph of constraint `id`.
fn value_of(glyphs: &[Glyph], id: ConstraintId) -> (String, bool, Option<(Point, Point)>) {
    let glyph = glyphs
        .iter()
        .find(|g| g.constraint() == Some(id))
        .expect("glyph");
    match &glyph.face {
        GlyphFace::Value {
            text,
            reference,
            span,
            ..
        } => (text.clone(), *reference, *span),
        other => panic!("a value: {other:?}"),
    }
}

#[test]
fn a_reference_dimension_shows_what_it_measures_not_its_target() {
    let (mut sketch, id) = sketch(DimTarget::Literal(5.0));
    assert!(sketch.set_driven(id, true));
    let (text, reference, span) = value_of(&glyphs_of(&sketch), id);
    assert_eq!(text, "0.70 mm", "the measured 0.7, not the 5 it was set to");
    assert!(
        reference,
        "drawn as a reference: dashed, in the reference ink"
    );
    let (a, b) = span.expect("a dimension line");
    assert!(
        (a.x - 50.0).abs() < 1e-3 && (b.x - 120.0).abs() < 1e-3,
        "{a:?} {b:?}"
    );
}

#[test]
fn a_driving_distance_shows_its_target_on_a_solid_dimension_line() {
    let (sketch, id) = sketch(DimTarget::Literal(5.0));
    let (text, reference, span) = value_of(&glyphs_of(&sketch), id);
    assert_eq!(text, "5.00 mm");
    assert!(!reference);
    assert!(span.is_some(), "every distance draws its dimension line");
}

#[test]
fn relations_draw_the_mockup_marks() {
    let plane = PlaneId::new();
    let ids: Vec<SketchEntityId> = (0..6).map(|_| SketchEntityId::new()).collect();
    let (l1, l2) = (SketchEntityId::new(), SketchEntityId::new());
    let point = |id, x, y| Entity::new(id, plane, EntityKind::Point { x, y });
    let mut sketch = SketchData {
        entities: vec![
            point(ids[0], 0.0, 0.0),
            point(ids[1], 1.0, 0.0),
            point(ids[2], 0.0, 1.0),
            point(ids[3], 1.0, 1.0),
            Entity::new(
                l1,
                plane,
                EntityKind::Line {
                    start: ids[0],
                    end: ids[1],
                },
            ),
            Entity::new(
                l2,
                plane,
                EntityKind::Line {
                    start: ids[2],
                    end: ids[3],
                },
            ),
        ],
        ..SketchData::default()
    };
    let kinds = [
        (ConstraintKind::Horizontal { line: l1 }, "H"),
        (ConstraintKind::Vertical { line: l2 }, "V"),
        (ConstraintKind::Parallel { l1, l2 }, "\u{2225}"),
        (ConstraintKind::Perpendicular { l1, l2 }, "\u{22A5}"),
        (ConstraintKind::EqualLength { l1, l2 }, "="),
        (
            ConstraintKind::Coincident {
                p1: ids[0],
                p2: ids[2],
            },
            COINCIDENT_MARK,
        ),
        (ConstraintKind::Fixed { point: ids[0] }, "\u{2316}"),
    ];
    let mut expected = Vec::new();
    for (kind, mark) in kinds {
        let id = ConstraintId::new();
        sketch.constraints.push(Constraint { id, kind });
        expected.push((id, mark));
    }
    let glyphs = glyphs_of(&sketch);
    for (id, mark) in expected {
        let glyph = glyphs
            .iter()
            .find(|g| g.constraint() == Some(id))
            .expect("glyph");
        assert_eq!(glyph.face, GlyphFace::Mark { text: mark });
        assert_eq!(
            glyph_at(
                &glyphs,
                Point::new(glyph.rect.center_x(), glyph.rect.center_y())
            )
            .map(|g| g.rect),
            Some(glyph.rect),
            "a click on the mark's pill hits a mark there"
        );
    }
}

/// A point, a line, a linear pattern of the point ("L×4"), a polar
/// pattern repeating it ("P×6"), and a mirror of the linear pattern.
fn patterned() -> (SketchData, ArrayId, ArrayId, ArrayId) {
    use signex_sketch::array::NumberingScheme;
    use signex_sketch::sketch::{PatternMirror, PatternSource};
    let plane = PlaneId::new();
    let (p, a, b, line, centre) = (
        SketchEntityId::new(),
        SketchEntityId::new(),
        SketchEntityId::new(),
        SketchEntityId::new(),
        SketchEntityId::new(),
    );
    let (linear, polar, mirror) = (ArrayId::new(), ArrayId::new(), ArrayId::new());
    let sketch = SketchData {
        entities: vec![
            Entity::new(p, plane, EntityKind::Point { x: 0.0, y: 0.0 }),
            Entity::new(a, plane, EntityKind::Point { x: 0.0, y: -1.0 }),
            Entity::new(b, plane, EntityKind::Point { x: 0.0, y: 1.0 }),
            Entity::new(centre, plane, EntityKind::Point { x: 5.0, y: 5.0 }),
            Entity::new(line, plane, EntityKind::Line { start: a, end: b }),
        ],
        arrays: vec![
            Array {
                id: linear,
                kind: ArrayKind::Linear {
                    source: p,
                    count_expr: "4".into(),
                    dx_expr: "1mm".into(),
                    dy_expr: "0mm".into(),
                },
                numbering: NumberingScheme::default(),
            },
            Array {
                id: polar,
                kind: ArrayKind::Polar {
                    source: p,
                    center: centre,
                    count_expr: "6".into(),
                    sweep_angle_expr: "360deg".into(),
                    depopulation: None,
                },
                numbering: NumberingScheme::default(),
            },
        ],
        pattern_sources: vec![PatternSource {
            array: polar,
            source: linear,
        }],
        pattern_mirrors: vec![PatternMirror {
            id: mirror,
            pattern: linear,
            line,
        }],
        ..SketchData::default()
    };
    (sketch, linear, polar, mirror)
}

fn pattern_face(glyphs: &[Glyph], id: ArrayId) -> (String, bool) {
    let glyph = glyphs
        .iter()
        .find(|g| g.target == GlyphTarget::Pattern(id))
        .expect("glyph");
    match &glyph.face {
        GlyphFace::Pattern { text, selected } => (text.clone(), *selected),
        other => panic!("a pattern glyph: {other:?}"),
    }
}

#[test]
fn a_pattern_of_a_pattern_names_its_source_and_a_mirror_names_its_pattern() {
    let (sketch, linear, polar, mirror) = patterned();
    let glyphs = glyphs_of(&sketch);
    assert_eq!(pattern_face(&glyphs, linear).0, "L×4");
    assert_eq!(pattern_face(&glyphs, polar).0, "P×6 of L×4");
    assert_eq!(pattern_face(&glyphs, mirror).0, "\u{21D4} L×4");
}

#[test]
fn a_mirror_glyph_selects_like_a_pattern() {
    let (sketch, _, _, mirror) = patterned();
    let mut state = FootprintEditorState::empty();
    state.canvas_text = words();
    state.sketch_overlay.selected_pattern = Some(mirror);
    let cstate = FootprintCanvasState::with_view(100.0, Point::new(50.0, 50.0));
    let glyphs = sketch_glyphs(&sketch, &state, &cstate);
    let (_, selected) = pattern_face(&glyphs, mirror);
    assert!(selected);
    let glyph = glyphs
        .iter()
        .find(|g| g.target == GlyphTarget::Pattern(mirror))
        .expect("glyph");
    assert_eq!(
        glyph_at(&glyphs, glyph.anchor).map(|g| g.target),
        Some(GlyphTarget::Pattern(mirror)),
        "a click on the mirror glyph hits it"
    );
}
