//! The hover hit index of the active sheet, kept while nothing it was built
//! from has changed.

use crate::schematic_runtime::hit_index::HitIndex;
use crate::schematic_runtime::{Designators, WireWidths};
use signex_types::schematic::{SchematicDefaults, SelectedItem};

use super::CanvasSlot;

/// Everything the index was built from. `revision` stands for the snapshot
/// (an edit, undo or redo moves it) and `class_revision` for the net-class
/// styles the wire widths read.
struct HitKey {
    revision: u64,
    class_revision: u64,
    defaults: SchematicDefaults,
    designators: Designators,
}

pub(super) struct HitCache {
    key: HitKey,
    index: HitIndex,
}

impl CanvasSlot {
    /// The object under `(x, y)`, the answer of `hit_test_styled` for the
    /// active snapshot, from an index rebuilt only when the snapshot revision,
    /// the designators, the class styles or the defaults change. A snapshot
    /// with an unknown revision (`0`) is never cached.
    pub fn hit_item_at(
        &self,
        x: f64,
        y: f64,
        widths: WireWidths<'_>,
        class_revision: u64,
    ) -> Option<SelectedItem> {
        let snapshot = self.active_snapshot()?;
        let revision = self.render_cache.as_ref().map_or(0, |c| c.revision());
        if revision == 0 {
            return crate::schematic_runtime::hit_test::hit_test_styled(
                snapshot,
                &self.designators,
                x,
                y,
                widths,
            );
        }
        let mut slot = self.hit_cache.borrow_mut();
        let fresh = slot.as_ref().is_some_and(|c| {
            c.key.revision == revision
                && c.key.class_revision == class_revision
                && c.key.defaults == widths.defaults
                && c.key.designators == self.designators
        });
        if !fresh {
            *slot = Some(HitCache {
                key: HitKey {
                    revision,
                    class_revision,
                    defaults: widths.defaults,
                    designators: self.designators.clone(),
                },
                index: HitIndex::build(snapshot, &self.designators, widths),
            });
        }
        slot.as_ref().and_then(|c| c.index.hit(x, y))
    }
}
