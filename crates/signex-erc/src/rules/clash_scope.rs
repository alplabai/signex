//! Which sheet reports a designator clash that involves only descendants.
//!
//! Every ancestor's ERC run walks the same subtree, so a clash between two
//! descendant sheets used to be reported once per ancestor (#998). It belongs
//! to the lowest sheet whose own symbols or at least two different direct
//! children's subtrees are involved; any higher ancestor stays quiet.

use std::collections::HashSet;

use signex_net::SheetKey;

use crate::context::ErcContext;

/// Keys of `branch` and everything below it. Visited keys are skipped, so a
/// shared sheet or a cycle is walked once.
fn subtree_keys(branch: &ErcContext) -> HashSet<&SheetKey> {
    let mut keys = HashSet::new();
    let mut stack = vec![branch];
    while let Some(ctx) = stack.pop() {
        if let Some(k) = &ctx.key
            && !keys.insert(k)
        {
            continue;
        }
        stack.extend(ctx.children.values().map(|c| c.as_ref()));
    }
    keys
}

/// True when every sheet in `involved` sits inside one direct child of `ctx`,
/// so that child (or something below it) is the lowest common ancestor and
/// reports the clash itself. A sheet without a project key can't be placed,
/// so it never counts as confined.
pub(super) fn confined_to_one_child(ctx: &ErcContext, involved: &[&ErcContext]) -> bool {
    let Some(keys) = involved
        .iter()
        .map(|c| c.key.as_ref())
        .collect::<Option<Vec<_>>>()
    else {
        return false;
    };
    ctx.children.values().any(|branch| {
        let inside = subtree_keys(branch);
        keys.iter().all(|k| inside.contains(k))
    })
}
