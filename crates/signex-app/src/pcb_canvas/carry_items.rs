//! The board items a drag carries besides footprints and vias (#2870):
//! tracks, free arcs and pads, drawn areas, strings, dimensions and
//! drawings, moved for the drag preview as the drop moves them
//! (`signex_engine::pcb::item_move`).

use std::collections::BTreeSet;

use signex_engine::pcb::dimension_refit::refit_on;
use signex_engine::pcb::item_move::moved_item;
use signex_engine::pcb::{BoardItem, BoardItemId};
use signex_types::pcb::{PcbBoard, Point};
use uuid::Uuid;

/// The ids of every item of `board` other than a footprint or via whose
/// uuid is in `carried`.
pub fn carried_item_ids(board: &PcbBoard, carried: &BTreeSet<Uuid>) -> Vec<BoardItemId> {
    let has = |id: &Uuid| carried.contains(id);
    let segments = board.segments.iter().map(|t| t.uuid).filter(has);
    let zones = board.zones.iter().map(|z| z.uuid).filter(has);
    let texts = board.texts.iter().map(|t| t.uuid).filter(has);
    let arcs = board.free.arcs.iter().map(|a| a.uuid).filter(has);
    let pads = board.free.pads.iter().map(|p| p.uuid).filter(has);
    let dims = board.free.dimensions.iter().map(|d| d.uuid).filter(has);
    let graphics = board.graphics.iter().map(|g| g.uuid).filter(has);
    (segments.map(BoardItemId::Segment))
        .chain(zones.map(BoardItemId::Zone))
        .chain(texts.map(BoardItemId::Text))
        .chain(arcs.map(BoardItemId::Arc))
        .chain(pads.map(BoardItemId::FreePad))
        .chain(dims.map(BoardItemId::Dimension))
        .chain(graphics.map(BoardItemId::Graphic))
        .collect()
}

/// The points a drag of the carried items snaps by: track and arc ends,
/// pad centres, area corners, string origins, dimension ends, drawing
/// points.
pub fn item_points(board: &PcbBoard, carried: &BTreeSet<Uuid>) -> Vec<Point> {
    let mut points = Vec::new();
    for id in carried_item_ids(board, carried) {
        match moved_item(board, id, Point::new(0.0, 0.0)) {
            Some(BoardItem::Segment(t)) => points.extend([t.start, t.end]),
            Some(BoardItem::Arc(a)) => points.extend([a.start, a.end]),
            Some(BoardItem::FreePad(p)) => points.push(p.position),
            Some(BoardItem::Zone(z)) => points.extend(z.outline),
            Some(BoardItem::Text(t)) => points.push(t.position),
            Some(BoardItem::Dimension(d)) => points.extend([d.start, d.end]),
            Some(BoardItem::Graphic(g)) => {
                points.extend([g.start, g.end, g.center].into_iter().flatten());
                points.extend(g.points);
            }
            _ => {}
        }
    }
    points
}

/// The carried items of `board` moved by `delta`.
fn moved(board: &PcbBoard, carried: &BTreeSet<Uuid>, delta: Point) -> Vec<BoardItem> {
    carried_item_ids(board, carried)
        .into_iter()
        .filter_map(|id| moved_item(board, id, delta))
        .collect()
}

/// Put `item` in `board`: in place of the item with its id when `replace`,
/// else added.
fn put(board: &mut PcbBoard, item: BoardItem, replace: bool) {
    fn set<T>(list: &mut Vec<T>, item: T, same: impl Fn(&T) -> bool, replace: bool) {
        match list.iter().position(same).filter(|_| replace) {
            Some(index) => list[index] = item,
            None => list.push(item),
        }
    }
    match item {
        BoardItem::Segment(t) => set(
            &mut board.segments,
            t.clone(),
            |o| o.uuid == t.uuid,
            replace,
        ),
        BoardItem::Zone(z) => set(&mut board.zones, z.clone(), |o| o.uuid == z.uuid, replace),
        BoardItem::Text(t) => set(&mut board.texts, t.clone(), |o| o.uuid == t.uuid, replace),
        BoardItem::Arc(a) => set(
            &mut board.free.arcs,
            a.clone(),
            |o| o.uuid == a.uuid,
            replace,
        ),
        BoardItem::FreePad(p) => {
            set(
                &mut board.free.pads,
                p.clone(),
                |o| o.uuid == p.uuid,
                replace,
            );
        }
        BoardItem::Dimension(d) => {
            let list = &mut board.free.dimensions;
            set(list, d.clone(), |o| o.uuid == d.uuid, replace);
        }
        BoardItem::Graphic(g) => set(
            &mut board.graphics,
            g.clone(),
            |o| o.uuid == g.uuid,
            replace,
        ),
        BoardItem::Footprint(_) | BoardItem::Via(_) => {}
    }
}

/// The radial dimensions of `board` a drag does not carry whose arc or
/// circle it does (#2507), each on that arc or circle as `into` holds it
/// moved: the drop puts them there (the engine's refit), so the drag
/// shows them following.
fn following_dimensions(
    board: &PcbBoard,
    carried: &BTreeSet<Uuid>,
    into: &PcbBoard,
) -> Vec<BoardItem> {
    board
        .free
        .dimensions
        .iter()
        .filter(|d| !carried.contains(&d.uuid))
        .filter(|d| d.measures.is_some_and(|m| carried.contains(&m)))
        .filter_map(|d| refit_on(d, into))
        .map(BoardItem::Dimension)
        .collect()
}

/// `into` (a copy of `board`) with the carried items moved by `delta`.
pub(super) fn move_carried_items(
    board: &PcbBoard,
    carried: &BTreeSet<Uuid>,
    delta: Point,
    into: &mut PcbBoard,
) {
    for item in moved(board, carried, delta) {
        put(into, item, true);
    }
    for item in following_dimensions(board, carried, into) {
        put(into, item, true);
    }
}

/// The carried items of `board` moved by `delta`, added to `into` (a board
/// holding only what a drag preview draws).
pub(super) fn add_carried_items(
    board: &PcbBoard,
    carried: &BTreeSet<Uuid>,
    delta: Point,
    into: &mut PcbBoard,
) {
    for item in moved(board, carried, delta) {
        put(into, item, false);
    }
    for item in following_dimensions(board, carried, into) {
        put(into, item, false);
    }
}
