//! What a press grabs to drag besides footprints, their strings and vias
//! (#2870): every other selectable board object, so each moves by
//! press-drag-release as it moves by Edit ▸ Move.

use std::collections::BTreeSet;

use pcb_geom::{ClickMode, ItemRef};
use signex_types::pcb::{PcbBoard, Point};
use signex_types::pcb_graphic::BoardGraphicKind;
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

/// The inside of an area or room outline, as a grab shape; `None` for an
/// outline of fewer than three corners or one the geometry cannot hold.
fn area_shape(outline: &[Point]) -> Option<pcb_geom::Shape> {
    if outline.len() < 3 {
        return None;
    }
    let ring = (outline.iter())
        .map(|p| pcb_geom::PointNm::from_mm(*p, "area outline"))
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    Some(pcb_geom::Shape::Polygon {
        ring: ring.into(),
        radius: 0,
    })
}

/// Whether a drag of `item` goes through [`DraggedItems`].
pub(super) fn is_dragged_item(item: ItemRef) -> bool {
    let mut probe = DraggedItems::default();
    probe.add(item);
    !probe.is_empty()
}

impl PcbSelectionState {
    /// The shapes of every object a press grabs to drag through
    /// [`DraggedItems`] that the selection filter lets a click pick: their
    /// strokes and edges, and the inside of each area and room (Caner,
    /// 2026-11-24). What the press drags is what a click there picks, so
    /// a track, pad, via or part on top of an area still wins.
    pub(super) fn item_grab_shapes(&self, board: &PcbBoard) -> Vec<pcb_geom::Shape> {
        let filter = &self.filter;
        let mut shapes = self.shapes_of(|item| is_dragged_item(item) && filter.allows(item));
        let zones = board.zones.iter().filter_map(|zone| {
            let kind = pcb_geom::PickKind::of_zone(zone).filter(|k| k.is_pickable())?;
            let item = ItemRef::Zone {
                zone: zone.uuid,
                kind,
            };
            filter.allows(item).then_some(zone.outline.as_slice())
        });
        let mut boxes: Vec<Vec<Point>> = (board.rooms.iter())
            .filter(|room| filter.allows(ItemRef::Room(room.uuid)))
            .map(|room| room.outline.corners())
            .collect();
        // A text frame is picked inside its box too, so it drags from there.
        boxes.extend(board.graphics.iter().filter_map(|graphic| {
            let frame = BoardGraphicKind::TextFrame;
            let item = ItemRef::Graphic {
                graphic: graphic.uuid,
                kind: pcb_geom::PickKind::of_graphic(frame),
            };
            let (a, b) = (graphic.start?, graphic.end?);
            (graphic.graphic_type == frame.token() && filter.allows(item))
                .then(|| vec![a, Point::new(b.x, a.y), b, Point::new(a.x, b.y)])
        }));
        let rings = zones.chain(boxes.iter().map(Vec::as_slice));
        shapes.extend(rings.filter_map(area_shape));
        shapes
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
