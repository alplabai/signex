//! mockup row 179d / mockup row 190 — the outline checks a board sketch editor reads from its
//! last solve, the lines Properties and the bake preview show for them, and
//! the bake preview's canvas state on a footprint.

use std::path::PathBuf;
use std::sync::Arc;

use signex_bake::board::CheckState;
use signex_i18n::RenderTranslatable as _;
use signex_library::FootprintFile;
use signex_sketch::Solver;
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::parameter::resolve;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use signex_sketch::sketch::SketchData;

use super::*;
use crate::library::editor::footprint::state::{
    BoardBackdrop, CheckLine, CheckStatus, EditorMode, check_lines,
};
use crate::library::editor::footprint::updates::apply_footprint_primitive_edit;
use crate::library::messages::FootprintEditorMsg as M;

/// A sketch whose only geometry is a closed polyline through `corners`,
/// tagged as board outline, and its edges.
fn outline_sketch(corners: &[(f64, f64)]) -> (SketchData, Vec<SketchEntityId>) {
    let plane = PlaneId::new();
    let mut sketch = SketchData::default();
    sketch.planes.push(Plane {
        id: plane,
        kind: PlaneKind::BoardTop,
    });
    let add = |sketch: &mut SketchData, kind| {
        let id = SketchEntityId::new();
        sketch.entities.push(Entity::new(id, plane, kind));
        id
    };
    let points: Vec<SketchEntityId> = corners
        .iter()
        .map(|&(x, y)| add(&mut sketch, EntityKind::Point { x, y }))
        .collect();
    let n = points.len();
    let edges: Vec<SketchEntityId> = (0..n)
        .map(|i| {
            let (start, end) = (points[i], points[(i + 1) % n]);
            add(&mut sketch, EntityKind::Line { start, end })
        })
        .collect();
    for entity in &mut sketch.entities {
        if edges.contains(&entity.id) {
            entity.board_outline = true;
        }
    }
    (sketch, edges)
}

/// An editor over `sketch`, solved once; a board sketch editor when
/// `board` (it carries a backdrop).
fn solved_editor(sketch: SketchData, board: bool) -> FootprintDocument {
    let params = resolve(&sketch.parameters).unwrap();
    let solve = Solver::default().solve(&sketch, &params).unwrap();
    let mut fp = Footprint::empty("t");
    fp.sketch = Some(sketch);
    let mut editor =
        FootprintDocument::new(PathBuf::from("t.snxpcb"), FootprintFile::from_footprint(fp));
    if board {
        editor.state.board_backdrop = Some(Arc::new(BoardBackdrop::default()));
    }
    editor.state.last_solve = Some(solve);
    editor
}

fn lines_of(editor: &FootprintDocument) -> Vec<CheckLine> {
    let sketch = editor.primitive().sketch.as_ref();
    check_lines(editor.state.board_checks(sketch).as_ref())
}

/// Rendered in en-US, without Fluent's isolation marks.
fn english(line: &CheckLine) -> String {
    line.text
        .render_en_us()
        .replace(['\u{2068}', '\u{2069}'], "")
}

const BOARD: [(f64, f64); 4] = [(0.0, 0.0), (50.0, 0.0), (50.0, 30.0), (0.0, 30.0)];

#[test]
fn a_board_editor_checks_its_outline_and_a_footprint_editor_does_not() {
    let (sketch, _) = outline_sketch(&BOARD);
    let board = solved_editor(sketch.clone(), true);
    let checks = board
        .state
        .board_checks(board.primitive().sketch.as_ref())
        .expect("a board with a solve has checks");
    assert_eq!(checks.closed, CheckState::Passed);

    let footprint = solved_editor(sketch, false);
    assert_eq!(
        footprint
            .state
            .board_checks(footprint.primitive().sketch.as_ref()),
        None,
        "a footprint has no outline checks"
    );
}

#[test]
fn a_clean_outline_reads_ok_and_the_router_check_says_why_it_did_not_run() {
    let (sketch, _) = outline_sketch(&BOARD);
    let lines = lines_of(&solved_editor(sketch, true));
    let statuses: Vec<CheckStatus> = lines.iter().map(|l| l.status).collect();
    assert_eq!(
        statuses,
        [
            CheckStatus::Passed,
            CheckStatus::Passed,
            CheckStatus::Passed,
            CheckStatus::NotChecked,
        ]
    );
    assert_eq!(english(&lines[0]), "Closed: one loop");
    assert_eq!(
        english(&lines[3]),
        "Inside corners: not checked, the project's fab profile names no router tool"
    );
    assert!(lines.iter().all(|l| l.place.is_none()), "nothing to select");
}

#[test]
fn a_crossing_line_names_the_point_and_selects_an_edge_that_crosses() {
    let (sketch, edges) = outline_sketch(&[(0.0, 0.0), (10.0, 10.0), (10.0, 0.0), (0.0, 10.0)]);
    let lines = lines_of(&solved_editor(sketch, true));
    let failed: Vec<&CheckLine> = lines
        .iter()
        .filter(|l| l.status == CheckStatus::Failed)
        .collect();
    assert_eq!(failed.len(), 1, "{lines:#?}");
    assert_eq!(english(failed[0]), "Self-intersection at (5.00, 5.00)");
    let place = failed[0].place.expect("a crossing names its edges");
    assert!(place == edges[0] || place == edges[2], "{place:?}");
}

#[test]
fn an_open_outline_line_names_the_gap_and_the_later_checks_wait() {
    let (mut sketch, edges) = outline_sketch(&BOARD);
    // Cut the left edge loose from the first corner, 0.02 mm short.
    let plane = sketch.planes[0].id;
    let short = SketchEntityId::new();
    sketch.entities.push(Entity::new(
        short,
        plane,
        EntityKind::Point { x: 0.0, y: 0.02 },
    ));
    let left = sketch
        .entities
        .iter_mut()
        .find(|e| e.id == edges[3])
        .unwrap();
    let EntityKind::Line { start, .. } = left.kind else {
        unreachable!()
    };
    left.kind = EntityKind::Line { start, end: short };

    let lines = lines_of(&solved_editor(sketch, true));

    assert_eq!(lines[0].status, CheckStatus::Failed);
    assert_eq!(
        english(&lines[0]),
        "Outline open: 0.020 mm gap at (0.00, 0.01)"
    );
    assert!(lines[0].place.is_some(), "a click selects an open end");
    assert_eq!(
        english(&lines[1]),
        "Self-intersection: not checked until the outline is one closed loop"
    );
}

#[test]
fn without_a_solve_there_are_no_checks_and_one_line_says_so() {
    let (sketch, _) = outline_sketch(&BOARD);
    let mut editor = solved_editor(sketch, true);
    editor.state.last_solve = None;
    assert_eq!(
        editor
            .state
            .board_checks(editor.primitive().sketch.as_ref()),
        None
    );
    let lines = lines_of(&editor);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].status, CheckStatus::NotChecked);
    assert_eq!(english(&lines[0]), "Not checked: the sketch is not solved.");
}

/// A footprint's preview draws its own fields: nothing board-shaped is
/// kept for the canvas, open or closed.
#[test]
fn a_footprint_preview_keeps_no_board_result() {
    let file = FootprintFile::from_footprint(Footprint::empty("t"));
    let mut editor = FootprintDocument::new(PathBuf::from("t.snxfpt"), file);
    apply_footprint_primitive_edit(&mut editor, M::SetMode(EditorMode::Sketch));
    apply_footprint_primitive_edit(&mut editor, M::SketchBakePreviewToggled);
    assert!(editor.state.sketch_exit.preview);
    assert!(editor.state.sketch_exit.board_result.is_none());
}

/// mockup row 179d: with the project's router tool, a sharp inside corner fails and
/// names its place; a convex board passes.
#[test]
fn with_a_router_tool_a_sharp_inside_corner_fails_and_names_its_point() {
    let l_board = [
        (0.0, 0.0),
        (20.0, 0.0),
        (20.0, 10.0),
        (10.0, 10.0),
        (10.0, 20.0),
        (0.0, 20.0),
    ];
    let (sketch, _) = outline_sketch(&l_board);
    let mut editor = solved_editor(sketch, true);
    editor.state.sketch_exit.router_tool_diameter = Some(2.0);
    let lines = lines_of(&editor);
    let failed: Vec<&CheckLine> = lines
        .iter()
        .filter(|l| l.status == CheckStatus::Failed)
        .collect();
    assert_eq!(failed.len(), 1, "{lines:#?}");
    assert_eq!(
        english(failed[0]),
        "Inside corner R 0.00 < R 1.00 at (10.00, 10.00)"
    );
    assert!(failed[0].place.is_some(), "a click selects the corner");

    let (sketch, _) = outline_sketch(&BOARD);
    let mut editor = solved_editor(sketch, true);
    editor.state.sketch_exit.router_tool_diameter = Some(2.0);
    let lines = lines_of(&editor);
    assert_eq!(
        english(&lines[3]),
        "Inside corners: the router can cut them"
    );
    assert_eq!(lines[3].status, CheckStatus::Passed);
}

/// mockup row 190: the open-outline line carries the Close gap offer of its two ends.
#[test]
fn an_open_outline_line_offers_close_gap_within_the_tolerance() {
    let (mut sketch, edges) = outline_sketch(&BOARD);
    let plane = sketch.planes[0].id;
    let short = SketchEntityId::new();
    sketch.entities.push(Entity::new(
        short,
        plane,
        EntityKind::Point { x: 0.0, y: 0.03 },
    ));
    let left = sketch
        .entities
        .iter_mut()
        .find(|e| e.id == edges[3])
        .unwrap();
    let EntityKind::Line { start, .. } = left.kind else {
        unreachable!()
    };
    left.kind = EntityKind::Line { start, end: short };

    let editor = solved_editor(sketch, true);
    let lines = lines_of(&editor);
    let offer = lines[0].close_gap.expect("the open line offers Close gap");
    assert!(offer.closes());
    assert!(offer.keep == short || offer.merge == short, "{offer:?}");
    let text = offer
        .text()
        .render_en_us()
        .replace(['\u{2068}', '\u{2069}'], "");
    assert_eq!(text, "Gap 0.03 mm, closes under the 0.05 mm heal tolerance");
    assert!(lines[1..].iter().all(|l| l.close_gap.is_none()));
}

/// mockup row 190: a footprint whose courtyard is open offers Close gap in its bake
/// preview; a closed one offers nothing.
#[test]
fn a_footprint_with_an_open_profile_offers_close_gap() {
    let (mut sketch, edges) = outline_sketch(&BOARD);
    for entity in &mut sketch.entities {
        if edges.contains(&entity.id) {
            entity.board_outline = false;
            entity.courtyard = Some(signex_sketch::attr::CourtyardAttr {});
        }
    }
    let closed = solved_editor(sketch.clone(), false);
    assert_eq!(
        closed.state.gap_offer(closed.primitive().sketch.as_ref()),
        None
    );

    let plane = sketch.planes[0].id;
    let short = SketchEntityId::new();
    sketch.entities.push(Entity::new(
        short,
        plane,
        EntityKind::Point { x: 0.0, y: 0.02 },
    ));
    let left = sketch
        .entities
        .iter_mut()
        .find(|e| e.id == edges[3])
        .unwrap();
    let EntityKind::Line { start, .. } = left.kind else {
        unreachable!()
    };
    left.kind = EntityKind::Line { start, end: short };
    let open = solved_editor(sketch, false);
    let offer = open
        .state
        .gap_offer(open.primitive().sketch.as_ref())
        .expect("an open courtyard offers Close gap");
    assert!(offer.closes());
    assert!((offer.gap_mm - 0.02).abs() < 1e-9);
}
