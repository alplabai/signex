//! #2518 — the host-aware role, bake-target and action lists.

use super::{SketchAction, SketchHost, SketchRole};
use crate::sketch::baked_kinds::BakedKind;
use crate::sketch::entity::EntityKind;
use crate::sketch::id::SketchEntityId;

#[test]
fn a_footprint_keeps_its_fifteen_roles_in_order() {
    use SketchRole as R;
    assert_eq!(
        SketchHost::Footprint.roles(),
        &[
            R::Unassigned,
            R::Pad,
            R::SilkTop,
            R::SilkBottom,
            R::Courtyard,
            R::Keepout,
            R::Cutout,
            R::MaskOpeningTop,
            R::MaskOpeningBottom,
            R::MaskExcludeTop,
            R::MaskExcludeBottom,
            R::PourTop,
            R::PourBottom,
            R::PasteApertureTop,
            R::PasteApertureBottom,
        ]
    );
}

#[test]
fn a_board_offers_the_targets_its_bake_produces() {
    use SketchRole as R;
    assert_eq!(
        SketchHost::Board.roles(),
        &[
            R::Unassigned,
            R::BoardOutline,
            R::Cutout,
            R::Slot,
            R::Keepout,
            R::BoardPour,
            R::VScore,
            R::MechanicalLayer,
            R::ComponentAnchor,
            R::MountingHole,
        ]
    );
    assert!(SketchHost::Board.offers_role(R::VScore));
    assert!(!SketchHost::Board.offers_role(R::Pad));
    assert!(!SketchHost::Footprint.offers_role(R::BoardOutline));
}

#[test]
fn the_bake_targets_of_each_host() {
    use BakedKind as K;
    assert_eq!(
        SketchHost::Board.bake_targets(),
        &[
            K::BoardOutline,
            K::Cutouts,
            K::Keepouts,
            K::BoardPours,
            K::VScores,
            K::Mechanical,
            K::ComponentAnchors,
            K::MountingHoles
        ]
    );
    assert_eq!(
        SketchHost::Footprint.bake_targets(),
        &[
            K::Pads,
            K::SilkF,
            K::SilkB,
            K::Courtyard,
            K::MaskOpenings,
            K::MaskExcludes,
            K::PasteApertures,
            K::Pours,
            K::Keepouts,
            K::Cutouts,
            K::VScores,
        ]
    );
}

/// No role in a picker does nothing: every offered role but Unassigned
/// bakes into a field its host writes.
#[test]
fn every_offered_role_bakes_into_a_target_of_its_host() {
    for host in [SketchHost::Footprint, SketchHost::Board] {
        for role in host.roles() {
            match role.target() {
                None => assert_eq!(*role, SketchRole::Unassigned),
                Some(kind) => assert!(
                    host.bake_targets().contains(&kind),
                    "{host:?} offers {role:?} but does not bake {kind:?}"
                ),
            }
        }
    }
}

#[test]
fn make_pad_from_profile_is_footprint_only() {
    assert!(SketchHost::Footprint.offers(SketchAction::MakePadFromProfile));
    assert!(!SketchHost::Board.offers(SketchAction::MakePadFromProfile));
}

#[test]
fn board_roles_need_their_shape() {
    let id = SketchEntityId::new;
    let point = EntityKind::Point { x: 0.0, y: 0.0 };
    let line = EntityKind::Line {
        start: id(),
        end: id(),
    };
    assert!(SketchRole::VScore.fits(&line));
    assert!(!SketchRole::VScore.fits(&point));
    assert!(SketchRole::BoardOutline.fits(&line));
    assert!(!SketchRole::BoardOutline.fits(&point));
    assert!(!SketchRole::MountingHole.fits(&line));
    assert!(SketchRole::Pad.fits(&line), "the make-a-pad shorthand");
}
