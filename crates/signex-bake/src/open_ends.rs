//! The open ends of a sketch profile, on either host (mockup row 179d, mockup row 190): the
//! endpoints of profile edges that no other baking edge shares, where the
//! profile walk stops. Each end is paired with the nearest other open end,
//! so a gap names both its edges, both its points and its width. The board
//! outline checks and the bake preview's Close gap offer read the same
//! list. Rules: `docs/PCB_SKETCH.md`, "Close gap".

use std::collections::HashMap;

use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::sketch::SketchData;
use signex_sketch::solver::FullSolveOutput;
use signex_sketch::solver::state::point_xy;
use signex_types::pcb::Point;

/// One open end of a profile, or a pair of them across a gap.
#[derive(Debug, Clone, PartialEq)]
pub struct OpenEnd {
    /// The edges whose ends do not meet: one, or the two across a gap.
    pub edges: Vec<SketchEntityId>,
    /// The open end points: the two a gap lies between, or the one end
    /// nothing pairs.
    pub ends: Vec<SketchEntityId>,
    /// The distance to the paired end; `None` for an end nothing pairs.
    pub gap_mm: Option<f64>,
    /// The middle of the gap, or the end itself when nothing pairs it.
    pub at: Option<Point>,
}

impl OpenEnd {
    /// The two points across the gap, when this end is paired.
    pub fn pair(&self) -> Option<(SketchEntityId, SketchEntityId)> {
        match self.ends.as_slice() {
            [a, b] => Some((*a, *b)),
            _ => None,
        }
    }
}

/// Whether `entity` is an edge of a profile some bake traces as a closed
/// loop on either host: the board outline, a cutout, a keepout, a pour, a
/// courtyard or a mask or paste shape.
pub fn bakes_a_profile(entity: &Entity) -> bool {
    entity.board_outline
        || entity.board_cutout.is_some()
        || entity.keepout.is_some()
        || entity.pour.is_some()
        || entity.courtyard.is_some()
        || entity.mask_opening.is_some()
        || entity.mask_exclude.is_some()
        || entity.paste_aperture.is_some()
}

/// The open ends of the profile edges `is_profile` picks, at the positions
/// of `solve`, paired nearest first. An end counts as joined when any other
/// baking edge shares its point. Construction and centerline entities never
/// count.
pub fn open_ends(
    sketch: &SketchData,
    solve: &FullSolveOutput,
    is_profile: impl Fn(&Entity) -> bool,
) -> Vec<OpenEnd> {
    let baking = || sketch.entities.iter().filter(|e| !e.bake_skipped());
    let mut degree: HashMap<SketchEntityId, usize> = HashMap::new();
    for (a, b) in baking().filter_map(edge_ends) {
        *degree.entry(a).or_default() += 1;
        *degree.entry(b).or_default() += 1;
    }
    let position = |id| {
        point_xy(id, &solve.result.state, &solve.result.index, sketch).map(|(x, y)| Point { x, y })
    };
    let mut ends: Vec<(SketchEntityId, SketchEntityId, Point)> = Vec::new();
    for entity in baking().filter(|e| is_profile(e)) {
        let Some((a, b)) = edge_ends(entity) else {
            continue;
        };
        for end in [a, b] {
            if degree.get(&end) == Some(&1)
                && let Some(at) = position(end)
            {
                ends.push((entity.id, end, at));
            }
        }
    }
    let mut out = Vec::new();
    while !ends.is_empty() {
        let (edge, end, at) = ends.remove(0);
        let nearest = ends
            .iter()
            .enumerate()
            .map(|(i, (_, _, other))| (i, distance(at, *other)))
            .min_by(|a, b| a.1.total_cmp(&b.1));
        let Some((i, gap)) = nearest else {
            out.push(OpenEnd {
                edges: vec![edge],
                ends: vec![end],
                gap_mm: None,
                at: Some(at),
            });
            break;
        };
        let (other_edge, other_end, other) = ends.remove(i);
        let mut edges = vec![edge];
        if other_edge != edge {
            edges.push(other_edge);
        }
        out.push(OpenEnd {
            edges,
            ends: vec![end, other_end],
            gap_mm: Some(gap),
            at: Some(Point {
                x: (at.x + other.x) / 2.0,
                y: (at.y + other.y) / 2.0,
            }),
        });
    }
    out
}

/// The two endpoints the profile walk joins an edge by; `None` for points
/// and circles.
fn edge_ends(entity: &Entity) -> Option<(SketchEntityId, SketchEntityId)> {
    match entity.kind {
        EntityKind::Line { start, end } | EntityKind::Arc { start, end, .. } => Some((start, end)),
        EntityKind::Point { .. } | EntityKind::Circle { .. } => None,
    }
}

fn distance(a: Point, b: Point) -> f64 {
    (a.x - b.x).hypot(a.y - b.y)
}

#[cfg(test)]
mod tests {
    use signex_sketch::Solver;
    use signex_sketch::attr::CourtyardAttr;
    use signex_sketch::parameter::resolve;
    use signex_sketch::plane::{Plane, PlaneId, PlaneKind};

    use super::*;

    fn sketch_with(
        points: &[(f64, f64)],
        lines: &[(usize, usize)],
    ) -> (SketchData, Vec<SketchEntityId>, Vec<SketchEntityId>) {
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
        let p: Vec<SketchEntityId> = points
            .iter()
            .map(|&(x, y)| add(EntityKind::Point { x, y }))
            .collect();
        let l: Vec<SketchEntityId> = lines
            .iter()
            .map(|&(a, b)| {
                add(EntityKind::Line {
                    start: p[a],
                    end: p[b],
                })
            })
            .collect();
        (sketch, p, l)
    }

    fn solved(sketch: &SketchData) -> FullSolveOutput {
        let params = resolve(&sketch.parameters).unwrap();
        Solver::default().solve(sketch, &params).unwrap()
    }

    #[test]
    fn a_courtyard_with_a_gap_names_both_points_and_the_width() {
        // A square courtyard whose last edge stops 0.03 mm short.
        let (mut sketch, p, l) = sketch_with(
            &[(0.0, 0.0), (5.0, 0.0), (5.0, 5.0), (0.0, 5.0), (0.0, 0.03)],
            &[(0, 1), (1, 2), (2, 3), (3, 4)],
        );
        for entity in &mut sketch.entities {
            if l.contains(&entity.id) {
                entity.courtyard = Some(CourtyardAttr {});
            }
        }
        let found = open_ends(&sketch, &solved(&sketch), bakes_a_profile);
        assert_eq!(found.len(), 1, "{found:?}");
        let (a, b) = found[0].pair().expect("a paired gap");
        assert!(
            (a == p[0] && b == p[4]) || (a == p[4] && b == p[0]),
            "{found:?}"
        );
        assert!((found[0].gap_mm.unwrap() - 0.03).abs() < 1e-9);
    }

    #[test]
    fn a_closed_profile_and_untagged_edges_have_no_open_ends() {
        let (mut sketch, _, l) = sketch_with(
            &[(0.0, 0.0), (5.0, 0.0), (5.0, 5.0)],
            &[(0, 1), (1, 2), (2, 0)],
        );
        let solve = solved(&sketch);
        assert!(
            open_ends(&sketch, &solve, bakes_a_profile).is_empty(),
            "untagged"
        );
        for entity in &mut sketch.entities {
            if l.contains(&entity.id) {
                entity.board_outline = true;
            }
        }
        assert!(
            open_ends(&sketch, &solve, bakes_a_profile).is_empty(),
            "closed"
        );
    }

    #[test]
    fn a_single_open_edge_pairs_its_own_two_ends() {
        let (mut sketch, _, l) = sketch_with(&[(0.0, 0.0), (5.0, 0.0)], &[(0, 1)]);
        sketch
            .entities
            .iter_mut()
            .find(|e| e.id == l[0])
            .unwrap()
            .board_outline = true;
        let found = open_ends(&sketch, &solved(&sketch), |e| e.board_outline);
        // Both ends of the one line are open and pair with each other.
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].edges, vec![l[0]]);
        assert!((found[0].gap_mm.unwrap() - 5.0).abs() < 1e-9);
    }
}
