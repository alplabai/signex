//! Decision 2 (Caner, 2026-11-20): the board targets Slot, Pour, Mechanical
//! layer and Component anchor, their shapes and what they bake into.

use super::{RoleShape, SketchHost, SketchRole};
use crate::sketch::baked_kinds::BakedKind;
use crate::sketch::board_attr::{
    BOARD_ITEM_KINDS, BoardSlotAttr, ComponentAnchorAttr, MechanicalAttr, SketchBoardItems,
};
use crate::sketch::data::SketchData;
use crate::sketch::entity::{Entity, EntityKind};
use crate::sketch::id::SketchEntityId;
use crate::sketch::plane::PlaneId;

fn shapes() -> [EntityKind; 4] {
    let id = SketchEntityId::new;
    [
        EntityKind::Point { x: 0.0, y: 0.0 },
        EntityKind::Line {
            start: id(),
            end: id(),
        },
        EntityKind::Arc {
            center: id(),
            start: id(),
            end: id(),
            sweep_ccw: true,
        },
        EntityKind::Circle {
            center: id(),
            radius: 1.0,
        },
    ]
}

#[test]
fn the_new_board_roles_bake_into_their_targets() {
    use BakedKind as K;
    use SketchRole as R;
    assert_eq!(R::Slot.target(), Some(K::Cutouts), "a slot is a cutout");
    assert_eq!(R::BoardPour.target(), Some(K::BoardPours));
    assert_eq!(R::MechanicalLayer.target(), Some(K::Mechanical));
    assert_eq!(R::ComponentAnchor.target(), Some(K::ComponentAnchors));
    for role in [
        R::Slot,
        R::BoardPour,
        R::MechanicalLayer,
        R::ComponentAnchor,
    ] {
        assert!(SketchHost::Board.offers_role(role), "{role:?}");
        assert!(!SketchHost::Footprint.offers_role(role), "{role:?}");
    }
    assert!(
        !SketchHost::Footprint
            .bake_targets()
            .contains(&K::BoardPours),
        "a footprint bakes its own pours"
    );
}

#[test]
fn an_anchor_takes_only_a_point_and_the_edge_roles_never_one() {
    let [point, line, arc, circle] = shapes();
    use SketchRole as R;
    assert!(R::ComponentAnchor.fits(&point));
    for edge in [&line, &arc, &circle] {
        assert!(!R::ComponentAnchor.fits(edge), "{edge:?}");
    }
    for role in [R::Slot, R::BoardPour, R::MechanicalLayer] {
        assert!(!role.fits(&point), "{role:?} on a point");
        for edge in [&line, &arc, &circle] {
            assert!(role.fits(edge), "{role:?} on {edge:?}");
        }
    }
}

/// The picker greys a misfit with the shape it needs (mockup row 179).
#[test]
fn a_misfit_names_the_shape_it_needs() {
    let [point, line, _, circle] = shapes();
    use SketchRole as R;
    assert_eq!(R::VScore.misfit(&circle), Some(RoleShape::Lines));
    assert_eq!(R::ComponentAnchor.misfit(&line), Some(RoleShape::Points));
    assert_eq!(R::MountingHole.misfit(&line), Some(RoleShape::Circles));
    assert_eq!(R::Slot.misfit(&point), Some(RoleShape::Edges));
    assert_eq!(R::ComponentAnchor.misfit(&point), None);
    assert_eq!(R::Unassigned.misfit(&point), None);
}

#[test]
fn every_board_role_bakes_into_a_board_target() {
    for role in SketchHost::Board.roles() {
        if let Some(kind) = role.target() {
            assert!(
                SketchHost::Board.bake_targets().contains(&kind),
                "{role:?} bakes {kind:?}"
            );
        }
    }
    for kind in BOARD_ITEM_KINDS {
        assert!(SketchHost::Board.bake_targets().contains(&kind), "{kind:?}");
    }
}

#[test]
fn a_pour_reads_as_the_board_pour_on_a_board() {
    use SketchRole as R;
    assert_eq!(R::PourTop.on_host(SketchHost::Board), R::BoardPour);
    assert_eq!(R::PourBottom.on_host(SketchHost::Board), R::BoardPour);
    assert_eq!(R::PourTop.on_host(SketchHost::Footprint), R::PourTop);
    assert_eq!(R::Cutout.on_host(SketchHost::Board), R::Cutout);
}

#[test]
fn the_new_kinds_have_their_own_tokens() {
    use BakedKind as K;
    assert_eq!(K::BoardPours.token(), "board_pours");
    assert_eq!(K::Mechanical.token(), "mechanical");
    assert_eq!(K::ComponentAnchors.token(), "component_anchors");
    assert_eq!(K::BoardPours.label(), "pours");
    assert_eq!(K::Mechanical.label(), "mechanical layer");
    assert_eq!(K::ComponentAnchors.label(), "component anchors");
}

#[test]
fn the_sketch_tags_the_new_kinds_by_their_attributes() {
    let plane = PlaneId::new();
    let mut data = SketchData::default();
    assert!(!data.carries_board_targets());
    assert!(!data.owns_board_items());

    let mut anchor = Entity::new(
        SketchEntityId::new(),
        plane,
        EntityKind::Point { x: 1.0, y: 2.0 },
    );
    anchor.component_anchor = Some(ComponentAnchorAttr::default());
    data.entities.push(anchor);
    assert!(data.tags_kind(BakedKind::ComponentAnchors));
    assert!(!data.tags_kind(BakedKind::Mechanical));
    assert!(data.carries_board_targets());
    assert!(data.owns_board_items());

    let mut drawing = Entity::new(
        SketchEntityId::new(),
        plane,
        EntityKind::Circle {
            center: SketchEntityId::new(),
            radius: 1.0,
        },
    );
    drawing.mechanical = Some(MechanicalAttr::default());
    data.entities.push(drawing);
    assert!(data.tags_kind(BakedKind::Mechanical));
    assert_eq!(MechanicalAttr::default().layer.index(), 1);
}

/// A slot alone changes the file (it is a cutout the bake checks) but
/// writes no board items: it is a cutout of the stored sketch.
#[test]
fn a_slot_is_a_board_target_but_no_board_item() {
    let mut slot = Entity::new(
        SketchEntityId::new(),
        PlaneId::new(),
        EntityKind::Line {
            start: SketchEntityId::new(),
            end: SketchEntityId::new(),
        },
    );
    slot.board_slot = Some(BoardSlotAttr::default());
    let mut data = SketchData::default();
    data.entities.push(slot);
    assert!(data.carries_board_targets());
    assert!(!data.owns_board_items());
}

#[test]
fn board_items_count_only_their_three_kinds() {
    let items = SketchBoardItems::default();
    assert_eq!(items.count(BakedKind::BoardPours), Some(0));
    assert_eq!(items.count(BakedKind::Mechanical), Some(0));
    assert_eq!(items.count(BakedKind::ComponentAnchors), Some(0));
    assert_eq!(items.count(BakedKind::Cutouts), None);
    assert_eq!(items.count(BakedKind::Pours), None);
}

#[test]
fn the_new_attributes_round_trip_through_serde_and_stay_out_when_unset() {
    let plane = PlaneId::new();
    let bare = Entity::new(
        SketchEntityId::new(),
        plane,
        EntityKind::Point { x: 0.0, y: 0.0 },
    );
    let json = serde_json::to_string(&bare).unwrap();
    for key in ["board_slot", "mechanical", "component_anchor"] {
        assert!(!json.contains(key), "{key} written while unset: {json}");
    }

    let mut entity = bare.clone();
    entity.board_slot = Some(BoardSlotAttr::default());
    entity.mechanical = Some(MechanicalAttr {
        layer: crate::pcb_layer::MechanicalLayer::new(13).unwrap(),
    });
    entity.component_anchor = Some(ComponentAnchorAttr {
        footprint: Some(uuid::Uuid::now_v7()),
    });
    let json = serde_json::to_string(&entity).unwrap();
    assert!(json.contains(r#""layer":"Mechanical13""#), "{json}");
    assert_eq!(serde_json::from_str::<Entity>(&json).unwrap(), entity);
}

#[test]
fn a_mechanical_attribute_on_another_layer_is_a_read_error() {
    let json = r#"{"layer":"TopCopper"}"#;
    let err = serde_json::from_str::<MechanicalAttr>(json).unwrap_err();
    assert!(err.to_string().contains("TopCopper"), "{err}");
}
