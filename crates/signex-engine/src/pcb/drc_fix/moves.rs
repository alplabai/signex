//! Fixes that move an item clear (#2940): a track or via too near other
//! copper, a via hole too near another hole, and a board string or drawing
//! on the silkscreen too near a pad. Each moves the least distance that
//! clears the gap, away from the other item.

use std::collections::BTreeSet;

use pcb_geom::{
    ItemRef, PointNm, Shape, board_graphic_placement, graphic_strokes, mm_to_nm, text_shape,
    tracks_on_tracks,
};
use signex_drc::Violation;
use signex_types::corner::{CornerMode, CornerRule};
use signex_types::object_id::IdSource;
use signex_types::pcb::{PcbBoard, Point, Segment};
use uuid::Uuid;

use super::LeftReason;
use super::plan::{
    Plan, away, clearing_move, copper_shapes, found, locked_name, pad_geometry, pad_of, scaled,
};
use crate::pcb::item_move::moved_item;
use crate::pcb::placement::{PlacementRefusal, move_vias};
use crate::pcb::{BoardItem, BoardItemId};

/// A moved via drags the tracks on it straight, adding no corner.
const STRAIGHT: CornerRule = CornerRule {
    mode: CornerMode::FreeAngle,
    is_flipped: false,
};

/// Clearance: move the track or via of the two, a track first, the least
/// distance that clears the other. The tracks joined to a moved track's
/// ends, and those on a moved via, follow it.
pub(super) fn clearance(
    board: &PcbBoard,
    v: &Violation,
    new_ids: &mut dyn IdSource,
) -> Result<Vec<Plan>, LeftReason> {
    let &[a, b] = v.items.as_slice() else {
        return Err(LeftReason::CannotChange(
            "the finding does not name two items",
        ));
    };
    let mut order = [(a, b), (b, a)];
    order.sort_by_key(|(mover, _)| !matches!(mover, ItemRef::Track(_)));
    let (mut plans, mut locked) = (Vec::new(), Vec::new());
    for (mover, other) in order {
        if !matches!(mover, ItemRef::Track(_) | ItemRef::Via(_)) {
            continue;
        }
        if let Some(name) = locked_name(board, mover) {
            locked.push(name);
            continue;
        }
        let mine = copper_shapes(board, mover, v.layers);
        let theirs = copper_shapes(board, other, v.layers);
        let Some(dir) = away(&mine, &theirs) else {
            continue;
        };
        let Some(mm) = clearing_move(&mine, &theirs, dir, v.required_nm) else {
            continue;
        };
        let delta = scaled(dir, mm);
        let plan = match mover {
            ItemRef::Track(id) => moved_track(board, id, delta),
            ItemRef::Via(id) => moved_via(board, id, delta, new_ids).map(Some),
            _ => Ok(None),
        };
        match plan {
            Ok(Some(plan)) => plans.push(plan),
            Err(LeftReason::Locked(names)) => locked.extend(names),
            Ok(None) | Err(_) => {}
        }
    }
    found(
        plans,
        locked,
        "neither item is a track or via that can move",
    )
}

/// Hole to hole: move the via of the two the least distance that takes its
/// hole clear of the other; the tracks on it follow.
pub(super) fn hole_to_hole(
    board: &PcbBoard,
    v: &Violation,
    new_ids: &mut dyn IdSource,
) -> Result<Vec<Plan>, LeftReason> {
    let &[a, b] = v.items.as_slice() else {
        return Err(LeftReason::CannotChange(
            "the finding does not name two holes",
        ));
    };
    let (mut plans, mut locked) = (Vec::new(), Vec::new());
    for (mover, other) in [(a, b), (b, a)] {
        let ItemRef::Via(id) = mover else {
            continue;
        };
        if let Some(name) = locked_name(board, mover) {
            locked.push(name);
            continue;
        }
        let (Some(mine), Some(theirs)) = (hole_disc(board, mover), hole_disc(board, other)) else {
            continue;
        };
        // Two holes on one centre: any way out is as short.
        let dir = away(&mine, &theirs).unwrap_or(Point::new(1.0, 0.0));
        let Some(mm) = clearing_move(&mine, &theirs, dir, v.required_nm) else {
            continue;
        };
        match moved_via(board, id, scaled(dir, mm), new_ids) {
            Ok(plan) => plans.push(plan),
            Err(LeftReason::Locked(names)) => locked.extend(names),
            Err(_) => {}
        }
    }
    found(
        plans,
        locked,
        "neither hole is a via that can move; a pad's hole moves with its part",
    )
}

/// Silkscreen to pad: move the board string or drawing the least distance
/// that clears the pad. A part's own silkscreen is its footprint's.
pub(super) fn silk_to_pad(board: &PcbBoard, v: &Violation) -> Result<Vec<Plan>, LeftReason> {
    let silk = v.items.iter().copied().find(|i| {
        matches!(
            i,
            ItemRef::Text(_) | ItemRef::Graphic { .. } | ItemRef::Footprint(_)
        )
    });
    let pad = v
        .items
        .iter()
        .copied()
        .find(|i| matches!(i, ItemRef::Pad { .. } | ItemRef::FreePad(_)));
    let (Some(silk), Some(pad)) = (silk, pad) else {
        return Err(LeftReason::CannotChange(
            "the finding does not name a silkscreen item and a pad",
        ));
    };
    let id = match silk {
        ItemRef::Text(id) => BoardItemId::Text(id),
        ItemRef::Graphic { graphic, .. } => BoardItemId::Graphic(graphic),
        _ => {
            return Err(LeftReason::CannotChange(
                "a part's own silkscreen is changed in its footprint, not on the board",
            ));
        }
    };
    let (Some(mine), Some((pad_copper, _))) = (silk_shapes(board, id), pad_geometry(board, pad))
    else {
        return Err(LeftReason::CannotChange(
            "the silkscreen or the pad has no shape to measure",
        ));
    };
    let theirs = [pad_copper];
    let clearing = away(&mine, &theirs)
        .and_then(|dir| Some((dir, clearing_move(&mine, &theirs, dir, v.required_nm)?)));
    let Some((dir, mm)) = clearing else {
        return Err(LeftReason::CannotChange(
            "no move of the silkscreen up to 10 mm clears the pad",
        ));
    };
    let moved = moved_item(board, id, scaled(dir, mm)).ok_or(LeftReason::CannotChange(
        "the silkscreen item is not on the board",
    ))?;
    Ok(vec![Plan::replacing(vec![moved])])
}

/// The track `id` moved by `delta`, with the ends of the tracks joined to
/// its ends; `Err` names a locked track joined to it.
fn moved_track(board: &PcbBoard, id: Uuid, delta: Point) -> Result<Option<Plan>, LeftReason> {
    let Some(segment) = board.segments.iter().find(|s| s.uuid == id) else {
        return Ok(None);
    };
    let joined = locked_joined(board, segment);
    if !joined.is_empty() {
        return Err(LeftReason::Locked(joined));
    }
    let Some(moved) = moved_item(board, BoardItemId::Segment(id), delta) else {
        return Ok(None);
    };
    let mut items = vec![moved];
    for ends in tracks_on_tracks(board, &BTreeSet::from([id]), &BTreeSet::new()) {
        let neighbour = &board.segments[ends.index];
        items.push(BoardItem::Segment(ends.follow(neighbour, delta)));
    }
    Ok(Some(Plan::replacing(items)))
}

/// The locked tracks of `segment`'s net and layer with an end on one of its
/// ends: they would not follow it.
fn locked_joined(board: &PcbBoard, segment: &Segment) -> Vec<String> {
    let ends = [segment.start, segment.end];
    board
        .segments
        .iter()
        .filter(|s| s.locked && s.uuid != segment.uuid)
        .filter(|s| s.net == segment.net && s.layer == segment.layer)
        .filter(|s| ends.contains(&s.start) || ends.contains(&s.end))
        .map(|s| format!("track {}", s.uuid))
        .collect()
}

/// The via `id` moved by `delta`, the tracks on it following.
fn moved_via(
    board: &PcbBoard,
    id: Uuid,
    delta: Point,
    new_ids: &mut dyn IdSource,
) -> Result<Plan, LeftReason> {
    let commands =
        move_vias(board, &BTreeSet::from([id]), delta, STRAIGHT, new_ids).map_err(|refusal| {
            match refusal {
                PlacementRefusal::Locked(names) => LeftReason::Locked(names),
                other => LeftReason::Refused(other.to_string()),
            }
        })?;
    Ok(Plan::of_commands(id.to_string(), commands))
}

/// The hole of the via or pad `item` names, as a disc of its drill.
fn hole_disc(board: &PcbBoard, item: ItemRef) -> Option<Vec<Shape>> {
    let (at, drill) = match item {
        ItemRef::Via(id) => {
            let via = board.vias.iter().find(|v| v.uuid == id)?;
            (via.position, via.drill)
        }
        ItemRef::Pad { .. } | ItemRef::FreePad(_) => {
            let (_, at) = pad_geometry(board, item)?;
            (at, pad_of(board, item)?.drill.as_ref()?.diameter)
        }
        _ => return None,
    };
    Some(vec![Shape::Disc {
        center: PointNm::from_mm(at, "hole position").ok()?,
        radius: mm_to_nm(drill, "hole drill").ok()?.abs() / 2,
    }])
}

/// The silkscreen of the board string or drawing `id`, as the check
/// measures it.
fn silk_shapes(board: &PcbBoard, id: BoardItemId) -> Option<Vec<Shape>> {
    match id {
        BoardItemId::Text(uuid) => {
            let text = board.texts.iter().find(|t| t.uuid == uuid)?;
            Some(vec![text_shape(text).ok()?])
        }
        BoardItemId::Graphic(uuid) => {
            let graphic = board.graphics.iter().find(|g| g.uuid == uuid)?;
            let placed = board_graphic_placement(graphic).ok()??;
            Some(graphic_strokes(&placed))
        }
        _ => None,
    }
}
