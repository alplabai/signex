//! The connected shapes of a board's mechanical edges: a closed loop, an
//! open chain or a branching figure, joined where edges share an end Point
//! on one layer. A pattern of any part repeats the whole shape. Rules:
//! `docs/PCB_SKETCH.md`, "Patterned board items".

use std::collections::HashMap;

use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;

/// The tagged edges, grouped into connected shapes.
pub(super) struct EdgeChains<'s> {
    chain_at: HashMap<SketchEntityId, usize>,
    chains: Vec<Vec<&'s Entity>>,
}

impl<'s> EdgeChains<'s> {
    pub(super) fn new(edges: impl Iterator<Item = &'s Entity>) -> Self {
        let edges: Vec<&'s Entity> = edges.collect();
        let mut at_point: HashMap<SketchEntityId, Vec<usize>> = HashMap::new();
        for (index, edge) in edges.iter().enumerate() {
            for end in ends(edge) {
                at_point.entry(end).or_default().push(index);
            }
        }
        let mut visited = vec![false; edges.len()];
        let mut chain_at = HashMap::new();
        let mut chains = Vec::new();
        for first in 0..edges.len() {
            if std::mem::replace(&mut visited[first], true) {
                continue;
            }
            let mut members = Vec::new();
            let mut stack = vec![first];
            while let Some(index) = stack.pop() {
                let edge = edges[index];
                chain_at.insert(edge.id, chains.len());
                members.push(edge);
                let touching = ends(edge)
                    .into_iter()
                    .filter_map(|end| at_point.get(&end))
                    .flatten();
                for &next in touching {
                    if !visited[next] && edges[next].mechanical == edge.mechanical {
                        visited[next] = true;
                        stack.push(next);
                    }
                }
            }
            chains.push(members);
        }
        Self { chain_at, chains }
    }

    pub(super) fn chains(&self) -> &[Vec<&'s Entity>] {
        &self.chains
    }

    /// The index in [`Self::chains`] of an edge given to [`Self::new`].
    pub(super) fn index_of(&self, edge: SketchEntityId) -> usize {
        self.chain_at[&edge]
    }
}

/// The Points an edge joins others at. An arc's centre joins nothing, and a
/// circle has no ends.
fn ends(edge: &Entity) -> Vec<SketchEntityId> {
    match edge.kind {
        EntityKind::Line { start, end } | EntityKind::Arc { start, end, .. } => vec![start, end],
        EntityKind::Circle { .. } | EntityKind::Point { .. } => Vec::new(),
    }
}
