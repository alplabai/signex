//! The role picker's wording (#2518): one catalog entry per role. The
//! list itself is the host's (`SketchHost::roles`).

use signex_bake::board::slot_size;
use signex_bake::closed_profiles::{ClosedProfile, closed_profiles, profile_of, profile_pad};
use signex_i18n::{I18n, MessageId, ids, t};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::sketch::SketchData;
use signex_sketch::solver::FullSolveOutput;
use signex_types::sketch::{SketchHost, SketchRole};

use crate::library::editor::footprint::sketch_dispatch::current_role_of;

/// One row of the Roles section (mockup row 190): a closed profile and the
/// role it bakes as.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileRoleRow {
    pub seed: SketchEntityId,
    pub edges: usize,
    pub role: SketchRole,
    /// The designator, when the profile is a pad.
    pub pad_number: Option<String>,
    /// The roles this profile can take.
    pub options: Vec<SketchRole>,
}

/// One row per closed profile of `sketch`, in entity order.
pub fn profile_role_rows(sketch: &SketchData, host: SketchHost) -> Vec<ProfileRoleRow> {
    closed_profiles(sketch)
        .iter()
        .filter_map(|profile| profile_row(sketch, profile, host))
        .collect()
}

fn profile_row(
    sketch: &SketchData,
    profile: &ClosedProfile,
    host: SketchHost,
) -> Option<ProfileRoleRow> {
    let seed = sketch.entities.iter().find(|e| e.id == profile.seed)?;
    let pad = profile_pad(sketch, profile);
    let role = match pad {
        Some(_) => SketchRole::Pad,
        None => edge_role(sketch, profile).on_host(host),
    };
    let is_circle = profile.is_circle(sketch);
    let options = host
        .roles()
        .iter()
        .copied()
        .filter(|&r| r != SketchRole::VScore && r.fits(&seed.kind))
        .filter(|&r| !(is_circle && r == SketchRole::Pad))
        .collect();
    Some(ProfileRoleRow {
        seed: profile.seed,
        edges: profile.edges.len(),
        role,
        pad_number: pad.and_then(|e| e.pad.as_ref()).map(|a| a.number.clone()),
        options,
    })
}

/// The Bake as rows of a board sketch (mockup row 179): the rows of
/// [`profile_role_rows`] on the board, and Slot offered only on a profile
/// that is a slot at `solve`, since the bake refuses any other.
pub fn board_profile_rows(
    sketch: &SketchData,
    solve: Option<&FullSolveOutput>,
) -> Vec<ProfileRoleRow> {
    let mut rows = profile_role_rows(sketch, SketchHost::Board);
    for row in &mut rows {
        let is_slot = solve.is_some_and(|solve| slot_size(sketch, solve, row.seed).is_some());
        if !is_slot {
            row.options.retain(|&role| role != SketchRole::Slot);
        }
    }
    rows
}

/// The size (length, width) in mm of the slot that edge `id` belongs to at
/// `solve` (mockup row 179, the Slot header); `None` for a point, a circle
/// or an edge of a profile that is no slot.
pub fn edge_slot_size(
    sketch: &SketchData,
    solve: Option<&FullSolveOutput>,
    id: SketchEntityId,
) -> Option<(f64, f64)> {
    let entity = sketch.entities.iter().find(|e| e.id == id)?;
    if !matches!(
        entity.kind,
        EntityKind::Line { .. } | EntityKind::Arc { .. }
    ) {
        return None;
    }
    let slot = slot_size(sketch, solve?, id)?;
    Some((slot.length, slot.width))
}

/// The role of the first edge that has one.
fn edge_role(sketch: &SketchData, profile: &ClosedProfile) -> SketchRole {
    profile
        .edges
        .iter()
        .filter_map(|id| sketch.entities.iter().find(|e| e.id == *id))
        .map(current_role_of)
        .find(|role| *role != SketchRole::Unassigned)
        .unwrap_or(SketchRole::Unassigned)
}

/// The role the picker shows for `entity`: an edge of a pad's profile reads
/// Pad, though the pad itself lives on the profile's centre Point.
pub fn shown_role(sketch: &SketchData, entity: &Entity, host: SketchHost) -> SketchRole {
    let is_edge = matches!(
        entity.kind,
        EntityKind::Line { .. } | EntityKind::Arc { .. }
    );
    if is_edge && profile_of(sketch, entity.id).is_some_and(|p| profile_pad(sketch, &p).is_some()) {
        return SketchRole::Pad;
    }
    current_role_of(entity).on_host(host)
}

/// The catalog id naming `role` in the picker on `host`. A cutout reads
/// "Board Cutout" in a footprint, where the board is elsewhere, and
/// "Cutout" on the board itself.
pub fn role_label_id(role: SketchRole, host: SketchHost) -> MessageId {
    use SketchRole as R;
    match role {
        R::Unassigned => ids::EDITORS_SKETCH_ROLE_UNASSIGNED,
        R::Pad => ids::EDITORS_SKETCH_ROLE_PAD,
        R::SilkTop => ids::EDITORS_SKETCH_ROLE_SILK_TOP,
        R::SilkBottom => ids::EDITORS_SKETCH_ROLE_SILK_BOTTOM,
        R::Courtyard => ids::EDITORS_SKETCH_ROLE_COURTYARD,
        R::Keepout => ids::EDITORS_SKETCH_ROLE_KEEPOUT,
        R::Cutout if host == SketchHost::Board => ids::EDITORS_SKETCH_ROLE_CUTOUT,
        R::Cutout => ids::EDITORS_SKETCH_ROLE_BOARD_CUTOUT,
        R::MaskOpeningTop => ids::EDITORS_SKETCH_ROLE_MASK_OPENING_TOP,
        R::MaskOpeningBottom => ids::EDITORS_SKETCH_ROLE_MASK_OPENING_BOTTOM,
        R::MaskExcludeTop => ids::EDITORS_SKETCH_ROLE_MASK_EXCLUDE_TOP,
        R::MaskExcludeBottom => ids::EDITORS_SKETCH_ROLE_MASK_EXCLUDE_BOTTOM,
        R::PourTop => ids::EDITORS_SKETCH_ROLE_POUR_TOP,
        R::PourBottom => ids::EDITORS_SKETCH_ROLE_POUR_BOTTOM,
        R::PasteApertureTop => ids::EDITORS_SKETCH_ROLE_PASTE_TOP,
        R::PasteApertureBottom => ids::EDITORS_SKETCH_ROLE_PASTE_BOTTOM,
        R::BoardOutline => ids::EDITORS_SKETCH_ROLE_BOARD_OUTLINE,
        R::MountingHole => ids::EDITORS_SKETCH_ROLE_MOUNTING_HOLE,
        R::VScore => ids::EDITORS_SKETCH_ROLE_V_SCORE,
        R::Slot => ids::EDITORS_SKETCH_ROLE_SLOT,
        R::BoardPour => ids::EDITORS_SKETCH_ROLE_BOARD_POUR,
        R::MechanicalLayer => ids::EDITORS_SKETCH_ROLE_MECHANICAL_LAYER,
        R::ComponentAnchor => ids::EDITORS_SKETCH_ROLE_COMPONENT_ANCHOR,
    }
}

/// `role` as the picker on `host` shows it, in the UI language.
pub fn role_label(role: SketchRole, host: SketchHost, i18n: &I18n) -> String {
    t!(i18n, role_label_id(role, host)).into_string()
}

/// Every role the picker on `host` offers with its label, in order.
pub fn role_labels(host: SketchHost, i18n: &I18n) -> Vec<(SketchRole, String)> {
    host.roles()
        .iter()
        .map(|&role| (role, role_label(role, host, i18n)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn en_us() -> I18n {
        I18n::new(&signex_i18n::LanguagePref::Locale("en-US".into()))
    }

    /// The board picker reads in the board sketch design's words.
    #[test]
    fn the_board_picker_names_its_targets() {
        let labels: Vec<String> = role_labels(SketchHost::Board, &en_us())
            .into_iter()
            .map(|(_, label)| label.replace(['\u{2068}', '\u{2069}'], ""))
            .collect();
        assert_eq!(
            labels,
            [
                "Unassigned",
                "Board outline",
                "Cutout",
                "Slot",
                "Keepout",
                "Pour",
                "V-score",
                "Mechanical layer",
                "Component anchor",
                "Mounting hole"
            ]
        );
    }

    #[test]
    fn the_footprint_picker_keeps_its_wording() {
        let i18n = en_us();
        let labels = role_labels(SketchHost::Footprint, &i18n);
        assert_eq!(labels.len(), 15);
        assert_eq!(labels[6].1, "Board Cutout");
        assert_eq!(labels[14].1, "Paste · Bottom");
    }

    /// DECISIONS 2026-11-24, mockup row 179: a board lists each closed
    /// profile, and a role the profile cannot take is not offered.
    #[test]
    fn a_board_lists_each_closed_profile_with_only_the_roles_that_fit() {
        let f = fixtures::rect_and_slot();
        let rows = board_profile_rows(&f.sketch, Some(&f.solve));
        assert_eq!(rows.len(), 2, "{rows:#?}");
        let (rect, slot) = (&rows[0], &rows[1]);
        assert!(f.rect.contains(&rect.seed) && f.slot.contains(&slot.seed));
        assert_eq!((rect.edges, slot.edges), (4, 4));
        for misfit in [
            SketchRole::Slot,
            SketchRole::MountingHole,
            SketchRole::ComponentAnchor,
            SketchRole::VScore,
            SketchRole::Pad,
        ] {
            assert!(
                !rect.options.contains(&misfit),
                "{misfit:?} on the rectangle"
            );
        }
        assert!(slot.options.contains(&SketchRole::Slot));
        for fits in [
            SketchRole::BoardOutline,
            SketchRole::Cutout,
            SketchRole::Keepout,
            SketchRole::BoardPour,
        ] {
            assert!(rect.options.contains(&fits), "{fits:?} on the rectangle");
        }
    }

    #[test]
    fn without_a_solve_no_profile_is_offered_slot() {
        let f = fixtures::rect_and_slot();
        let rows = board_profile_rows(&f.sketch, None);
        assert!(
            rows.iter()
                .all(|row| !row.options.contains(&SketchRole::Slot))
        );
    }

    /// The footprint list stays as it was: every footprint role but V-score
    /// on a loop of lines.
    #[test]
    fn the_footprint_list_is_unchanged() {
        let f = fixtures::rect_and_slot();
        let rows = profile_role_rows(&f.sketch, SketchHost::Footprint);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].options, SketchHost::Footprint.roles());
    }

    #[test]
    fn every_edge_of_a_slot_reads_its_size_and_nothing_else_does() {
        let f = fixtures::rect_and_slot();
        for &edge in &f.slot {
            let (length, width) = edge_slot_size(&f.sketch, Some(&f.solve), edge)
                .unwrap_or_else(|| panic!("{edge:?} is a slot edge"));
            assert!((length - 12.0).abs() < 1e-6 && (width - 3.0).abs() < 1e-6);
        }
        for &edge in &f.rect {
            assert_eq!(edge_slot_size(&f.sketch, Some(&f.solve), edge), None);
        }
        assert_eq!(edge_slot_size(&f.sketch, None, f.slot[0]), None);
        let point = f.sketch.entities[0].id;
        assert_eq!(edge_slot_size(&f.sketch, Some(&f.solve), point), None);
    }
}

#[cfg(test)]
#[path = "roles_fixtures.rs"]
pub(crate) mod fixtures;
