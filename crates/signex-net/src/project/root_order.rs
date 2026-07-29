//! Walk order for `ProjectGraph.roots` (#540).
//!
//! The multi-root traversal skips a root it has already reached as an earlier
//! root's child, so a declared page that another declared page references
//! contributes one occurrence rather than two. That skip only fires when the
//! *referencing* page happens to be walked first, and the caller's order —
//! sorted [`SheetKey`] — is decided by page names, which have nothing to do
//! with who references whom. Walked the other way round, the referenced page
//! is stitched twice: its terminals are duplicated, every refdes on it raises
//! a spurious `SharedReferenceAcrossInstances`, and every subsequent `NetId`
//! shifts.
//!
//! [`order_roots`] removes the order-dependence by putting a root that reaches
//! another root ahead of it, so the skip always has something to skip. The
//! caller's order survives as the tiebreak, so a flat project — no page
//! referencing any other, which is what `Add Existing Sheet` produces — is
//! walked in exactly the order it was handed.

use std::collections::HashSet;

use super::{ProjectGraph, SheetKey};

/// The order to walk `graph.roots` in: every root that reaches another root
/// precedes it, ties broken by the caller's order.
///
/// Duplicate keys are dropped (first occurrence wins) — a repeated root is
/// skipped by the traversal anyway, and deduplicating here keeps a doubled
/// page from being counted as its own referencer.
///
/// The project root is not pinned to the front. If some page references it,
/// the root is walked as that page's child instead of as a top-level page,
/// which is the same rule every other root gets. The alternative — pin it and
/// let the referencing page reach it a second time — is the duplication this
/// module exists to remove, and nothing downstream requires the root to be
/// occurrence 0.
pub(super) fn order_roots(graph: &ProjectGraph) -> Vec<SheetKey> {
    let mut keys: Vec<SheetKey> = Vec::with_capacity(graph.roots.len());
    for root in graph.roots {
        if !keys.contains(&root.key) {
            keys.push(root.key.clone());
        }
    }

    // `reaches[i]` = the indices of the other roots reachable from `keys[i]`.
    // Reachability, not the direct child edge: a page three levels above
    // another page still has to be walked first for the skip to fire.
    let reaches: Vec<Vec<usize>> = keys
        .iter()
        .map(|key| {
            let descendants = descendants(graph, key);
            (0..keys.len())
                .filter(|&j| descendants.contains(&keys[j]))
                .collect()
        })
        .collect();

    let mut order: Vec<SheetKey> = Vec::with_capacity(keys.len());
    let mut emitted = vec![false; keys.len()];
    for _ in 0..keys.len() {
        let next = (0..keys.len())
            .find(|&i| {
                !emitted[i]
                    && !(0..keys.len()).any(|j| !emitted[j] && j != i && reaches[j].contains(&i))
            })
            // Every root still to be walked is reachable from another one:
            // a reference cycle. Break it on the caller's order so the walk
            // stays deterministic; `visit`'s own path check still reports the
            // back edge as a `SheetCycle`.
            .unwrap_or_else(|| {
                (0..keys.len())
                    .find(|&i| !emitted[i])
                    .expect("the loop runs once per root, so one is always left")
            });
        emitted[next] = true;
        order.push(keys[next].clone());
    }
    order
}

/// Every key reachable from `start` through resolved child references,
/// excluding `start` itself — a key that only reaches itself through a cycle
/// is not its own referencer.
fn descendants(graph: &ProjectGraph, start: &SheetKey) -> HashSet<SheetKey> {
    let mut seen: HashSet<SheetKey> = HashSet::from([start.clone()]);
    let mut out: HashSet<SheetKey> = HashSet::new();
    let mut stack: Vec<SheetKey> = vec![start.clone()];

    while let Some(key) = stack.pop() {
        let (Some(sheet), Some(submap)) = (graph.sheets.get(&key), graph.resolved.get(&key)) else {
            continue;
        };
        for cs in &sheet.child_sheets {
            // A reference that does not resolve, or resolves to a sheet the
            // caller never loaded, is `MissingChild` at walk time and reaches
            // nothing here.
            let Some(child) = submap.get(cs.filename.as_str()) else {
                continue;
            };
            if graph.sheets.contains_key(child) && seen.insert(child.clone()) {
                out.insert(child.clone());
                stack.push(child.clone());
            }
        }
    }
    out
}
