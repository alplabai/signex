//! Building an [`ErcContext`] tree over a sheet hierarchy without projecting
//! any sheet more than once per ERC run (#998).
//!
//! A sheet reachable through N paths (a diamond: `A -> B, C; B, C -> D`) used
//! to be projected N times, 2^k for a chain of k diamonds. [`HierarchyMemo`]
//! keeps each fully-expanded child context behind an [`Arc`], so every parent
//! that reaches the same sheet shares one projection.

use std::collections::HashMap;
use std::sync::Arc;

use signex_net::SheetKey;
use signex_types::schematic::SchematicSheet;

use crate::context::ErcContext;

type Resolutions = HashMap<SheetKey, HashMap<String, SheetKey>>;
type Sheets = HashMap<SheetKey, SchematicSheet>;

/// Per-run cache of child contexts, keyed by [`SheetKey`]. Create one per ERC
/// run and hand it to every sheet's context build; it is only valid for the
/// `resolutions` / `sheets` tables it was first used with.
#[derive(Debug, Default)]
pub struct HierarchyMemo {
    contexts: HashMap<SheetKey, Arc<ErcContext>>,
    projected: usize,
}

impl HierarchyMemo {
    pub fn new() -> Self {
        Self::default()
    }

    /// How many sheet contexts this memo has projected so far, the top sheets
    /// handed to [`HierarchyMemo::context_for`] included.
    pub fn projected_count(&self) -> usize {
        self.projected
    }

    /// Context for `snapshot` with its `children` filled recursively; see
    /// [`ErcContext::from_snapshot_with_children`] for the table shapes.
    ///
    /// A child that is already an ancestor (a reference cycle) is attached
    /// but not expanded, which bounds the recursion. A subtree that contained
    /// such a cut depends on the path that reached it, so it is never cached;
    /// every other subtree is path-independent and is shared.
    pub fn context_for<'a>(
        &mut self,
        snapshot: &SchematicSheet,
        own_key: Option<&'a SheetKey>,
        resolutions: &'a Resolutions,
        sheets: &Sheets,
    ) -> ErcContext {
        self.expand(snapshot, own_key, resolutions, sheets, &mut Vec::new())
            .0
    }

    /// Returns the context and whether a cycle cut happened inside it.
    fn expand<'a>(
        &mut self,
        snapshot: &SchematicSheet,
        key: Option<&'a SheetKey>,
        resolutions: &'a Resolutions,
        sheets: &Sheets,
        ancestors: &mut Vec<&'a SheetKey>,
    ) -> (ErcContext, bool) {
        let mut children = HashMap::new();
        let mut cut = false;
        if let Some(k) = key {
            ancestors.push(k);
        }
        for (filename, child_key) in key.and_then(|k| resolutions.get(k)).into_iter().flatten() {
            let Some(child) = sheets.get(child_key) else {
                continue;
            };
            let shared = if ancestors.contains(&child_key) {
                cut = true;
                self.projected += 1;
                let mut leaf = ErcContext::project(child, HashMap::new());
                leaf.key = Some(child_key.clone());
                Arc::new(leaf)
            } else if let Some(hit) = self.contexts.get(child_key) {
                Arc::clone(hit)
            } else {
                let (ctx, sub_cut) =
                    self.expand(child, Some(child_key), resolutions, sheets, ancestors);
                let shared = Arc::new(ctx);
                if sub_cut {
                    cut = true;
                } else {
                    self.contexts.insert(child_key.clone(), Arc::clone(&shared));
                }
                shared
            };
            children.insert(filename.clone(), shared);
        }
        if key.is_some() {
            ancestors.pop();
        }
        self.projected += 1;
        let mut ctx = ErcContext::project(snapshot, children);
        ctx.key = key.cloned();
        (ctx, cut)
    }
}
