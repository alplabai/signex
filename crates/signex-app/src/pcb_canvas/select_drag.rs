//! What a press grabs to drag besides footprints, their strings and vias
//! (#2870): every other selectable board object, so each moves by
//! press-drag-release as it moves by Edit ▸ Move.

use std::collections::BTreeSet;

use pcb_geom::{ClickMode, ItemRef};
use signex_types::pcb::{PcbBoard, Point};
use uuid::Uuid;

use super::PcbSelectionState;

/// What a drag carries of the selection: the uuids of its tracks, free
/// arcs and pads, areas, strings, dimensions and drawings, and its rooms.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DraggedItems {
    pub items: BTreeSet<Uuid>,
    pub rooms: BTreeSet<Uuid>,
}

impl DraggedItems {
    pub fn is_empty(&self) -> bool {
        self.items.is_empty() && self.rooms.is_empty()
    }

    fn add(&mut self, item: ItemRef) {
        match item {
            ItemRef::Track(id)
            | ItemRef::Arc(id)
            | ItemRef::Text(id)
            | ItemRef::FreePad(id)
            | ItemRef::Dimension(id)
            | ItemRef::Zone { zone: id, .. }
            | ItemRef::Graphic { graphic: id, .. } => {
                self.items.insert(id);
            }
            ItemRef::Room(id) => {
                self.rooms.insert(id);
            }
            ItemRef::Pad { .. }
            | ItemRef::Via(_)
            | ItemRef::Footprint(_)
            | ItemRef::FootprintText { .. } => {}
        }
    }
}

/// Whether a drag of `item` goes through [`DraggedItems`].
pub(super) fn is_dragged_item(item: ItemRef) -> bool {
    let mut probe = DraggedItems::default();
    probe.add(item);
    !probe.is_empty()
}

impl PcbSelectionState {
    /// The shapes of every object a press grabs to drag through
    /// [`DraggedItems`] that the selection filter lets a click pick.
    pub(super) fn item_grab_shapes(&self) -> Vec<pcb_geom::Shape> {
        let filter = &self.filter;
        self.shapes_of(|item| is_dragged_item(item) && filter.allows(item))
    }

    /// What a press at `at` drags of these objects: the one a click there
    /// picks, with the rest of the selection when it is selected; otherwise
    /// it alone, which becomes the selection. `None` when the press is on
    /// none of them.
    pub fn grab_items_for_drag(
        &mut self,
        board: &PcbBoard,
        at: Point,
        tolerance_mm: f64,
    ) -> Option<DraggedItems> {
        let (item, _) = self.pick_at(board, at, tolerance_mm)?;
        if !is_dragged_item(item) {
            return None;
        }
        if !self.selection.contains(item) {
            self.selection.apply_click(Some(item), ClickMode::Replace);
        }
        let mut dragged = DraggedItems::default();
        self.selection.items().for_each(|i| dragged.add(i));
        Some(dragged)
    }
}
