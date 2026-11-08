//! Minimum spanning trees, for the ratsnest between unrouted copper.

use crate::union_find::UnionFind;
use crate::units::PointNm;

/// An undirected edge between nodes `a` and `b`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Edge {
    pub a: usize,
    pub b: usize,
    /// Any monotone cost; [`euclidean_mst`] uses the squared distance in nm².
    pub weight: i128,
}

/// The minimum spanning forest of `node_count` nodes over `edges`
/// (Kruskal). Ties break by `(a, b)`, so the result does not depend on the
/// order of `edges`. One tree per connected component.
///
/// # Panics
/// If an edge names a node `>= node_count`.
pub fn spanning_forest(node_count: usize, edges: &[Edge]) -> Vec<Edge> {
    let mut sorted: Vec<Edge> = edges
        .iter()
        .map(|e| Edge {
            a: e.a.min(e.b),
            b: e.a.max(e.b),
            weight: e.weight,
        })
        .collect();
    sorted.sort_by_key(|e| (e.weight, e.a, e.b));
    let mut sets = UnionFind::new(node_count);
    sorted
        .into_iter()
        .filter(|e| sets.union(e.a, e.b))
        .collect()
}

/// The Euclidean minimum spanning tree of `points` (Prim, O(n²)), as edges
/// between point indices weighted by squared distance. Ties go to the lower
/// index. Empty for fewer than two points.
pub fn euclidean_mst(points: &[PointNm]) -> Vec<Edge> {
    let n = points.len();
    if n < 2 {
        return Vec::new();
    }
    let mut in_tree = vec![false; n];
    // For every node outside the tree: its cheapest link into it.
    let mut best: Vec<(i128, usize)> = points
        .iter()
        .map(|p| (p.distance_sq(points[0]), 0))
        .collect();
    in_tree[0] = true;
    let mut edges = Vec::with_capacity(n - 1);
    for _ in 1..n {
        let Some(next) = (0..n)
            .filter(|&i| !in_tree[i])
            .min_by_key(|&i| (best[i].0, i))
        else {
            break;
        };
        in_tree[next] = true;
        let (weight, from) = best[next];
        edges.push(Edge {
            a: from.min(next),
            b: from.max(next),
            weight,
        });
        for i in (0..n).filter(|&i| !in_tree[i]) {
            let d = points[i].distance_sq(points[next]);
            if d < best[i].0 {
                best[i] = (d, next);
            }
        }
    }
    edges
}

#[cfg(test)]
mod tests {
    use super::*;

    fn total(edges: &[Edge]) -> i128 {
        edges.iter().map(|e| e.weight).sum()
    }

    #[test]
    fn kruskal_takes_the_cheapest_acyclic_edges() {
        let e = |a, b, weight| Edge { a, b, weight };
        let edges = [e(0, 1, 4), e(1, 2, 1), e(0, 2, 2), e(2, 3, 7), e(1, 3, 5)];
        let tree = spanning_forest(4, &edges);
        assert_eq!(tree, vec![e(1, 2, 1), e(0, 2, 2), e(1, 3, 5)]);
    }

    #[test]
    fn kruskal_is_independent_of_edge_order_and_direction() {
        let e = |a, b, weight| Edge { a, b, weight };
        let forward = [e(0, 1, 3), e(1, 2, 3), e(0, 2, 3)];
        let backward = [e(2, 0, 3), e(2, 1, 3), e(1, 0, 3)];
        assert_eq!(spanning_forest(3, &forward), spanning_forest(3, &backward));
    }

    #[test]
    fn kruskal_leaves_one_tree_per_component() {
        let e = |a, b, weight| Edge { a, b, weight };
        let tree = spanning_forest(5, &[e(0, 1, 1), e(3, 4, 1)]);
        assert_eq!(tree.len(), 2);
    }

    #[test]
    fn prim_matches_kruskal_on_the_complete_graph() {
        let points: Vec<PointNm> = [(0, 0), (10, 0), (10, 10), (3, 4), (-7, 2), (20, -5)]
            .into_iter()
            .map(|(x, y)| PointNm::new(x * 1_000, y * 1_000))
            .collect();
        let mut all = Vec::new();
        for a in 0..points.len() {
            for b in a + 1..points.len() {
                all.push(Edge {
                    a,
                    b,
                    weight: points[a].distance_sq(points[b]),
                });
            }
        }
        let prim = euclidean_mst(&points);
        assert_eq!(prim.len(), points.len() - 1);
        assert_eq!(total(&prim), total(&spanning_forest(points.len(), &all)));
    }

    #[test]
    fn prim_handles_tiny_and_coincident_inputs() {
        assert!(euclidean_mst(&[]).is_empty());
        assert!(euclidean_mst(&[PointNm::ORIGIN]).is_empty());
        let same = [PointNm::new(5, 5); 3];
        let tree = euclidean_mst(&same);
        assert_eq!(tree.len(), 2);
        assert_eq!(total(&tree), 0);
    }
}
