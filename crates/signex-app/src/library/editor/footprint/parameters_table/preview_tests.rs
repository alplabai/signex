//! Mockup row 189 and the 2026-11-24 decision: the live bake preview under
//! the parameters table redraws on each keystroke, keeps the last good bake
//! marked while an expression is in error, reads the current solve without
//! solving again, and sits beside Live checks, whose error line comes
//! first. On the footprint sketch and the board sketch alike.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use signex_library::{Footprint, FootprintFile};
use signex_sketch::array::{Array, ArrayId, ArrayKind, NumberingScheme};
use signex_sketch::attr::PadAttr;
use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::{ConstraintId, SketchEntityId};
use signex_sketch::parameter::resolve;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use signex_sketch::sketch::SketchData;
use signex_sketch::{SolveError, Solver};
use signex_types::sketch::SketchHost;

use super::SketchParametersMsg;
use super::checks::failed_lines;
use super::preview::{
    BakePreview, Freshness, Mm, PadsHidden, PreviewPad, failing_parameters, of_editor,
};
use super::preview_canvas::{PreviewDrawing, arrow_head, dimension_label, label_rect};
use super::view::problems_of;
use crate::app::FootprintDocument;
use crate::library::editor::footprint::sketch_solve_task::{
    SolveOutcome, dispatch_pending, run_solve, run_solve_with, snapshot_request,
};
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
        let preview = BakePreview::of(&sketch, &solve, host, &BTreeSet::new(), Freshness::Current);
        assert!((drawn_length(&preview) - 2.0).abs() < EPS, "{host:?}");
        assert!((preview.dimensions[0].value - 2.0).abs() < EPS);
    }
}

/// Mockup row 189: the preview draws each driving dimension's line, end
/// to end over what it measures, with its label in the middle of it.
#[test]
fn a_driving_dimension_draws_its_line_with_the_label_in_the_middle() {
    let near = |p: Mm, q: Mm| (p.0 - q.0).hypot(p.1 - q.1) < EPS;
    for host in HOSTS {
        let editor = editor_on(host, line_sketch());
        let preview = of_editor(&editor).expect("solved on entry");
        let edge = &preview.edges[0];
        let (a, b) = (edge[0], edge[edge.len() - 1]);
        let dimension = &preview.dimensions[0];
        let (from, to) = dimension.span.expect("a distance has a line");
        assert!(
            (near(from, a) && near(to, b)) || (near(from, b) && near(to, a)),
            "{host:?}: {from:?}..{to:?} is not the measured line {a:?}..{b:?}"
        );
        let middle = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
        assert!(near(dimension.at, middle), "{host:?}: the label sits on it");
    }
}

/// The label pill holds its text and the arrowheads point at the line's
/// ends, both sized from the type scale.
#[test]
fn a_dimension_label_pill_holds_its_text_and_arrows_point_out() {
    use iced::Point;
    let centre = Point::new(100.0, 50.0);
    let short = label_rect(centre, "2 mm");
    let long = label_rect(centre, "w = 2 mm");
    assert!(short.contains(centre), "centred on its place");
    assert!(long.width > short.width, "a longer label is wider");
    let [tip, p, q] = arrow_head(Point::ORIGIN, Point::new(100.0, 0.0));
    assert_eq!(tip, Point::ORIGIN);
    assert!(p.x > 0.0 && q.x > 0.0, "the head opens back along the line");
    assert!((p.y + q.y).abs() < 1e-4, "symmetric about it");
}

/// Mockup row 189: the table is a panel across the canvas, not a card at
/// its top right; the live preview sits under the rows, wider than the
/// Live checks beside it.
#[test]
fn the_table_is_a_panel_across_the_canvas_with_the_preview_beside_live_checks() {
    use signex_types::theme::{ThemeId, theme_tokens};
    let editor = editor_on(SketchHost::Footprint, line_sketch());
    let tokens = theme_tokens(ThemeId::GraphiteDark);
    let i18n = signex_i18n::I18n::en_us();
    let window = iced::Size::new(1600.0, 1000.0);
    let texts =
        crate::layout_probe::lay_out_texts(super::view::overlay(&editor, &tokens, &i18n), window);
    let find = |label: &str| {
        texts
            .iter()
            .find(|t| t.text.replace(['\u{2068}', '\u{2069}'], "") == label)
            .unwrap_or_else(|| panic!("no {label:?} in {texts:#?}"))
            .bounds
    };
    let title = find("Parameters");
    assert!(
        title.x < window.width / 8.0,
        "the panel starts at the canvas's left: {title:?}"
    );
    let add = find("Add parameter");
    let preview = find("Live bake preview");
    let checks = find("Live checks");
    assert!(preview.y > add.y && checks.y > add.y, "under the table");
    assert!(
        checks.x - preview.x > window.width / 2.0,
        "the preview takes the wider share: {preview:?} {checks:?}"
    );
}

/// What reaches the layer under the table in [`table_over_a_probe`].
#[derive(Debug, Clone, PartialEq)]
enum Probe {
    /// The canvas under the table saw the press or the wheel.
    Below,
    /// The table itself published something.
    Table,
}

/// The open table over a layer that reports every press and wheel, as
/// the sketch canvas under it would act on them.
fn table_over_a_probe<'a>(
    editor: &'a FootprintDocument,
    tokens: &'a signex_types::theme::ThemeTokens,
    i18n: &signex_i18n::I18n,
) -> iced::Element<'a, Probe> {
    use iced::Widget as _;
    use iced::widget::{Space, Stack, mouse_area};
    let below = mouse_area(
        Space::new()
            .width(iced::Length::Fill)
            .height(iced::Length::Fill),
    )
    .on_press(Probe::Below)
    .on_scroll(|_| Probe::Below);
    Stack::new()
        .width(iced::Length::Fill)
        .height(iced::Length::Fill)
        .push(below.boxed())
        .push(
            super::view::overlay(editor, tokens, i18n)
                .map(|_| Probe::Table)
                .boxed(),
        )
        .boxed()
}

/// Mockup row 189: the table panel is opaque. A press or a wheel on the
/// live preview never reaches the sketch canvas under it (an invisible
/// click would draw there), while the strip above the panel still does.
#[test]
fn a_press_or_wheel_on_the_table_never_reaches_the_canvas_under_it() {
    use crate::layout_probe::{lay_out_texts, messages_on_click_at, messages_on_scroll_at};
    use signex_types::theme::{ThemeId, theme_tokens};
    let tokens = theme_tokens(ThemeId::GraphiteDark);
    let i18n = signex_i18n::I18n::en_us();
    let window = iced::Size::new(1600.0, 1000.0);
    let body = crate::styles::TypeScale::DEFAULT.body;
    for host in HOSTS {
        let editor = editor_on(host, line_sketch());
        let (e, t, i) = (&editor, &tokens, &i18n);
        let probe = move || table_over_a_probe(e, t, i);
        let texts = lay_out_texts(probe(), window);
        let title = crate::layout_probe::find(&texts, "Live bake preview").bounds;
        // Inside the drawing, under the preview card's title.
        let on_preview = iced::Point::new(title.center_x(), title.y + body * 4.0);
        let pressed = messages_on_click_at(probe(), window, None, on_preview);
        assert!(!pressed.contains(&Probe::Below), "{host:?}: {pressed:?}");
        let wheel = iced::mouse::ScrollDelta::Lines { x: 0.0, y: 1.0 };
        let wheeled = messages_on_scroll_at(probe(), window, on_preview, wheel);
        assert!(!wheeled.contains(&Probe::Below), "{host:?}: {wheeled:?}");
        let above = iced::Point::new(window.width / 2.0, body);
        let through = messages_on_click_at(probe(), window, None, above);
        assert!(
            through.contains(&Probe::Below),
            "{host:?}: above the panel the canvas still hears the press: {through:?}"
        );
    }
}

/// The panel ends above the sketch mode bar: even laid over it, Bake
/// still takes a click.
#[test]
fn the_sketch_mode_bar_stays_in_reach_under_the_open_table() {
    use crate::library::editor::standalone::sketch_exit::footprint_overlay;
    use crate::library::messages::{LibraryMessage, PrimitiveEdit};
    use iced::Widget as _;
    use signex_types::theme::{ThemeId, theme_tokens};
    let editor = editor_on(SketchHost::Footprint, line_sketch());
    let tokens = theme_tokens(ThemeId::GraphiteDark);
    let i18n = signex_i18n::I18n::en_us();
    let window = iced::Size::new(1600.0, 1000.0);
    let both = iced::widget::Stack::new()
        .width(iced::Length::Fill)
        .height(iced::Length::Fill)
        .push(footprint_overlay(&editor, &tokens, &i18n))
        .push(super::view::overlay(&editor, &tokens, &i18n))
        .boxed();
    let clicked = crate::layout_probe::messages_on_click(both, window, "Bake");
    assert!(
        clicked.iter().any(|m| matches!(
            m,
            LibraryMessage::PrimitiveEditorEvent {
                msg: PrimitiveEdit::Footprint(M::SketchBakeRequested),
                ..
            }
        )),
        "the table covers Bake"
    );
}

/// A preview label is plain text: Fluent's bidi isolation marks would
/// count as characters and widen its pill.
#[test]
fn a_preview_dimension_label_carries_no_isolation_marks() {
    let i18n = signex_i18n::I18n::en_us();
    let editor = editor_on(SketchHost::Footprint, line_sketch());
    let preview = of_editor(&editor).expect("solved on entry");
    let label = dimension_label(&preview.dimensions[0], &i18n);
    assert!(label.starts_with("w = "), "named after w: {label:?}");
    assert!(!label.contains(['\u{2068}', '\u{2069}']), "{label:?}");
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
    let preview = BakePreview::of(
        &sketch,
        &solve,
        SketchHost::Footprint,
        &BTreeSet::new(),
        Freshness::Current,
    );
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
            depopulation: None,
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
    let footprint = BakePreview::of(
        &sketch,
        &solve,
        SketchHost::Footprint,
        &none,
        Freshness::Current,
    );
    assert_eq!(footprint.pads.len(), 3, "the source and its two copies");
    for pad in &footprint.pads {
        let PreviewPad::Outline(corners) = pad else {
            panic!("a 0.6 x 1 mm pad is drawn as its outline");
        };
        let width = (corners[1].0 - corners[0].0).abs();
        assert!((width - 0.6).abs() < EPS, "sized by pad_w");
    }
    let board = BakePreview::of(
        &sketch,
        &solve,
        SketchHost::Board,
        &none,
        Freshness::Current,
    );
    assert!(board.pads.is_empty());
    assert!(footprint.pads_hidden.is_none() && board.pads_hidden.is_none());
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

    let read = BakePreview::of(
        &sketch,
        &solve,
        SketchHost::Footprint,
        &BTreeSet::new(),
        Freshness::Current,
    );
    assert_eq!(read.pads.len(), 1);
    assert!((outline_angle_deg(&read.pads[0]).abs() - 30.0).abs() < 1e-6);

    sketch.parameters.insert("junk", "bodyy + 1mm");
    let failing = failing_parameters(&sketch);
    assert_eq!(failing, BTreeSet::from(["junk".to_string()]));
    let last_good = BakePreview::of(
        &sketch,
        &solve,
        SketchHost::Footprint,
        &failing,
        Freshness::Current,
    );
    for pad in &last_good.pads {
        assert!(
            (outline_angle_deg(pad).abs() - 30.0).abs() < 1e-6,
            "a drawn pad keeps the solved angle"
        );
    }
    assert!(last_good.pads.is_empty(), "the families cannot be read");
    assert_eq!(
        last_good.pads_hidden,
        Some(PadsHidden::UntilExpressionReads),
        "and the card says so"
    );
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

/// A drag tick on the line's free end `b`, its solve left owed.
fn drag_free_end(editor: &mut FootprintDocument) {
    let sketch = editor.primitive().sketch.as_ref().expect("a sketch");
    let b = sketch
        .entities
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::Point { .. }))
        .nth(1)
        .expect("the line's free end")
        .id;
    apply_footprint_primitive_edit(
        editor,
        M::SketchMovePoint {
            id: b,
            dx: 0.0,
            dy: 1.0,
        },
    );
}

/// The owed solve starts, and answers `outcome` instead of solving.
fn answer_owed_solve(editor: &mut FootprintDocument, outcome: SolveOutcome) {
    let path = PathBuf::from("preview.snxfpt");
    let _task = dispatch_pending(editor, &path);
    let request = snapshot_request(editor).expect("a sketch");
    let answer = run_solve_with(request, move |_, _| outcome);
    apply_footprint_primitive_edit(editor, answer);
}

fn did_not_converge() -> SolveOutcome {
    SolveOutcome::SolveFailed(SolveError::DidNotConverge {
        iters: 1,
        final_residual_norm: 1.0,
    })
}

/// The preview card's texts, in order.
fn card_texts(editor: &FootprintDocument) -> Vec<String> {
    use signex_types::theme::{ThemeId, theme_tokens};
    let tokens = theme_tokens(ThemeId::GraphiteDark);
    let i18n = signex_i18n::I18n::en_us();
    crate::layout_probe::lay_out_texts(
        super::view::overlay(editor, &tokens, &i18n),
        iced::Size::new(1600.0, 1000.0),
    )
    .into_iter()
    .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
    .collect()
}

const UNSOLVED: &str = "The sketch does not solve: this is the last good bake.";

/// #2856, Caner 2026-11-27: a failed solve still clears `last_solve` (the
/// canvas shows the sketch unsolved, #613), but the preview keeps the last
/// good bake, dimmed, with a "last good" caption, until a solve succeeds.
#[test]
fn a_failed_solve_keeps_the_last_good_preview_dimmed_until_a_solve_succeeds() {
    let mut editor = editor_on(SketchHost::Footprint, line_sketch());
    assert!(!card_texts(&editor).iter().any(|t| t == UNSOLVED));
    drag_free_end(&mut editor);
    answer_owed_solve(&mut editor, did_not_converge());
    assert!(
        editor.state.last_solve.is_none(),
        "the canvas shows it unsolved"
    );

    let preview = of_editor(&editor).expect("the last good solve is drawn");
    assert!(preview.is_unsolved, "marked as the last good bake");
    assert!(!preview.is_last_good, "no expression is in error");
    assert!(
        (drawn_length(&preview) - 2.0).abs() < EPS,
        "at the last good solve"
    );
    assert!(
        card_texts(&editor).iter().any(|t| t == UNSOLVED),
        "captioned"
    );
    let look = super::view::Look::of(&signex_types::theme::theme_tokens(
        signex_types::theme::ThemeId::GraphiteDark,
    ));
    let drawing = PreviewDrawing::of(preview, Vec::new(), iced::Color::BLACK, look);
    assert!(drawing.edge.a < look.ink.a, "and dimmed");
    assert!(drawing.label.a < look.muted.a);

    // A second failure keeps the same last good bake.
    drag_free_end(&mut editor);
    answer_owed_solve(&mut editor, did_not_converge());
    let preview = of_editor(&editor).expect("still drawn");
    assert!(preview.is_unsolved && (drawn_length(&preview) - 2.0).abs() < EPS);

    type_w(&mut editor, "3mm");
    let preview = of_editor(&editor).expect("solved");
    assert!(!preview.is_unsolved, "a successful solve replaces it");
    assert!((drawn_length(&preview) - 3.0).abs() < EPS);
    assert!(
        editor.state.last_good_solve.is_none(),
        "and drops the cache"
    );
    assert!(!card_texts(&editor).iter().any(|t| t == UNSOLVED));
    let current = PreviewDrawing::of(preview, Vec::new(), iced::Color::BLACK, look);
    assert_eq!(current.edge, look.ink, "drawn at full strength again");
}

/// A last good bake after a failed solve hides the pads: the table's units
/// may have changed since that solve.
#[test]
fn a_failed_solve_hides_the_pads_of_the_last_good_preview() {
    let sketch = pad_sketch();
    let params = resolve(&sketch.parameters).unwrap();
    let solve = Solver::default().solve(&sketch, &params).unwrap();
    let none = BTreeSet::new();
    let host = SketchHost::Footprint;
    let preview = BakePreview::of(&sketch, &solve, host, &none, Freshness::LastGood);
    assert!(preview.pads.is_empty());
    assert_eq!(preview.pads_hidden, Some(PadsHidden::UntilSketchSolves));
    assert!(preview.is_unsolved && !preview.is_last_good);
}

/// #2859 part 3, Caner 2026-11-27: between an edit and the solve that
/// lands for it, the solve's numbers need not match the table's units, so
/// the pads are hidden, saying so, and come back when the solve lands. The
/// edges stay, at full strength.
#[test]
fn pads_are_hidden_while_a_solve_is_owed() {
    const PADS_WAIT: &str = "Pads are hidden until the sketch solves.";
    let mut editor = editor_on(SketchHost::Footprint, pad_sketch());
    assert_eq!(of_editor(&editor).expect("solved").pads.len(), 3);
    let pad = editor
        .primitive()
        .sketch
        .as_ref()
        .expect("a sketch")
        .entities[0]
        .id;
    apply_footprint_primitive_edit(
        &mut editor,
        M::SketchMovePoint {
            id: pad,
            dx: 0.0,
            dy: 1.0,
        },
    );
    assert!(editor.state.sketch_solve.is_solve_owed());
    let owed = of_editor(&editor).expect("the last solve is drawn");
    assert!(owed.pads.is_empty(), "no pads read with stale units");
    assert_eq!(owed.pads_hidden, Some(PadsHidden::UntilSketchSolves));
    assert!(!owed.is_unsolved && !owed.is_last_good, "not marked stale");
    assert!(card_texts(&editor).iter().any(|t| t == PADS_WAIT));

    let path = PathBuf::from("preview.snxfpt");
    let _task = dispatch_pending(&mut editor, &path);
    assert!(editor.state.sketch_solve.is_solve_owed(), "still running");
    assert!(of_editor(&editor).expect("drawn").pads.is_empty());
    let answer = run_solve(snapshot_request(&editor).expect("a sketch"));
    apply_footprint_primitive_edit(&mut editor, answer);
    assert!(!editor.state.sketch_solve.is_solve_owed());
    let landed = of_editor(&editor).expect("solved");
    assert_eq!(landed.pads.len(), 3, "the solve landed: pads are back");
    assert_eq!(landed.pads_hidden, None);
    assert!(!card_texts(&editor).iter().any(|t| t == PADS_WAIT));
}

#[test]
fn an_owed_solve_hides_the_pads_but_does_not_dim_the_preview() {
    let sketch = pad_sketch();
    let params = resolve(&sketch.parameters).unwrap();
    let solve = Solver::default().solve(&sketch, &params).unwrap();
    let none = BTreeSet::new();
    let host = SketchHost::Footprint;
    let current = BakePreview::of(&sketch, &solve, host, &none, Freshness::Current);
    let owed = BakePreview::of(&sketch, &solve, host, &none, Freshness::Owed);
    assert!(owed.pads.is_empty());
    assert_eq!(owed.pads_hidden, Some(PadsHidden::UntilSketchSolves));
    assert!(!owed.is_unsolved);
    assert_eq!(owed.edges, current.edges);
    assert_eq!(owed.dimensions, current.dimensions);
}
