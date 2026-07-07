//! Iterative path-compression union-find over `(i64, i64)` keys.
//!
//! Shared by `signex-net`'s netlist builder and `signex-erc`'s rules /
//! context. Both used to keep their own recursive copy — the recursive
//! form was a real stack-overflow vector on degenerate wire chains >10K
//! segments (HI-17). This is the single canonical implementation.

use std::collections::HashMap;

/// Bucketed point key — quantised mm coordinate so coincident-up-to-eps
/// endpoints hash to the same node. The 1 µm bucket (`* 1000.0`) matches
/// the schematic's underlying nm precision and is finer than any real
/// snap step (smallest is 25.4 µm = 1 mil).
pub type Key = (i64, i64);
type Parent = HashMap<Key, Key>;

/// Find the root of `x`, compressing the path along the way. Iterative
/// (no recursion) so a degenerate `N`-deep chain doesn't overflow the
/// thread stack.
///
/// Inserts `x` into `parent` if it isn't already present.
pub fn find(parent: &mut Parent, x: Key) -> Key {
    parent.entry(x).or_insert(x);
    // Pass 1: walk to the root, recording every node along the chain.
    let mut visited: Vec<Key> = Vec::new();
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
pub fn union(parent: &mut Parent, a: Key, b: Key) {
    let ra = find(parent, a);
    let rb = find(parent, b);
    if ra != rb {
        parent.insert(ra, rb);
    }
}
