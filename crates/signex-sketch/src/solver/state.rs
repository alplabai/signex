use std::collections::{HashMap, HashSet};

use crate::constraint::ConstraintKind;
use crate::entity::{Entity, EntityKind};
use crate::id::SketchEntityId;
use crate::sketch::SketchData;

/// Maps a Point's `SketchEntityId` to its `(x, y)` offset in the
/// state vector, plus per-circle radius offsets and the set of
/// Fixed-constrained points whose coords are read directly from the
/// `Entity` instead of the state vector.
#[derive(Debug, Default, Clone)]
pub struct EntityIndex {
    pub points: HashMap<SketchEntityId, (usize, usize)>,
    pub radii: HashMap<SketchEntityId, usize>,
    pub fixed: HashSet<SketchEntityId>,
}

#[derive(Debug, Clone)]
pub struct PackedState {
    pub vector: Vec<f64>,
    pub index: EntityIndex,
}

/// Build a state vector from a sketch. Fixed-constrained Points are
/// excluded from the state vector; their coordinates are read directly
/// from the [`Entity`] at residual time so the solver cannot move them.
pub fn pack(sketch: &SketchData) -> PackedState {
    let fixed: HashSet<SketchEntityId> = sketch
        .constraints
        .iter()
        .filter_map(|c| match c.kind {
            ConstraintKind::Fixed { point } => Some(point),
            _ => None,
        })
        .collect();

    let mut vector = Vec::new();
    let mut index = EntityIndex {
        fixed: fixed.clone(),
        ..Default::default()
    };

    for entity in &sketch.entities {
        match entity.kind {
            EntityKind::Point { x, y } => {
                if fixed.contains(&entity.id) {
                    continue;
                }
                let xi = vector.len();
                vector.push(x);
                let yi = vector.len();
                vector.push(y);
                index.points.insert(entity.id, (xi, yi));
            }
            EntityKind::Circle { radius, .. } => {
                let ri = vector.len();
                vector.push(radius);
                index.radii.insert(entity.id, ri);
            }
            _ => {}
        }
    }

    PackedState { vector, index }
}

/// Look up a Point's current `(x, y)` — either from the state vector
/// (free variable) or directly from the [`Entity`] (Fixed-constrained).
pub fn point_xy(
    id: SketchEntityId,
    state: &[f64],
    index: &EntityIndex,
    sketch: &SketchData,
) -> Option<(f64, f64)> {
    if index.fixed.contains(&id) {
        let entity = sketch.entities.iter().find(|e| e.id == id)?;
        if let EntityKind::Point { x, y } = entity.kind {
            return Some((x, y));
        }
        return None;
    }
    let (xi, yi) = index.points.get(&id)?;
    Some((state[*xi], state[*yi]))
}

/// Look up the radius of a [`EntityKind::Circle`] from the state
/// vector. Returns `None` for non-circle entities.
pub fn circle_radius(id: SketchEntityId, state: &[f64], index: &EntityIndex) -> Option<f64> {
    let ri = index.radii.get(&id)?;
    Some(state[*ri])
}

/// Resolve the start/end Point IDs for a [`EntityKind::Line`].
pub fn line_endpoints(
    id: SketchEntityId,
    sketch: &SketchData,
) -> Option<(SketchEntityId, SketchEntityId)> {
    let line = sketch.entities.iter().find(|e| e.id == id)?;
    match line.kind {
        EntityKind::Line { start, end } => Some((start, end)),
        _ => None,
    }
}

/// Resolve the centre/start/end Point IDs and CCW flag for an
/// [`EntityKind::Arc`].
pub fn arc_refs(
    id: SketchEntityId,
    sketch: &SketchData,
) -> Option<(SketchEntityId, SketchEntityId, SketchEntityId, bool)> {
    let arc = sketch.entities.iter().find(|e| e.id == id)?;
    match arc.kind {
        EntityKind::Arc {
            center,
            start,
            end,
            sweep_ccw,
        } => Some((center, start, end, sweep_ccw)),
        _ => None,
    }
}

/// Resolve the centre Point ID for either an [`EntityKind::Arc`] or
/// [`EntityKind::Circle`].
pub fn center_of(id: SketchEntityId, sketch: &SketchData) -> Option<SketchEntityId> {
    let entity = sketch.entities.iter().find(|e| e.id == id)?;
    match entity.kind {
        EntityKind::Arc { center, .. } => Some(center),
        EntityKind::Circle { center, .. } => Some(center),
        _ => None,
    }
}

/// Lookup helper used by tests that need an [`Entity`] by ID.
pub fn find_entity<'a>(id: SketchEntityId, sketch: &'a SketchData) -> Option<&'a Entity> {
    sketch.entities.iter().find(|e| e.id == id)
}
