//! The net highlight a canvas draws: what stays bright while everything else
//! dims (#1271). The Model owns the highlight; each slot holds the part for
//! the sheet it shows.

use std::collections::HashSet;
use std::sync::Arc;

use super::CanvasSlot;

impl CanvasSlot {
    /// Set what stays bright on this canvas' sheet; `None` for no highlight.
    /// Drops the drawn content only when it changed.
    pub fn set_net_highlight(&mut self, bright: Option<Arc<HashSet<uuid::Uuid>>>) {
        let same = match (&self.net_highlight, &bright) {
            (None, None) => true,
            (Some(a), Some(b)) => Arc::ptr_eq(a, b) || a == b,
            _ => false,
        };
        if same {
            return;
        }
        self.net_highlight = bright;
        self.clear_content_cache();
    }

    /// The elements the net highlight keeps bright, when one is active.
    #[must_use]
    pub fn net_highlight(&self) -> Option<&HashSet<uuid::Uuid>> {
        self.net_highlight.as_deref()
    }
}
