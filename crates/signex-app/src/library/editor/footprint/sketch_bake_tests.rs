//! #1544 — an empty bake of a sketch-owned kind clears that kind.

use signex_library::primitive::footprint::{Footprint, FpGraphic, FpGraphicKind};
use signex_sketch::SketchData;
use signex_sketch::attr::{BoardCutoutAttr, PourAttr, SilkAttr};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use signex_types::layer::SignexLayer;

use super::sketch_dispatch::apply_sketch_edit;
use super::sketch_mode::SketchEdit;
use super::state::FootprintEditorState;

/// A closed 5 mm square. Returns the footprint, the four edges and the
/// plane, so a test can tag, open or delete them.
fn square() -> (Footprint, [SketchEntityId; 4], PlaneId) {
    let plane = PlaneId::new();
    let pts: Vec<_> = (0..4).map(|_| SketchEntityId::new()).collect();
    let xy = [(0.0, 0.0), (5.0, 0.0), (5.0, 5.0), (0.0, 5.0)];
    let mut entities: Vec<Entity> = pts
        .iter()
        .zip(xy)
        .map(|(id, (x, y))| Entity::new(*id, plane, EntityKind::Point { x, y }))
        .collect();
    let edges: [SketchEntityId; 4] = std::array::from_fn(|_| SketchEntityId::new());
    for (i, id) in edges.iter().enumerate() {
        let (start, end) = (pts[i], pts[(i + 1) % 4]);
        entities.push(Entity::new(*id, plane, EntityKind::Line { start, end }));
    }
    let mut fp = Footprint::empty("bake");
    fp.sketch = Some(SketchData {
        planes: vec![Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        }],
        entities,
        ..SketchData::default()
    });
    (fp, edges, plane)
}

fn entity_mut(fp: &mut Footprint, id: SketchEntityId) -> &mut Entity {
    fp.sketch
        .as_mut()
        .unwrap()
        .entities
        .iter_mut()
        .find(|e| e.id == id)
        .unwrap()
}

fn rebuild(state: &mut FootprintEditorState, fp: &mut Footprint) {
    apply_sketch_edit(state, fp, SketchEdit::ForceRebuild).unwrap();
}

fn pour_attr() -> PourAttr {
    PourAttr {
        layer: SignexLayer::TopCopper,
        net: None,
        fill_type: Default::default(),
        thermal_relief: Default::default(),
        clearance_expr: None,
        min_thickness_expr: None,
        priority: 0,
    }
}

#[test]
fn breaking_a_pour_profile_clears_the_baked_pour() {
    let (mut fp, edges, _) = square();
    entity_mut(&mut fp, edges[0]).pour = Some(pour_attr());
    let mut state = FootprintEditorState::from_footprint(&fp);
    rebuild(&mut state, &mut fp);
    assert_eq!(fp.pours.len(), 1);

    // Dropping one edge opens the chain; the bake skips the pour.
    apply_sketch_edit(&mut state, &mut fp, SketchEdit::DeleteEntity(edges[2])).unwrap();
    assert!(fp.pours.is_empty(), "stale pour survived an open profile");
    assert!(
        state
            .solve_warnings
            .iter()
            .any(|w| w.contains("pours were cleared")),
        "{:?}",
        state.solve_warnings
    );
}

#[test]
fn deleting_the_pour_entity_clears_the_baked_pour() {
    let (mut fp, edges, _) = square();
    entity_mut(&mut fp, edges[0]).pour = Some(pour_attr());
    let mut state = FootprintEditorState::from_footprint(&fp);
    rebuild(&mut state, &mut fp);
    assert_eq!(fp.pours.len(), 1);

    entity_mut(&mut fp, edges[0]).pour = None;
    rebuild(&mut state, &mut fp);
    assert!(fp.pours.is_empty());
}

#[test]
fn breaking_or_removing_a_cutout_clears_it() {
    let (mut fp, edges, _) = square();
    entity_mut(&mut fp, edges[0]).board_cutout = Some(BoardCutoutAttr {
        edge_radius_expr: None,
        through: true,
    });
    let mut state = FootprintEditorState::from_footprint(&fp);
    rebuild(&mut state, &mut fp);
    assert_eq!(fp.cutouts.len(), 1);

    apply_sketch_edit(&mut state, &mut fp, SketchEdit::DeleteEntity(edges[2])).unwrap();
    assert!(
        fp.cutouts.is_empty(),
        "stale cutout survived an open profile"
    );
}

#[test]
fn a_silk_tag_that_stops_baking_clears_the_baked_silk() {
    let (mut fp, edges, _) = square();
    entity_mut(&mut fp, edges[0]).silk = Some(SilkAttr {
        layer: SignexLayer::TopSilk,
    });
    let mut state = FootprintEditorState::from_footprint(&fp);
    rebuild(&mut state, &mut fp);
    assert_eq!(fp.silk_f.len(), 1);

    // Retarget the tag to a non-silk layer: the bake skips it.
    entity_mut(&mut fp, edges[0]).silk = Some(SilkAttr {
        layer: SignexLayer::TopCopper,
    });
    rebuild(&mut state, &mut fp);
    assert!(fp.silk_f.is_empty(), "stale silk survived");
}

#[test]
fn silk_drawn_by_hand_survives_a_sketch_with_no_silk_entity() {
    let (mut fp, edges, _) = square();
    entity_mut(&mut fp, edges[0]).pour = Some(pour_attr());
    fp.silk_f.push(FpGraphic {
        kind: FpGraphicKind::Line {
            from: [0.0, 0.0],
            to: [1.0, 0.0],
        },
        stroke_width: 0.15,
        filled: false,
    });
    let mut state = FootprintEditorState::from_footprint(&fp);
    rebuild(&mut state, &mut fp);
    assert_eq!(fp.silk_f.len(), 1, "hand-drawn silk must be kept");
}
