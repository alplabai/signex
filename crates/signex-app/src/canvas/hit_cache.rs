//! The hover hit index of the active sheet, kept while nothing it was built
//! from has changed.

use crate::schematic_runtime::hit_index::HitIndex;
use crate::schematic_runtime::{Designators, WireWidths};
use signex_types::designator::DesignatorStandard;
use signex_types::schematic::{SchematicDefaults, SelectedItem};

use super::CanvasSlot;

/// Everything an item box of the active sheet is computed from. `revision`
/// stands for the snapshot (an edit, undo or redo moves it) and
/// `class_revision` for the net-class styles the wire widths read.
#[derive(Clone, PartialEq)]
pub(super) struct SheetKey {
    revision: u64,
    class_revision: u64,
    defaults: SchematicDefaults,
    designator_standard: Option<DesignatorStandard>,
    /// Value fields are boxed on their drawn text (#1986).
    special_strings: Option<super::SpecialStrings>,
    designators: Designators,
    /// Rich notes are boxed as they draw (#1114): a landed compile re-boxes.
    rich_notes: Option<u64>,
}

impl SheetKey {
    pub(super) fn new(
        revision: u64,
        class_revision: u64,
        widths: &WireWidths<'_>,
        designators: &Designators,
    ) -> Self {
        Self {
            revision,
            class_revision,
            defaults: widths.defaults,
            designator_standard: widths.designator_standard,
            special_strings: widths.special_strings.cloned(),
            designators: designators.clone(),
            rich_notes: widths.rich_notes.map(|r| r.revision()),
        }
    }

    /// Whether this is the key of `new` with the same arguments, without
    /// building one.
    pub(super) fn matches(
        &self,
        revision: u64,
        class_revision: u64,
        widths: &WireWidths<'_>,
        designators: &Designators,
    ) -> bool {
        self.revision == revision
            && self.class_revision == class_revision
            && self.defaults == widths.defaults
            && self.designator_standard == widths.designator_standard
            && self.special_strings.as_ref() == widths.special_strings
            && self.designators == *designators
            && self.rich_notes == widths.rich_notes.map(|r| r.revision())
    }
}

pub(super) struct HitCache {
    key: SheetKey,
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
            c.key
                .matches(revision, class_revision, &widths, &self.designators)
        });
        if !fresh {
            *slot = Some(HitCache {
                key: SheetKey::new(revision, class_revision, &widths, &self.designators),
                index: HitIndex::build(snapshot, &self.designators, widths),
            });
        }
        slot.as_ref().and_then(|c| c.index.hit(x, y))
    }
}
