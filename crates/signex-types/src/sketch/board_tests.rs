//! #2234 — the board bake attributes on a sketch entity and the board
//! baked kinds.

use crate::sketch::SketchData;
use crate::sketch::attr::MountingHoleAttr;
use crate::sketch::baked_kinds::BakedKind;
use crate::sketch::entity::{Entity, EntityKind};
use crate::sketch::id::SketchEntityId;
use crate::sketch::plane::PlaneId;

fn point() -> Entity {
    Entity::new(
        SketchEntityId::new(),
        PlaneId::new(),
        EntityKind::Point { x: 1.0, y: 2.0 },
    )
}

/// A footprint sketch's entities never carry the board attributes, and
/// writing one must not mention them, so footprint files keep their bytes.
#[test]
fn an_entity_without_board_attributes_writes_neither_key() {
    let text = toml::to_string(&point()).unwrap();
    assert!(!text.contains("board_outline"), "{text}");
    assert!(!text.contains("mounting_hole"), "{text}");
}

#[test]
fn the_board_attributes_round_trip() {
    let mut entity = point();
    entity.board_outline = true;
    entity.mounting_hole = Some(MountingHoleAttr {
        drill_expr: Some("3.2mm".into()),
        plated: true,
        net: Some("GND".into()),
    });
    let text = toml::to_string(&entity).unwrap();
    assert_eq!(toml::from_str::<Entity>(&text).unwrap(), entity);
}

#[test]
fn the_board_kinds_are_tagged_by_their_attributes() {
    let mut sketch = SketchData::default();
    assert!(!sketch.tags_kind(BakedKind::BoardOutline));
    assert!(!sketch.tags_kind(BakedKind::MountingHoles));

    let mut edge = point();
    edge.board_outline = true;
    let mut hole = point();
    hole.mounting_hole = Some(MountingHoleAttr::default());
    sketch.entities = vec![edge, hole];
    assert!(sketch.tags_kind(BakedKind::BoardOutline));
    assert!(sketch.tags_kind(BakedKind::MountingHoles));

    sketch.record_baked(BakedKind::BoardOutline, true);
    assert_eq!(sketch.baked_kinds, vec!["board_outline".to_string()]);
}

/// A construction or centerline edge is skipped by every bake, board
/// stages included.
#[test]
fn construction_geometry_is_still_bake_skipped_on_a_board() {
    let mut edge = point();
    edge.board_outline = true;
    edge.construction = true;
    assert!(edge.bake_skipped());
}
