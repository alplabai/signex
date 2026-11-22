//! The role of a whole closed profile (mockup row 190). A profile takes the
//! Pad role through a centre Point with a custom shape traced from the loop;
//! any other role is set on every edge and releases that pad. Each pick is
//! one undo step. Rules: `docs/FOOTPRINT_SKETCH.md`, "Roles".

use signex_bake::closed_profiles::{ClosedProfile, profile_of, profile_pad};
use signex_sketch::entity::EntityKind;
use signex_sketch::id::SketchEntityId;

use crate::app::FootprintDocument;
use crate::library::editor::footprint::sketch_conflict::mutate_solve_or_revert;
use crate::library::editor::footprint::sketch_dispatch::{
    apply_sketch_edit_with_warnings, report_solve_failure, set_entity_role,
};
use crate::library::editor::footprint::sketch_mode::SketchEdit;
use crate::library::messages::{FootprintEditorMsg, RoleTag};

/// The Role picker on one line or arc speaks for its profile: Pad makes the
/// loop a pad, and another role on a pad's loop releases the pad.
pub(in crate::library::editor::footprint::updates) fn route_role_pick(
    editor: &mut FootprintDocument,
    msg: FootprintEditorMsg,
) -> FootprintEditorMsg {
    let FootprintEditorMsg::SketchSetRole { id, role } = msg else {
        return msg;
    };
    let Some(sketch) = editor.primitive().sketch.as_ref() else {
        return msg;
    };
    let is_edge = sketch
        .entities
        .iter()
        .any(|e| e.id == id && matches!(e.kind, EntityKind::Line { .. } | EntityKind::Arc { .. }));
    if !is_edge {
        return msg;
    }
    if role == RoleTag::Pad {
        editor.state.selected_sketch = Some(id);
        return FootprintEditorMsg::SketchMakePadFromProfile;
    }
    let is_pad_loop =
        profile_of(sketch, id).is_some_and(|profile| profile_pad(sketch, &profile).is_some());
    if is_pad_loop {
        return FootprintEditorMsg::SketchProfileRolePicked { seed: id, role };
    }
    msg
}

pub(in crate::library::editor::footprint::updates) fn apply(
    editor: &mut FootprintDocument,
    seed: SketchEntityId,
    role: RoleTag,
) {
    editor.record_edit(None, |editor| {
        let before = editor.primitive().sketch.clone();
        set_profile_role(editor, seed, role);
        let changed = editor.primitive().sketch != before;
        if changed {
            editor.mark_dirty();
        }
        changed
    });
    editor.canvas_cache.clear();
}

fn set_profile_role(editor: &mut FootprintDocument, seed: SketchEntityId, role: RoleTag) {
    let Some(sketch) = editor.primitive().sketch.as_ref() else {
        return;
    };
    let Some(profile) = profile_of(sketch, seed) else {
        return;
    };
    let pad = profile_pad(sketch, &profile).map(|entity| entity.id);
    if role == RoleTag::Pad {
        // A circle has no loop to trace: see decisions on #2835.
        let can_mint = !profile.is_circle(sketch) && editor.state.last_solve.is_some();
        if pad.is_none() && can_mint {
            make_pad(editor, &profile);
        }
        return;
    }
    if let Some(centre) = pad {
        editor.with_parts(|state, fp| {
            apply_sketch_edit_with_warnings(state, fp, SketchEdit::DeleteEntity(centre));
        });
    }
    let solved = editor.with_parts(|state, fp| {
        mutate_solve_or_revert(state, fp, |fp| {
            for &edge in &profile.edges {
                set_entity_role(fp, edge, role);
            }
        })
    });
    if let Err(error) = &solved {
        editor.with_parts(|state, _| report_solve_failure(state, error));
    }
}

/// Clear the loop's edge roles and mint its pad.
fn make_pad(editor: &mut FootprintDocument, profile: &ClosedProfile) {
    editor.with_parts(|_, fp| {
        for &edge in &profile.edges {
            set_entity_role(fp, edge, RoleTag::Unassigned);
        }
    });
    editor.state.selected_sketch = Some(profile.seed);
    editor.state.selected_sketch_secondary = None;
    editor.state.selected_sketch_extra.clear();
    super::pad_bridge::make_pad_from_profile(editor);
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use signex_library::primitive::footprint::PadShape as LibPadShape;
    use signex_library::{Footprint, FootprintFile};
    use signex_sketch::Solver;
    use signex_sketch::attr::CourtyardAttr;
    use signex_sketch::entity::Entity;
    use signex_sketch::parameter::resolve;
    use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
    use signex_sketch::sketch::SketchData;

    use super::*;
    use crate::library::editor::footprint::updates::apply_footprint_primitive_edit;

    /// A footprint whose sketch is one closed square, solved once.
    fn square() -> (FootprintDocument, Vec<SketchEntityId>) {
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
        let corners = [(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)];
        let p: Vec<_> = corners
            .iter()
            .map(|&(x, y)| add(EntityKind::Point { x, y }))
            .collect();
        let edges: Vec<_> = (0..4)
            .map(|i| {
                add(EntityKind::Line {
                    start: p[i],
                    end: p[(i + 1) % 4],
                })
            })
            .collect();
        let params = resolve(&sketch.parameters).unwrap();
        let solve = Solver::default().solve(&sketch, &params).unwrap();
        let mut fp = Footprint::empty("t");
        fp.sketch = Some(sketch);
        let mut editor =
            FootprintDocument::new(PathBuf::from("t.snxfpt"), FootprintFile::from_footprint(fp));
        editor.state.last_solve = Some(solve);
        (editor, edges)
    }

    fn pick(editor: &mut FootprintDocument, seed: SketchEntityId, role: RoleTag) {
        apply_footprint_primitive_edit(
            editor,
            FootprintEditorMsg::SketchProfileRolePicked { seed, role },
        );
    }

    /// DECISIONS 2026-11-22, mockup row 190: a closed profile can take the
    /// Pad role, as a custom pad, in one undo step.
    #[test]
    fn a_closed_profile_takes_the_pad_role_as_a_custom_pad() {
        let (mut editor, edges) = square();
        let before = editor.primitive().sketch.clone();

        pick(&mut editor, edges[1], RoleTag::Pad);

        let pads = &editor.primitive().pads;
        assert_eq!(pads.len(), 1);
        assert!(matches!(pads[0].shape, LibPadShape::Custom(_)));
        assert_eq!(pads[0].position, [1.0, 1.0]);
        assert!(editor.undo(), "one undo step");
        assert_eq!(editor.primitive().sketch, before);
        assert!(!editor.history.can_undo(), "and only one");
    }

    #[test]
    fn picking_pad_again_on_another_edge_does_not_mint_a_second_pad() {
        let (mut editor, edges) = square();
        pick(&mut editor, edges[0], RoleTag::Pad);

        apply_footprint_primitive_edit(
            &mut editor,
            FootprintEditorMsg::SketchSetRole {
                id: edges[2],
                role: RoleTag::Pad,
            },
        );

        assert_eq!(editor.primitive().pads.len(), 1);
    }

    #[test]
    fn another_role_on_a_pad_profile_releases_the_pad_and_tags_every_edge() {
        let (mut editor, edges) = square();
        pick(&mut editor, edges[0], RoleTag::Pad);

        pick(&mut editor, edges[0], RoleTag::Courtyard);

        assert!(editor.primitive().pads.is_empty());
        let sketch = editor.primitive().sketch.as_ref().unwrap();
        assert!(sketch.entities.iter().all(|e| e.pad.is_none()));
        for id in &edges {
            let edge = sketch.entities.iter().find(|e| e.id == *id).unwrap();
            assert_eq!(edge.courtyard, Some(CourtyardAttr {}));
        }
    }

    /// DECISIONS 2026-11-22, mockup row 190: a Point stays a simple pad.
    #[test]
    fn a_point_still_takes_the_pad_role_as_a_simple_pad() {
        let (mut editor, _) = square();
        let point = editor.primitive().sketch.as_ref().unwrap().entities[0].id;

        apply_footprint_primitive_edit(
            &mut editor,
            FootprintEditorMsg::SketchSetRole {
                id: point,
                role: RoleTag::Pad,
            },
        );

        let pads = &editor.primitive().pads;
        assert_eq!(pads.len(), 1);
        assert!(matches!(pads[0].shape, LibPadShape::Rect));
    }
}
