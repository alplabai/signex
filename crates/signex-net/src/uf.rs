//! Iterative path-compression union-find, generic over the node key.
//!
//! Shared by `signex-net`'s netlist builder / cross-sheet stitcher and
//! `signex-erc`'s rules / context. Both used to keep their own recursive copy —
//! the recursive form was a real stack-overflow vector on degenerate wire
//! chains >10K segments (HI-17). This is the single canonical implementation.
//!
//! Most callers union bucketed point keys ([`Key`]); the cross-sheet stitcher
//! unions richer nodes (an occurrence id plus a per-sheet root), so `find` /
//! `union` are generic over any `K: Eq + Hash + Copy`.

use std::collections::HashMap;
use std::hash::Hash;

/// Bucketed point key — quantised mm coordinate so coincident-up-to-eps
/// endpoints hash to the same node. The 1 µm bucket (`* 1000.0`) matches
/// the schematic's underlying nm precision and is finer than any real
/// snap step (smallest is 25.4 µm = 1 mil).
pub type Key = (i64, i64);

/// Find the root of `x`, compressing the path along the way. Iterative
/// (no recursion) so a degenerate `N`-deep chain doesn't overflow the
/// thread stack.
///
/// Inserts `x` into `parent` if it isn't already present.
pub fn find<K: Eq + Hash + Copy>(parent: &mut HashMap<K, K>, x: K) -> K {
    parent.entry(x).or_insert(x);
    // Pass 1: walk to the root, recording every node along the chain.
    let mut visited: Vec<K> = Vec::new();
    let mut cur = x;
    loop {
        let p = *parent.get(&cur).unwrap_or(&cur);
        if p == cur {
            break;
        }
        visited.push(cur);
        cur = p;
    }
    let root = cur;
    // Pass 2: re-point every visited node directly at the root.
    for n in visited {
        parent.insert(n, root);
    }
    root
}

/// Union the two equivalence classes containing `a` and `b`.
pub fn union<K: Eq + Hash + Copy>(parent: &mut HashMap<K, K>, a: K, b: K) {
    let ra = find(parent, a);
    let rb = find(parent, b);
    if ra != rb {
        parent.insert(ra, rb);
    }
}
