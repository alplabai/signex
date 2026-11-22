//! The closed profiles of a sketch (mockup row 190): each loop of lines and
//! arcs, and each circle, once. The Roles section lists them, and a closed
//! profile can carry the Pad role through a centre Point whose custom shape
//! is traced from the loop. Rules: `docs/FOOTPRINT_SKETCH.md`, "Roles".

use std::collections::HashSet;

use signex_sketch::attr::{CustomPadShape, PadShape};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::sketch::SketchData;

use crate::profile::trace_closed_profile_entities;

/// One closed profile: the edge it is reached by and all its edges.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosedProfile {
    pub seed: SketchEntityId,
    pub edges: Vec<SketchEntityId>,
}

impl ClosedProfile {
    /// Whether the profile is one circle, which has no loop to trace.
    pub fn is_circle(&self, sketch: &SketchData) -> bool {
        sketch
            .entities
            .iter()
            .any(|e| e.id == self.seed && matches!(e.kind, EntityKind::Circle { .. }))
    }
}

/// Every closed profile of `sketch`, in entity order. An open or branching
/// chain is none; construction and centerline entities never count.
pub fn closed_profiles(sketch: &SketchData) -> Vec<ClosedProfile> {
    let mut seen: HashSet<SketchEntityId> = HashSet::new();
    let mut out = Vec::new();
    for entity in sketch.entities.iter().filter(|e| !e.bake_skipped()) {
        if seen.contains(&entity.id) {
            continue;
        }
        if let Some(profile) = profile_from(sketch, entity) {
            seen.extend(profile.edges.iter().copied());
            out.push(profile);
        }
    }
    out
}

/// The closed profile `edge` is part of, if any.
pub fn profile_of(sketch: &SketchData, edge: SketchEntityId) -> Option<ClosedProfile> {
    let entity = sketch.entities.iter().find(|e| e.id == edge)?;
    if entity.bake_skipped() {
        return None;
    }
    profile_from(sketch, entity)
}

fn profile_from(sketch: &SketchData, entity: &Entity) -> Option<ClosedProfile> {
    match entity.kind {
        EntityKind::Circle { .. } => Some(ClosedProfile {
            seed: entity.id,
            edges: vec![entity.id],
        }),
        EntityKind::Line { .. } | EntityKind::Arc { .. } => {
            let traced = trace_closed_profile_entities(sketch, entity.id).ok()?;
            Some(ClosedProfile {
                seed: entity.id,
                edges: traced.edges.iter().map(|edge| edge.id).collect(),
            })
        }
        EntityKind::Point { .. } => None,
    }
}

/// The pad whose custom shape is traced from `profile`, if any.
pub fn profile_pad<'a>(sketch: &'a SketchData, profile: &ClosedProfile) -> Option<&'a Entity> {
    sketch.entities.iter().find(|entity| {
        let Some(pad) = entity.pad.as_ref() else {
            return false;
        };
        matches!(
            &pad.shape,
            PadShape::Custom(CustomPadShape::SketchProfile { source })
                if source.first().is_some_and(|seed| profile.edges.contains(seed))
        )
    })
}

#[cfg(test)]
mod tests {
    use signex_sketch::attr::PadAttr;
    use signex_sketch::plane::{Plane, PlaneId, PlaneKind};

    use super::*;

    /// A closed square, an open chain and a circle.
    fn sketch() -> (SketchData, Vec<SketchEntityId>, SketchEntityId) {
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
        let square: Vec<_> = (0..4)
            .map(|i| {
                add(EntityKind::Line {
                    start: p[i],
                    end: p[(i + 1) % 4],
                })
            })
            .collect();
        let a = add(EntityKind::Point { x: 5.0, y: 0.0 });
        let b = add(EntityKind::Point { x: 6.0, y: 0.0 });
        add(EntityKind::Line { start: a, end: b });
        let center = add(EntityKind::Point { x: 9.0, y: 0.0 });
        let circle = add(EntityKind::Circle {
            center,
            radius: 1.0,
        });
        (sketch, square, circle)
    }

    #[test]
    fn each_closed_profile_is_listed_once_and_an_open_chain_is_not() {
        let (sketch, square, circle) = sketch();
        let found = closed_profiles(&sketch);
        assert_eq!(found.len(), 2, "{found:?}");
        assert_eq!(found[0].edges.len(), 4);
        assert!(square.iter().all(|id| found[0].edges.contains(id)));
        assert_eq!(found[1].edges, vec![circle]);
        assert!(found[1].is_circle(&sketch));
    }

    #[test]
    fn a_pad_traced_from_any_edge_owns_the_profile() {
        let (mut sketch, square, _) = sketch();
        let plane = sketch.planes[0].id;
        let centre = SketchEntityId::new();
        let mut entity = Entity::new(centre, plane, EntityKind::Point { x: 1.0, y: 1.0 });
        entity.pad = Some(PadAttr {
            shape: PadShape::Custom(CustomPadShape::SketchProfile {
                source: vec![square[2]],
            }),
            ..PadAttr::default()
        });
        sketch.entities.push(entity);
        let profile = profile_of(&sketch, square[0]).unwrap();
        assert_eq!(profile_pad(&sketch, &profile).map(|e| e.id), Some(centre));
    }
}
