//! Which format a sketch needs on disk. The links added in `snxfpt/1.2`
//! (reference dimensions, pattern sources and mirrors, per-instance pad
//! overrides) are lost on a re-save by a build that does not know them, so
//! a sketch holding any of them asks for the newer token. A numbering sweep
//! and a pattern a board bake expands ask for a newer one still, and a
//! linear pattern's suppression list or a pour's corners the newest. Policy:
//! `docs/FORMAT_VERSIONS.md`.

use std::collections::{HashMap, HashSet};

use crate::sketch::array::{ArrayKind, NumberingScheme};
use crate::sketch::data::SketchData;
use crate::sketch::entity::{Entity, EntityKind};
use crate::sketch::id::SketchEntityId;

impl SketchData {
    /// Whether the sketch holds anything only `snxfpt/1.2` (and the
    /// matching board token) carries: a reference dimension, a pattern
    /// source link, a pattern mirror or a per-instance pad override.
    pub fn needs_format_1_2(&self) -> bool {
        !self.driven_dimensions.is_empty()
            || !self.pattern_sources.is_empty()
            || !self.pattern_mirrors.is_empty()
            || !self.pad_overrides.is_empty()
    }

    /// Whether an array numbers its cells in other than row-by-row order.
    /// An older build drops the `sweep` key on save and renumbers the pads.
    pub fn has_numbering_sweep(&self) -> bool {
        self.arrays.iter().any(|array| {
            matches!(
                &array.numbering,
                NumberingScheme::LinearIncrement { sweep, .. } if !sweep.is_row_by_row()
            )
        })
    }

    /// Whether a linear array has a depopulation (#2688): an older build
    /// does not know the key, would bake the suppressed pads and drop the
    /// list on save. One of the triggers of `SNXFPT_FORMAT_V1_5` and
    /// `SNXPCB_FORMAT_V1_13`.
    pub fn has_linear_suppression(&self) -> bool {
        self.arrays.iter().any(|array| {
            matches!(
                &array.kind,
                ArrayKind::Linear {
                    depopulation: Some(_),
                    ..
                }
            )
        })
    }

    /// Whether a pour has corners other than Sharp (#2598): an older build,
    /// which does not know the keys, would bake the outline sharp and drop
    /// them on save. A trigger of `SNXFPT_FORMAT_V1_5` and
    /// `SNXPCB_FORMAT_V1_13`.
    pub fn has_pour_corners(&self) -> bool {
        self.entities
            .iter()
            .filter_map(|entity| entity.pour.as_ref())
            .any(|pour| pour.has_corner_shape())
    }

    /// Whether a pattern may repeat a board bake target, so the board holds
    /// instances an older build's bake would lose. Every pattern, a mirror
    /// included, ends at an array, so the arrays' sources decide. Errs high:
    /// it follows all connected baking edges, a superset of what a bake
    /// repeats, because a missed instance is lost and a raised token is not.
    pub fn repeats_baked_target(&self) -> bool {
        if self.arrays.is_empty() {
            return false;
        }
        let reach = self.baked_target_reach();
        self.arrays.iter().any(|a| reach.contains(&a.kind.source()))
    }

    /// The board bake targets, their points, and every baking edge joined
    /// to them through shared points.
    fn baked_target_reach(&self) -> HashSet<SketchEntityId> {
        let edges: Vec<&Entity> = self
            .entities
            .iter()
            .filter(|e| !e.bake_skipped() && !matches!(e.kind, EntityKind::Point { .. }))
            .collect();
        let mut edges_at: HashMap<SketchEntityId, Vec<&Entity>> = HashMap::new();
        for &edge in &edges {
            for point in edge.point_refs() {
                edges_at.entry(point).or_default().push(edge);
            }
        }
        let mut reach = HashSet::new();
        let mut open = Vec::new();
        for target in self.entities.iter().filter(|e| is_board_bake_target(e)) {
            reach.insert(target.id);
            open.extend(target.point_refs());
        }
        let mut walked = HashSet::new();
        while let Some(point) = open.pop() {
            if !walked.insert(point) {
                continue;
            }
            reach.insert(point);
            for edge in edges_at.get(&point).into_iter().flatten() {
                reach.insert(edge.id);
                open.extend(edge.point_refs());
            }
        }
        reach
    }
}

/// Whether a board bake writes an item from `entity`. The outline and the
/// anchor count: a pattern of them refuses the bake here, where an older
/// build would bake them without it.
fn is_board_bake_target(entity: &Entity) -> bool {
    !entity.bake_skipped()
        && (entity.board_outline
            || entity.keepout.is_some()
            || entity.board_cutout.is_some()
            || entity.v_score.is_some()
            || entity.mounting_hole.is_some()
            || entity.pour.is_some()
            || entity.mechanical.is_some()
            || entity.component_anchor.is_some())
}
