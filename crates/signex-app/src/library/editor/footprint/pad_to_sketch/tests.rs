use super::*;
use signex_library::primitive::footprint::Footprint;
use signex_library::primitive::footprint::PadShape as LibPadShape;
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use signex_sketch::sketch::SketchData;

fn editor_pad(number: &str, x: f64, y: f64) -> EditorPad {
    let mut p = EditorPad::new_default(number.into(), (x, y));
    p.size_mm = (1.0, 0.5);
    p
}

#[test]
fn empty_pads_mint_nothing() {
    let mut fp = Footprint::empty("test");
    let mut pads: Vec<EditorPad> = Vec::new();
    let n = auto_mint_for_literal_pads(&mut pads, &mut fp);
    assert_eq!(n, 0);
    assert!(fp.sketch.is_none() || fp.sketch.as_ref().unwrap().entities.is_empty());
}

#[test]
fn three_pads_mint_three_points_with_pad_attrs() {
    let mut fp = Footprint::empty("test");
    let mut pads = vec![
        editor_pad("1", 0.0, 0.0),
        editor_pad("2", 1.27, 0.0),
        editor_pad("3", 2.54, 0.0),
    ];
    let n = auto_mint_for_literal_pads(&mut pads, &mut fp);
    assert_eq!(n, 3);
    let sketch = fp.sketch.as_ref().unwrap();
    assert_eq!(sketch.planes.len(), 1);
    // v0.16 — per pad: 1 centre Point + 4 corner Points + 4 outline
    // Lines = 9 entities. 3 pads × 9 = 27.
    assert_eq!(sketch.entities.len(), 27);
    let attr_carriers: Vec<&Entity> = sketch.entities.iter().filter(|e| e.pad.is_some()).collect();
    assert_eq!(attr_carriers.len(), 3);
    for entity in attr_carriers {
        assert!(matches!(entity.kind, EntityKind::Point { .. }));
        assert!(!entity.construction);
        let attr = entity.pad.as_ref().unwrap();
        assert!(!attr.number.is_empty());
        assert_eq!(attr.size_x_expr, "1mm");
        assert_eq!(attr.size_y_expr, "0.5mm");
    }
    for pad in &pads {
        assert!(pad.sketch_entity_id.is_some());
        assert!(pad.corner_entity_ids.is_some());
    }
}

#[test]
fn skip_when_sketch_already_has_entities() {
    let mut fp = Footprint::empty("test");
    let mut sketch = SketchData::default();
    let plane = Plane {
        id: PlaneId::new(),
        kind: PlaneKind::BoardTop,
    };
    sketch.planes.push(plane.clone());
    sketch.entities.push(Entity::new(
        SketchEntityId::new(),
        plane.id,
        EntityKind::Point { x: 0.0, y: 0.0 },
    ));
    fp.sketch = Some(sketch);

    let mut pads = vec![editor_pad("1", 0.0, 0.0)];
    let n = auto_mint_for_literal_pads(&mut pads, &mut fp);
    assert_eq!(n, 0, "auto-mint must skip when sketch is already populated");
    assert_eq!(fp.sketch.as_ref().unwrap().entities.len(), 1);
    assert!(
        pads[0].sketch_entity_id.is_none(),
        "skip leaves the link unset"
    );
}

#[test]
fn skip_when_sketch_only_has_construction_entities() {
    let mut fp = Footprint::empty("test");
    let mut sketch = SketchData::default();
    let plane = Plane {
        id: PlaneId::new(),
        kind: PlaneKind::BoardTop,
    };
    let plane_id = plane.id;
    sketch.planes.push(plane);
    let mut construction = Entity::new(
        SketchEntityId::new(),
        plane_id,
        EntityKind::Point { x: 0.0, y: 0.0 },
    );
    construction.construction = true;
    sketch.entities.push(construction);
    fp.sketch = Some(sketch);

    let mut pads = vec![editor_pad("1", 0.0, 0.0)];
    let n = auto_mint_for_literal_pads(&mut pads, &mut fp);
    assert_eq!(n, 1);
    // Pre-existing construction (1) + minted centre (1) + 4 corner
    // Points + 4 outline Lines = 10.
    assert_eq!(fp.sketch.as_ref().unwrap().entities.len(), 10);
    assert!(pads[0].sketch_entity_id.is_some());
    assert!(pads[0].corner_entity_ids.is_some());
}

#[test]
fn mirror_add_pad_links_to_new_sketch_entity() {
    let mut fp = Footprint::empty("test");
    let mut pad = editor_pad("X", 5.0, 5.0);
    assert!(pad.sketch_entity_id.is_none());
    mirror_add_pad_to_sketch(&mut pad, &mut fp);
    let id = pad.sketch_entity_id.expect("mirror should mint id");
    let sketch = fp.sketch.as_ref().unwrap();
    let entity = sketch
        .entities
        .iter()
        .find(|e| e.id == id)
        .expect("entity exists");
    match entity.kind {
        EntityKind::Point { x, y } => assert_eq!((x, y), (5.0, 5.0)),
        _ => panic!("minted entity must be a Point"),
    }
    assert!(entity.pad.is_some(), "Point should carry PadAttr");
}

#[test]
fn mirror_add_pad_with_existing_link_is_noop() {
    let mut fp = Footprint::empty("test");
    let mut pad = editor_pad("X", 0.0, 0.0);
    pad.sketch_entity_id = Some(SketchEntityId::new());
    mirror_add_pad_to_sketch(&mut pad, &mut fp);
    assert!(fp.sketch.is_none() || fp.sketch.as_ref().unwrap().entities.is_empty());
}

#[test]
fn mirror_move_pad_updates_sketch_point() {
    let mut fp = Footprint::empty("test");
    let mut pad = editor_pad("X", 0.0, 0.0);
    mirror_add_pad_to_sketch(&mut pad, &mut fp);
    pad.position_mm = (3.5, 7.25);
    mirror_move_pad_in_sketch(&pad, &mut fp);
    let id = pad.sketch_entity_id.unwrap();
    let entity = fp
        .sketch
        .as_ref()
        .unwrap()
        .entities
        .iter()
        .find(|e| e.id == id)
        .unwrap();
    match entity.kind {
        EntityKind::Point { x, y } => assert_eq!((x, y), (3.5, 7.25)),
        _ => panic!("entity must still be a Point"),
    }
}

#[test]
fn mirror_delete_pad_drops_sketch_entity() {
    let mut fp = Footprint::empty("test");
    let mut pad = editor_pad("X", 0.0, 0.0);
    mirror_add_pad_to_sketch(&mut pad, &mut fp);
    // v0.16 — 1 centre + 4 corners + 4 lines = 9.
    assert_eq!(fp.sketch.as_ref().unwrap().entities.len(), 9);
    mirror_delete_pad_from_sketch(&pad, &mut fp);
    assert_eq!(fp.sketch.as_ref().unwrap().entities.len(), 0);
}

#[test]
fn format_f64_trims_trailing_zeros() {
    use super::attr::format_f64;
    assert_eq!(format_f64(1.0), "1");
    assert_eq!(format_f64(1.5), "1.5");
    assert_eq!(format_f64(0.25), "0.25");
    assert_eq!(format_f64(1.27), "1.27");
    assert_eq!(format_f64(0.0), "0");
}

#[test]
fn shape_change_preserves_corner_positions() {
    use crate::library::editor::footprint::state::FootprintEditorState;

    let mut fp = Footprint::empty("test");
    let mut pad = editor_pad("1", 0.0, 0.0);
    pad.shape = LibPadShape::Rect;
    mirror_add_pad_to_sketch(&mut pad, &mut fp);

    let corner_ids = pad.corner_entity_ids.expect("corners minted");
    let snapshot_corner_pos = |fp: &Footprint| -> Vec<(f64, f64)> {
        corner_ids
            .iter()
            .map(|id| {
                let entity = fp
                    .sketch
                    .as_ref()
                    .unwrap()
                    .entities
                    .iter()
                    .find(|e| e.id == *id)
                    .expect("corner Point present");
                match entity.kind {
                    EntityKind::Point { x, y } => (x, y),
                    _ => panic!("corner must be Point"),
                }
            })
            .collect()
    };

    let before = snapshot_corner_pos(&fp);

    pad.shape = LibPadShape::Oval;
    let mut s = FootprintEditorState::empty();
    s.pads = vec![pad.clone()];
    FootprintEditorState::sync_pads_to_primitive(&s, &mut fp);

    let after = snapshot_corner_pos(&fp);

    assert_eq!(
        before, after,
        "corner positions must remain stable across shape changes"
    );
}
