//! m190, bake: roles, the preview's counts, Close gap, and an open profile.
//! Close gap heals up to 0.05 mm (decision of 2026-11-20).

use signex_app::library::editor::footprint::EditorMode;
use signex_app::library::editor::footprint::sketch_mode::BakePreview;
use signex_app::library::messages::{FootprintEditorMsg, RoleTag};
use signex_library::Footprint;
use signex_sketch::Solver;
use signex_sketch::attr::CourtyardAttr;
use signex_sketch::entity::EntityKind;
use signex_sketch::id::SketchEntityId;
use signex_sketch::parameter::resolve;
use signex_types::sketch::baked_kinds::BakedKind;

use super::support::{FpRig, SketchBuilder, open_footprint, sketch_rig};

/// A courtyard square that stops `gap` short of its first corner, solved,
/// in Sketch mode; the two open ends.
fn open_courtyard(gap: f64) -> (FpRig, SketchEntityId, SketchEntityId) {
    let mut s = SketchBuilder::new();
    let corners = [(0.0, 0.0), (5.0, 0.0), (5.0, 5.0), (0.0, 5.0), (0.0, gap)];
    let points: Vec<SketchEntityId> = corners.iter().map(|&(x, y)| s.point(x, y)).collect();
    for pair in points.windows(2) {
        s.line(pair[0], pair[1]);
    }
    for entity in &mut s.data.entities {
        if matches!(entity.kind, EntityKind::Line { .. }) {
            entity.courtyard = Some(CourtyardAttr {});
        }
    }
    let params = resolve(&s.data.parameters).expect("no parameters");
    let solve = Solver::default().solve(&s.data, &params).expect("solves");
    let mut footprint = Footprint::empty("courtyard");
    footprint.sketch = Some(s.data);
    let mut rig = open_footprint(footprint);
    rig.send(FootprintEditorMsg::SetMode(EditorMode::Sketch));
    rig.editor_mut().state.last_solve = Some(solve);
    rig.editor_mut().mark_saved();
    (rig, points[0], points[4])
}

fn offered(rig: &FpRig) -> bool {
    rig.editor()
        .state
        .gap_offer(rig.editor().primitive().sketch.as_ref())
        .is_some()
}

/// A closed square of four lines; the lines.
fn square() -> (FpRig, Vec<SketchEntityId>) {
    let mut s = SketchBuilder::new();
    let corners = [
        s.point(0.0, 0.0),
        s.point(5.0, 0.0),
        s.point(5.0, 5.0),
        s.point(0.0, 5.0),
    ];
    let lines = (0..4)
        .map(|i| s.line(corners[i], corners[(i + 1) % 4]))
        .collect();
    (sketch_rig(s.data), lines)
}

/// What the bake preview counts for `kind`.
fn preview_count(rig: &FpRig, kind: BakedKind) -> Option<usize> {
    match BakePreview::of_footprint(rig.editor().primitive()) {
        BakePreview::Counts(counts) => counts
            .into_iter()
            .find_map(|(baked, count)| (baked == kind).then_some(count)),
        BakePreview::Refused(why) => panic!("the footprint preview refused: {why}"),
    }
}

#[test]
fn m190_a_closed_profile_given_the_courtyard_role_bakes_a_courtyard() {
    let (mut rig, lines) = square();
    assert_eq!(preview_count(&rig, BakedKind::Courtyard), Some(0));

    for &id in &lines {
        rig.send(FootprintEditorMsg::SketchSetRole {
            id,
            role: RoleTag::Courtyard,
        });
    }

    assert_eq!(preview_count(&rig, BakedKind::Courtyard), Some(1));
    assert!(rig.dirty());
}

#[test]
fn m190_undo_takes_a_role_back_off_the_entity() {
    let (mut rig, lines) = square();
    rig.send(FootprintEditorMsg::SketchSetRole {
        id: lines[0],
        role: RoleTag::Courtyard,
    });

    rig.undo();

    let line = rig
        .sketch()
        .entities
        .iter()
        .find(|e| e.id == lines[0])
        .expect("the line");
    assert!(line.courtyard.is_none());
    assert!(!rig.dirty());
}

#[test]
fn m190_close_gap_joins_the_ends_as_one_undo_step() {
    let (mut rig, keep, merge) = open_courtyard(0.03);
    assert!(offered(&rig), "an open courtyard offers Close gap");
    let steps = rig.steps();

    rig.send(FootprintEditorMsg::SketchCloseGapRequested { keep, merge });

    assert!(!offered(&rig), "the profile is closed");
    assert_eq!(rig.steps(), steps + 1);
    assert!(rig.dirty());
    rig.undo();
    assert!(rig.sketch().entities.iter().any(|e| e.id == merge));
    assert!(!rig.dirty());
}

#[test]
fn m190_close_gap_refuses_a_gap_wider_than_the_tolerance() {
    let (mut rig, keep, merge) = open_courtyard(0.2);
    let (before, steps) = (rig.sketch().clone(), rig.steps());

    rig.send(FootprintEditorMsg::SketchCloseGapRequested { keep, merge });

    assert_eq!(rig.sketch(), &before);
    assert_eq!(rig.steps(), steps);
    assert!(!rig.dirty());
}

/// m190: "Errors stop the bake" and "Bake is disabled until they are fixed".
/// The 2026-11-20 warn-only decision names the board's outline checks.
#[test]
fn m190_bake_with_an_open_profile_does_not_leave_sketch_mode() {
    let (mut rig, _, _) = open_courtyard(0.2);
    assert!(offered(&rig), "the profile is open");

    rig.send(FootprintEditorMsg::SketchBakeRequested);

    assert_eq!(
        rig.editor().state.mode,
        EditorMode::Sketch,
        "an error stops the bake until it is fixed"
    );
}
