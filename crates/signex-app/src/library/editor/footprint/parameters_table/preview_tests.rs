//! Mockup row 189 and the 2026-11-24 decision: the live bake preview under
//! the parameters table redraws on each keystroke, keeps the last good bake
//! marked while an expression is in error, reads the current solve without
//! solving again, and sits beside Live checks, whose error line comes
//! first. On the footprint sketch and the board sketch alike.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use signex_library::{Footprint, FootprintFile};
use signex_sketch::Solver;
use signex_sketch::array::{Array, ArrayId, ArrayKind, NumberingScheme};
use signex_sketch::attr::PadAttr;
use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::{ConstraintId, SketchEntityId};
use signex_sketch::parameter::resolve;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use signex_sketch::sketch::SketchData;
use signex_types::sketch::SketchHost;

use super::SketchParametersMsg;
use super::checks::failed_lines;
use super::preview::{BakePreview, PreviewPad, failing_parameters, of_editor};
use super::view::problems_of;
use crate::app::FootprintDocument;
use crate::library::editor::footprint::state::{BoardBackdrop, EditorMode, EditorPad};
use crate::library::editor::footprint::updates::apply_footprint_primitive_edit;
use crate::library::messages::FootprintEditorMsg as M;

const EPS: f64 = 1e-6;
const HOSTS: [SketchHost; 2] = [SketchHost::Footprint, SketchHost::Board];

/// A line from `a`, fixed at the origin, to `b`, at the distance `w`
/// (2 mm) from it.
fn line_sketch() -> SketchData {
    let plane = PlaneId::new();
    let mut sketch = SketchData::default();
    sketch.planes.push(Plane {
        id: plane,
        kind: PlaneKind::BoardTop,
    });
    let mut add = |kind| {
        let id = SketchEntityId::new();
        sketch.entities.push(Entity::new(id, plane, kind));
        id
    };
    let a = add(EntityKind::Point { x: 0.0, y: 0.0 });
    let b = add(EntityKind::Point { x: 1.0, y: 0.0 });
    add(EntityKind::Line { start: a, end: b });
    sketch.parameters.insert("w", "2mm");
    sketch.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Fixed { point: a },
    });
    sketch.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::DistancePtPt {
            p1: a,
            p2: b,
            target: DimTarget::Expr("w".into()),
        },
    });
    sketch
}

/// An editor over `sketch` in Sketch mode with the table open, on `host`.
fn editor_on(host: SketchHost, sketch: SketchData) -> FootprintDocument {
    let mut fp = Footprint::empty("t");
    fp.sketch = Some(sketch);
    let mut editor = FootprintDocument::new(
        PathBuf::from("preview.snxfpt"),
        FootprintFile::from_footprint(fp),
    );
    if host == SketchHost::Board {
        editor.state.board_backdrop = Some(Arc::new(BoardBackdrop::default()));
    }
    apply_footprint_primitive_edit(&mut editor, M::SetMode(EditorMode::Sketch));
    send(&mut editor, SketchParametersMsg::TableToggled);
    assert_eq!(editor.state.sketch_host(), host);
    editor
}

fn send(editor: &mut FootprintDocument, msg: SketchParametersMsg) {
    apply_footprint_primitive_edit(editor, M::SketchParameters(msg));
}

fn type_w(editor: &mut FootprintDocument, expr: &str) {
    send(
        editor,
        SketchParametersMsg::ExpressionEdited {
            name: "w".into(),
            expr: expr.into(),
        },
    );
}

/// The length of the preview's only edge.
fn drawn_length(preview: &BakePreview) -> f64 {
    assert_eq!(preview.edges.len(), 1, "one line bakes");
    let edge = &preview.edges[0];
    let (a, b) = (edge[0], edge[edge.len() - 1]);
    (b.0 - a.0).hypot(b.1 - a.1)
}

#[test]
fn a_keystroke_redraws_the_preview_on_both_hosts() {
    for host in HOSTS {
        let mut editor = editor_on(host, line_sketch());
        let preview = of_editor(&editor).expect("solved on entry");
        assert!((drawn_length(&preview) - 2.0).abs() < EPS, "{host:?}");
        type_w(&mut editor, "3mm");
        let preview = of_editor(&editor).expect("re-solved");
        assert!((drawn_length(&preview) - 3.0).abs() < EPS, "{host:?}");
        let dimension = &preview.dimensions[0];
        assert_eq!(dimension.name.as_deref(), Some("w"));
        assert!((dimension.value - 3.0).abs() < EPS);
        assert!(!preview.is_last_good && !dimension.is_last_good);
    }
}

#[test]
fn a_bad_expression_keeps_the_last_good_bake_marked_on_both_hosts() {
    for host in HOSTS {
        let mut editor = editor_on(host, line_sketch());
        type_w(&mut editor, "3mm");
        type_w(&mut editor, "bodyy + 1mm");
        let preview = of_editor(&editor).expect("the last good solve is kept");
        assert!((drawn_length(&preview) - 3.0).abs() < EPS, "{host:?}");
        assert!(preview.is_last_good, "the preview is marked");
        let dimension = &preview.dimensions[0];
        assert!(dimension.is_last_good, "and so is the dimension w drives");
        assert!(
            (dimension.value - 3.0).abs() < EPS,
            "with its last good value"
        );
        type_w(&mut editor, "4mm");
        let preview = of_editor(&editor).expect("solved again");
        assert!(!preview.is_last_good, "fixed: the mark goes");
        assert!((drawn_length(&preview) - 4.0).abs() < EPS);
    }
}

/// The preview bakes from the solve it is given and the parameters that
/// solve ran with. Were it to resolve or solve again it would draw 5 mm.
#[test]
fn the_preview_reads_the_current_solve_and_never_solves() {
    let mut sketch = line_sketch();
    let params = resolve(&sketch.parameters).unwrap();
    let solve = Solver::default().solve(&sketch, &params).unwrap();
    sketch.parameters.insert("w", "5mm");
    for host in HOSTS {
        let preview = BakePreview::of(&sketch, &solve, host, &BTreeSet::new());
        assert!((drawn_length(&preview) - 2.0).abs() < EPS, "{host:?}");
        assert!((preview.dimensions[0].value - 2.0).abs() < EPS);
    }
}

#[test]
fn construction_edges_are_not_previewed() {
    let mut sketch = line_sketch();
    for entity in &mut sketch.entities {
        if matches!(entity.kind, EntityKind::Line { .. }) {
            entity.construction = true;
        }
    }
    let params = resolve(&sketch.parameters).unwrap();
    let solve = Solver::default().solve(&sketch, &params).unwrap();
    let preview = BakePreview::of(&sketch, &solve, SketchHost::Footprint, &BTreeSet::new());
    assert!(preview.edges.is_empty());
}

/// A pad point sized by `pad_w`, in a linear pattern of three 2 mm apart.
fn pad_sketch() -> SketchData {
    let plane = PlaneId::new();
    let mut sketch = SketchData::default();
    sketch.planes.push(Plane {
        id: plane,
        kind: PlaneKind::BoardTop,
    });
    let id = SketchEntityId::new();
    let mut point = Entity::new(id, plane, EntityKind::Point { x: 1.0, y: 0.0 });
    point.pad = Some(PadAttr {
        number: "1".into(),
        size_x_expr: "pad_w".into(),
        size_y_expr: "1mm".into(),
        ..PadAttr::default()
    });
    sketch.entities.push(point);
    sketch.parameters.insert("pad_w", "0.6mm");
    sketch.arrays.push(Array {
        id: ArrayId::new(),
        kind: ArrayKind::Linear {
            source: id,
            count_expr: "3".into(),
            dx_expr: "2mm".into(),
            dy_expr: "0mm".into(),
        },
        numbering: NumberingScheme::default(),
    });
    sketch
}

#[test]
fn a_footprint_previews_its_baked_pads_and_a_board_sketch_none() {
    let sketch = pad_sketch();
    let params = resolve(&sketch.parameters).unwrap();
    let solve = Solver::default().solve(&sketch, &params).unwrap();
    let none = BTreeSet::new();
    let footprint = BakePreview::of(&sketch, &solve, SketchHost::Footprint, &none);
    assert_eq!(footprint.pads.len(), 3, "the source and its two copies");
    for pad in &footprint.pads {
        let PreviewPad::Outline(corners) = pad else {
            panic!("a 0.6 x 1 mm pad is drawn as its outline");
        };
        let width = (corners[1].0 - corners[0].0).abs();
        assert!((width - 0.6).abs() < EPS, "sized by pad_w");
    }
    let board = BakePreview::of(&sketch, &solve, SketchHost::Board, &none);
    assert!(board.pads.is_empty());
    assert!(!footprint.pads_hidden && !board.pads_hidden);
}

/// The angle a drawn pad outline is turned by, in degrees.
fn outline_angle_deg(pad: &PreviewPad) -> f64 {
    let PreviewPad::Outline(corners) = pad else {
        panic!("a 0.6 x 1 mm pad is drawn as its outline");
    };
    let (dx, dy) = (corners[1].0 - corners[0].0, corners[1].1 - corners[0].1);
    dy.atan2(dx).to_degrees()
}

/// A pad turned by the angle parameter `rot` (30 deg). While another row
/// is in error the pad bake cannot read `rot` as an angle, so the preview
/// draws no pad rather than one turned by a length-read value.
#[test]
fn an_angle_driven_pad_is_drawn_at_its_angle_or_not_at_all() {
    let mut sketch = pad_sketch();
    sketch.arrays.clear();
    sketch.parameters.insert("rot", "30deg");
    for entity in &mut sketch.entities {
        if let Some(pad) = entity.pad.as_mut() {
            pad.rotation_expr = Some("rot".into());
        }
    }
    let params = resolve(&sketch.parameters).unwrap();
    let solve = Solver::default().solve(&sketch, &params).unwrap();

    let read = BakePreview::of(&sketch, &solve, SketchHost::Footprint, &BTreeSet::new());
    assert_eq!(read.pads.len(), 1);
    assert!((outline_angle_deg(&read.pads[0]).abs() - 30.0).abs() < 1e-6);

    sketch.parameters.insert("junk", "bodyy + 1mm");
    let failing = failing_parameters(&sketch);
    assert_eq!(failing, BTreeSet::from(["junk".to_string()]));
    let last_good = BakePreview::of(&sketch, &solve, SketchHost::Footprint, &failing);
    for pad in &last_good.pads {
        assert!(
            (outline_angle_deg(pad).abs() - 30.0).abs() < 1e-6,
            "a drawn pad keeps the solved angle"
        );
    }
    assert!(last_good.pads.is_empty(), "the families cannot be read");
    assert!(last_good.pads_hidden, "and the card says so");
    assert!(last_good.is_last_good);
    assert_eq!(last_good.edges, read.edges, "edges stay");
}

#[test]
fn failing_parameters_are_the_rows_without_a_value() {
    let mut sketch = line_sketch();
    sketch.parameters.insert("v", "w * 2");
    assert!(failing_parameters(&sketch).is_empty());
    sketch.parameters.insert("w", "bodyy");
    assert_eq!(
        failing_parameters(&sketch),
        BTreeSet::from(["v".to_string(), "w".to_string()]),
        "w is in error and v is blocked by it"
    );
}

/// Live checks sits beside the preview, and a parameter's error line comes
/// before the pad overlaps.
#[test]
fn live_checks_lists_the_error_line_before_the_overlaps_on_both_hosts() {
    let i18n = signex_i18n::I18n::en_us();
    for host in HOSTS {
        let mut editor = editor_on(host, line_sketch());
        type_w(&mut editor, "bodyy + 1mm");
        let pad = |number: &str| {
            let mut pad = EditorPad::new_default(number.to_string(), (0.0, 0.0));
            pad.size_mm = (1.0, 1.0);
            pad.layers = vec![signex_library::primitive::footprint::LayerId::new("F.Cu")];
            pad
        };
        let pads = vec![pad("1"), pad("2")];
        let table = editor.state.parameters_table.as_deref().expect("open");
        let error = problems_of(&editor, table, &i18n)
            .into_iter()
            .map(|line| line.into_string())
            .collect::<Vec<_>>();
        assert_eq!(error.len(), 1, "{host:?}: w is in error");
        let error = &error[0];
        let problems = problems_of(&editor, table, &i18n);
        let lines: Vec<String> = failed_lines(problems, &pads, &i18n)
            .into_iter()
            .map(|line| line.into_string())
            .collect();
        assert_eq!(lines.len(), 2, "the error and one overlap");
        assert_eq!(&lines[0], error, "the error line comes first");
        assert!(lines[1].contains('1') && lines[1].contains('2'));
    }
}
