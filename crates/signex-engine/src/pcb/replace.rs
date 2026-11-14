//! Replace tracks, vias, zones and texts in place, keeping their uuid and
//! list index: the edit behind a primitive's Properties fields (#2355).

use std::collections::{BTreeSet, HashSet};

use signex_types::format::parse_pcb_layer;
use signex_types::pcb::{Pad, PcbBoard, Segment, Via};
use uuid::Uuid;

use super::command::{Applied, BoardCommand, BoardPatch, BoardPatchKind};
use super::error::BoardCommandError;
use super::items::{self, BoardItem, BoardItemId};

/// Put each item over the board item with its id. Refused whole, before any
/// write, for a footprint (it has its own commands), an id named twice or not
/// on the board, a number that is not finite, or a changed value the board
/// cannot hold (see [`check_segment`] and [`check_via`]).
pub(super) fn replace(
    board: &mut PcbBoard,
    items: Vec<BoardItem>,
) -> Result<Applied, BoardCommandError> {
    let ids: Vec<BoardItemId> = items.iter().map(BoardItem::id).collect();
    let mut seen = HashSet::with_capacity(ids.len());
    let mut slots = Vec::with_capacity(ids.len());
    for item in &items {
        let id = item.id();
        if matches!(item, BoardItem::Footprint(_)) {
            return Err(BoardCommandError::NotReplaceable(id));
        }
        if !seen.insert(id) {
            return Err(BoardCommandError::DuplicateItem(id));
        }
        let index = items::position(board, id).ok_or(BoardCommandError::ItemNotFound(id))?;
        items::check_finite(item)?;
        match item {
            BoardItem::Segment(new) => check_segment(board, &board.segments[index], new)?,
            BoardItem::Via(new) => check_via(board, &board.vias[index], new)?,
            _ => {}
        }
        slots.push(index);
    }
    let previous = items
        .into_iter()
        .zip(slots)
        .map(|(item, index)| items::swap_at(board, index, item))
        .collect();
    Ok(Applied {
        inverse: BoardCommand::ReplaceItems { items: previous },
        patch: BoardPatch {
            kind: BoardPatchKind::ItemsReplaced,
            items: ids,
        },
    })
}

/// A track's width is a positive number; a changed layer is a copper layer
/// and a changed net is in the net table (or 0, no net). Unchanged values
/// pass, so a board read with an odd value can still be edited elsewhere.
fn check_segment(board: &PcbBoard, old: &Segment, new: &Segment) -> Result<(), BoardCommandError> {
    if new.width != old.width {
        check_positive(new.width, "track width")?;
    }
    let copper = parse_pcb_layer(&new.layer).and_then(|l| l.copper());
    if new.layer != old.layer && copper.is_none() {
        return Err(BoardCommandError::NotCopperLayer(new.layer.clone()));
    }
    check_net(board, old.net, new.net)
}

/// A via's diameter and hole are positive numbers and the hole fits inside
/// the via, once either changed; a changed net is in the net table.
fn check_via(board: &PcbBoard, old: &Via, new: &Via) -> Result<(), BoardCommandError> {
    if new.diameter != old.diameter || new.drill != old.drill {
        check_positive(new.diameter, "via diameter")?;
        check_positive(new.drill, "via hole size")?;
        if new.drill >= new.diameter {
            return Err(BoardCommandError::HoleTooLarge {
                hole: new.drill,
                diameter: new.diameter,
            });
        }
    }
    check_net(board, old.net, new.net)
}

fn check_positive(value: f64, field: &'static str) -> Result<(), BoardCommandError> {
    if !value.is_finite() {
        return Err(BoardCommandError::NonFinite { field });
    }
    if value <= 0.0 {
        return Err(BoardCommandError::NotPositive { field });
    }
    Ok(())
}

fn check_net(board: &PcbBoard, old: u32, new: u32) -> Result<(), BoardCommandError> {
    if new == old || new == 0 || board.nets.iter().any(|n| n.number == new) {
        return Ok(());
    }
    Err(BoardCommandError::UnknownNet(new))
}

/// A board pad and the footprint that holds it.
#[derive(Debug, Clone, PartialEq)]
pub struct FootprintPad {
    pub footprint: Uuid,
    pub pad: Pad,
}

/// Put each pad over the pad with its uuid on its footprint, at the same
/// index. Refused whole, before any write, for a footprint or pad that is
/// gone, a pad named twice, a position or turn that is not finite, or a
/// changed size, hole or net the board cannot hold (see [`check_pad`]).
/// Locks are the caller's, as for placement edits.
pub(super) fn replace_pads(
    board: &mut PcbBoard,
    pads: Vec<FootprintPad>,
) -> Result<Applied, BoardCommandError> {
    let mut seen = HashSet::with_capacity(pads.len());
    let mut slots = Vec::with_capacity(pads.len());
    for new in &pads {
        let (fp_index, fp) = board
            .footprints
            .iter()
            .enumerate()
            .find(|(_, fp)| fp.uuid == new.footprint)
            .ok_or(BoardCommandError::FootprintNotFound(new.footprint))?;
        let not_found = BoardCommandError::PadNotFound {
            footprint: new.footprint,
            pad: new.pad.uuid,
        };
        let pad_index = fp
            .pads
            .iter()
            .position(|p| p.uuid == new.pad.uuid)
            .ok_or(not_found)?;
        if !seen.insert(new.pad.uuid) {
            return Err(BoardCommandError::DuplicatePadChange {
                footprint: new.footprint,
                pad: new.pad.uuid,
            });
        }
        items::check_pads_finite(std::slice::from_ref(&new.pad))?;
        check_pad(board, &fp.pads[pad_index], &new.pad)?;
        slots.push((fp_index, pad_index));
    }
    let footprints: BTreeSet<Uuid> = pads.iter().map(|p| p.footprint).collect();
    let previous = pads
        .into_iter()
        .zip(slots)
        .map(|(new, (f, p))| FootprintPad {
            footprint: new.footprint,
            pad: std::mem::replace(&mut board.footprints[f].pads[p], new.pad),
        })
        .collect();
    Ok(Applied {
        inverse: BoardCommand::ReplacePads { pads: previous },
        patch: BoardPatch {
            kind: BoardPatchKind::PadsReplaced,
            items: footprints.into_iter().map(BoardItemId::Footprint).collect(),
        },
    })
}

/// A changed size is positive on both axes; a changed hole is positive and
/// fits inside the pad; a changed net is in the table under its own name.
fn check_pad(board: &PcbBoard, old: &Pad, new: &Pad) -> Result<(), BoardCommandError> {
    if new.size != old.size {
        check_positive(new.size.x, "pad x size")?;
        check_positive(new.size.y, "pad y size")?;
    }
    if (new.drill != old.drill || new.size != old.size)
        && let Some(drill) = new.drill.as_ref()
    {
        check_positive(drill.diameter, "pad hole size")?;
        let smaller = new.size.x.min(new.size.y);
        if drill.diameter > smaller {
            return Err(BoardCommandError::HoleTooLarge {
                hole: drill.diameter,
                diameter: smaller,
            });
        }
    }
    if new.net == old.net {
        return Ok(());
    }
    match new.net.as_ref() {
        None => Ok(()),
        Some(net)
            if board
                .nets
                .iter()
                .any(|n| n.number == net.number && n.name == net.name) =>
        {
            Ok(())
        }
        Some(net) => Err(BoardCommandError::UnknownNet(net.number)),
    }
}
