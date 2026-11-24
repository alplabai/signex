//! The Mirror ghost follows the Mirror card (mockup row 191): it draws the
//! card's line and picks and the copies OK writes, never the selection.

use signex_sketch::SketchData;
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};

use super::{MirrorGhost, Positions};
use crate::library::editor::footprint::sketch_mode::mirror_card::{
    MirrorCard, MirrorField, MirrorObject,
};
use crate::library::editor::footprint::state::FootprintEditorState;

const TOL: f64 = 0.3;

/// A centerline at x = 0, a Line from (2, 0) to (4, 1) and a Point at
/// (3, 2).
struct Drawing {
    sketch: SketchData,
    centerline: SketchEntityId,
    line: SketchEntityId,
    point: SketchEntityId,
}

fn drawing() -> Drawing {
    let plane = PlaneId::new();
    let mut sketch = SketchData {
        planes: vec![Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        }],
        ..SketchData::default()
    };
    let mut add = |kind| {
        let id = SketchEntityId::new();
        sketch.entities.push(Entity::new(id, plane, kind));
        id
    };
    let c0 = add(EntityKind::Point { x: 0.0, y: -5.0 });
    let c1 = add(EntityKind::Point { x: 0.0, y: 5.0 });
    let centerline = add(EntityKind::Line { start: c0, end: c1 });
    let a = add(EntityKind::Point { x: 2.0, y: 0.0 });
    let b = add(EntityKind::Point { x: 4.0, y: 1.0 });
    let line = add(EntityKind::Line { start: a, end: b });
    let point = add(EntityKind::Point { x: 3.0, y: 2.0 });
    for e in &mut sketch.entities {
        e.centerline = e.id == centerline;
    }
    Drawing {
        sketch,
        centerline,
        line,
        point,
    }
}

fn card(objects: Vec<MirrorObject>, line: Option<SketchEntityId>) -> MirrorCard {
    MirrorCard {
        objects,
        line,
        keep_linked: true,
        field: MirrorField::Objects,
    }
}

fn near(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9
}

fn plan(d: &Drawing, state: &FootprintEditorState, card: Option<&MirrorCard>) -> MirrorGhost {
    let at = Positions {
        sketch: &d.sketch,
        state,
    };
    MirrorGhost::plan(&at, card, Some((10.0, 10.0)), TOL)
}

#[test]
fn the_ghost_draws_the_copies_ok_writes_about_the_cards_line() {
    let d = drawing();
    let state = FootprintEditorState::empty();
    let picked = vec![MirrorObject::Entity(d.line), MirrorObject::Entity(d.point)];
    let ghost = plan(&d, &state, Some(&card(picked, Some(d.centerline))));

    assert_eq!(ghost.copies.len(), 1);
    let copy = &ghost.copies[0];
    assert!(near(copy[0], (-2.0, 0.0)) && near(copy[1], (-4.0, 1.0)));
    assert_eq!(ghost.copied_points.len(), 1);
    assert!(near(ghost.copied_points[0], (-3.0, 2.0)));
    // The mirror line and the picked Line are drawn wide, the Point ringed.
    assert_eq!(ghost.wide.len(), 2);
    assert!(near(ghost.wide[0][0], (0.0, -5.0)));
    assert!(near(ghost.wide[1][0], (2.0, 0.0)));
    assert_eq!(ghost.picked_points.len(), 1);
    assert!(near(ghost.picked_points[0], (3.0, 2.0)));
}

#[test]
fn without_a_line_on_the_card_the_ghost_draws_no_copy_whatever_is_selected() {
    let d = drawing();
    let mut state = FootprintEditorState::empty();
    // The old ghost mirrored about a selected Line.
    state.selected_sketch = Some(d.centerline);
    let picked = vec![MirrorObject::Entity(d.line)];
    let ghost = plan(&d, &state, Some(&card(picked, None)));

    assert!(ghost.copies.is_empty() && ghost.copied_points.is_empty());
    assert_eq!(ghost.wide.len(), 1, "only the picked Line");
}

#[test]
fn the_ghost_shows_the_line_the_next_pick_takes() {
    let d = drawing();
    let state = FootprintEditorState::empty();
    let at = Positions {
        sketch: &d.sketch,
        state: &state,
    };
    let empty = card(Vec::new(), None);
    let on_centerline = MirrorGhost::plan(&at, Some(&empty), Some((0.05, 2.0)), TOL);
    assert_eq!(on_centerline.wide.len(), 1, "the centerline a click takes");

    let on_plain = MirrorGhost::plan(&at, Some(&empty), Some((3.0, 0.5)), TOL);
    assert!(
        on_plain.wide.is_empty(),
        "a plain Line is no mirror line yet"
    );

    let mut line_field = empty.clone();
    line_field.field = MirrorField::Line;
    let focused = MirrorGhost::plan(&at, Some(&line_field), Some((3.0, 0.5)), TOL);
    assert_eq!(focused.wide.len(), 1, "with Mirror line focused any Line");
}

#[test]
fn no_card_no_ghost() {
    let d = drawing();
    let mut state = FootprintEditorState::empty();
    state.selected_sketch = Some(d.centerline);
    assert_eq!(plan(&d, &state, None), MirrorGhost::default());
}
