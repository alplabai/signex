//! What a press grabs to drag besides footprints, their strings and vias
//! (#2870): every other selectable board object, so each moves by
//! press-drag-release as it moves by Edit ▸ Move.

use std::collections::BTreeSet;
use std::sync::Arc;

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

/// What a press grabs, kept until the index is rebuilt (a board, view or
/// document change) or the selection filter changes, so a paint on every
/// pointer step does not rebuild it.
#[derive(Debug)]
pub(super) struct GrabCache {
    index_build: u32,
    filter: pcb_geom::PickFilter,
    shapes: Arc<[pcb_geom::Shape]>,
}

/// The whole selection a drag carries (#2870): its footprints (a selected
/// pad stands for its part), vias, and other items and rooms.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DraggedSelection {
    pub footprints: BTreeSet<Uuid>,
    pub vias: BTreeSet<Uuid>,
    pub rest: DraggedItems,
}

impl PcbSelectionState {
    /// What a press grabs to drag: every footprint, the designators and
    /// comments and the vias the filter lets a click pick, and every other
    /// object ([`Self::item_grab_shapes`]). Built once per index build and
    /// filter.
    pub(super) fn grab_shapes(&mut self, board: &PcbBoard) -> Arc<[pcb_geom::Shape]> {
        self.ensure_index(board);
        if let Some(cache) = &self.grab_cache
            && cache.index_build == self.index_builds
            && cache.filter == self.filter
        {
            return Arc::clone(&cache.shapes);
        }
        let mut grab = self.shapes_of_footprints(board, |_| true);
        // A designator or comment is grabbed to move on its own (#2368),
        // unless the filter turns texts off: a press there then starts a
        // box, as before texts were picked.
        if self.filter.is_on(pcb_geom::PickKind::Texts) {
            grab.extend(self.shapes_of(|item| matches!(item, ItemRef::FootprintText { .. })));
        }
        // A via is grabbed unless the filter turns vias off.
        if self.filter.is_on(pcb_geom::PickKind::Vias) {
            grab.extend(self.shapes_of(|item| matches!(item, ItemRef::Via(_))));
        }
        // Every other selectable object drags too (#2870).
        grab.extend(self.item_grab_shapes(board));
        let shapes: Arc<[pcb_geom::Shape]> = grab.into();
        self.grab_cache = Some(GrabCache {
            index_build: self.index_builds,
            filter: self.filter.clone(),
            shapes: Arc::clone(&shapes),
        });
        shapes
    }

    /// Everything selected, as a drag carries it.
    pub fn selection_for_drag(&self) -> DraggedSelection {
        let mut dragged = DraggedSelection {
            footprints: self.selection.footprints(),
            ..Default::default()
        };
        for item in self.selection.items() {
            match item {
                ItemRef::Via(id) => {
                    dragged.vias.insert(id);
                }
                other => dragged.rest.add(other),
            }
        }
        dragged
    }

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
