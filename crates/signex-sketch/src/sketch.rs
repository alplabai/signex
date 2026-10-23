use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::array::Array;
use crate::array::ArrayId;
use crate::constraint::Constraint;
use crate::entity::Entity;
use crate::id::{ConstraintId, SketchEntityId};
use crate::parameter::ParameterTable;
use crate::plane::Plane;

/// Top-level container for a footprint sketch. Persisted as part of
/// a `Footprint`'s schema (v2+).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SketchData {
    #[serde(default)]
    pub planes: Vec<Plane>,
    #[serde(default)]
    pub entities: Vec<Entity>,
    #[serde(default)]
    pub constraints: Vec<Constraint>,
    #[serde(default)]
    pub arrays: Vec<Array>,
    #[serde(default)]
    pub parameters: ParameterTable,
}

/// What [`SketchData::remove_entity_cascade`] took out of the sketch.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Removed {
    pub entities: Vec<SketchEntityId>,
    pub constraints: Vec<ConstraintId>,
    pub arrays: Vec<ArrayId>,
}

impl SketchData {
    /// Remove `id` and everything that cannot exist without it: each
    /// line, arc or circle that names it as an endpoint or centre, and
    /// each constraint and array that names any removed entity. The sketch never
    /// holds a dangling reference afterwards: a sketch that already had
    /// some (see [`Self::dangling_refs`]) is healed by the same pass, so
    /// a delete is also the way out of a corrupt file.
    pub fn remove_entity_cascade(&mut self, id: SketchEntityId) -> Removed {
        let mut removed = Removed::default();
        let mut doomed: HashSet<SketchEntityId> = HashSet::from([id]);
        loop {
            let before = removed.entities.len() + removed.constraints.len() + removed.arrays.len();
            let known: HashSet<SketchEntityId> = self.entities.iter().map(|e| e.id).collect();
            let is_gone = |r: &SketchEntityId| doomed.contains(r) || !known.contains(r);
            self.entities.retain(|e| {
                let gone = doomed.contains(&e.id) || e.point_refs().iter().any(is_gone);
                if gone {
                    removed.entities.push(e.id);
                }
                !gone
            });
            self.constraints.retain(|c| {
                let gone = c.kind.entity_refs().iter().any(is_gone);
                if gone {
                    removed.constraints.push(c.id);
                }
                !gone
            });
            self.arrays.retain(|a| {
                let gone = a.kind.entity_refs().iter().any(is_gone);
                if gone {
                    removed.arrays.push(a.id);
                }
                !gone
            });
            if removed.entities.len() + removed.constraints.len() + removed.arrays.len() == before {
                return removed;
            }
            doomed.extend(removed.entities.iter().copied());
        }
    }

    /// Every entity id that an entity or constraint names but the sketch
    /// does not contain, sorted and de-duplicated. Empty for a
    /// well-formed sketch.
    pub fn dangling_refs(&self) -> Vec<SketchEntityId> {
        let known: HashSet<SketchEntityId> = self.entities.iter().map(|e| e.id).collect();
        let mut missing: Vec<SketchEntityId> = self
            .entities
            .iter()
            .flat_map(Entity::point_refs)
            .chain(self.constraints.iter().flat_map(|c| c.kind.entity_refs()))
            .chain(self.arrays.iter().flat_map(|a| a.kind.entity_refs()))
            .filter(|r| !known.contains(r))
            .collect();
        missing.sort_by_key(ToString::to_string);
        missing.dedup();
        missing
    }
}
