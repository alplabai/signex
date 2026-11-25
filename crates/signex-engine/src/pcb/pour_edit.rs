//! Slice Pour as board commands (#2216). Every polygon pour on the copper
//! layer that the slice line meets is rewritten with the shapes `pcb_geom`
//! cuts it into: the largest keeps the pour's identity and each other piece
//! is a new pour with the same settings. The commands run as one batch, so
//! the edit is one undo step. Pour Cutout edits no pour: it places a cutout
//! region (`draw::new_pour_cutout`, #2871) the fill leaves empty. Shelve
//! all and Unshelve all (#2523) set every pour's `shelved` in one replace;
//! Repour clears `unfilled` (#2506) in one replace.

use pcb_geom::{AreaShape, slice_area};
use signex_types::format::parse_pcb_layer;
use signex_types::pcb::{PcbBoard, Point, Zone};
use signex_types::pcb_layer::CopperLayer;
use uuid::Uuid;

use super::{BoardCommand, BoardItem, BoardItemId};

/// The command that shelves (`shelved`) or unshelves every polygon pour of
/// `board` as one step; `None` when every pour already is.
pub fn shelve_all_pours(board: &PcbBoard, shelved: bool) -> Option<BoardCommand> {
    let items: Vec<BoardItem> = board
        .zones
        .iter()
        .filter(|zone| zone.is_pour() && zone.shelved != shelved)
        .map(|zone| {
            BoardItem::Zone(Zone {
                shelved,
                ..zone.clone()
            })
        })
        .collect();
    (!items.is_empty()).then_some(BoardCommand::ReplaceItems { items })
}

/// The command Repour runs first (#2506): it clears `unfilled` on every
/// unfilled pour among `zones` (every pour of `board` for `None`) as one
/// step; `None` when none of them is unfilled, so a Repour of filled pours
/// stays a refresh with no undo step.
pub fn refill_pours(board: &PcbBoard, zones: Option<&[Uuid]>) -> Option<BoardCommand> {
    let items: Vec<BoardItem> = board
        .zones
        .iter()
        .filter(|zone| zone.is_unfilled_pour())
        .filter(|zone| zones.is_none_or(|ids| ids.contains(&zone.uuid)))
        .map(|zone| {
            BoardItem::Zone(Zone {
                unfilled: false,
                ..zone.clone()
            })
        })
        .collect();
    (!items.is_empty()).then_some(BoardCommand::ReplaceItems { items })
}

/// The commands that slice every pour on `layer` the segment from `a` to
/// `b` meets along the line through them; empty when none is cut in two.
/// `new_id` names each new piece.
pub fn slice_pours(
    board: &PcbBoard,
    layer: CopperLayer,
    a: Point,
    b: Point,
    new_id: impl FnMut() -> Uuid,
) -> Vec<BoardCommand> {
    rewrite(board, layer, |zone| slice_area(zone, a, b), new_id)
}

fn rewrite(
    board: &PcbBoard,
    layer: CopperLayer,
    shapes_of: impl Fn(&Zone) -> Option<Vec<AreaShape>>,
    mut new_id: impl FnMut() -> Uuid,
) -> Vec<BoardCommand> {
    let mut replaced = Vec::new();
    let mut added = Vec::new();
    let mut removed = Vec::new();
    let pours = board
        .zones
        .iter()
        .filter(|zone| zone.is_pour() && is_on(zone, layer));
    for zone in pours {
        let Some(shapes) = shapes_of(zone) else {
            continue;
        };
        let mut shapes = shapes.into_iter();
        let Some(first) = shapes.next() else {
            removed.push(BoardItemId::Zone(zone.uuid));
            continue;
        };
        replaced.push(BoardItem::Zone(reshaped(zone, zone.uuid, first)));
        for shape in shapes {
            added.push(BoardItem::Zone(reshaped(zone, new_id(), shape)));
        }
    }
    let mut commands = Vec::new();
    if !replaced.is_empty() {
        commands.push(BoardCommand::ReplaceItems { items: replaced });
    }
    if !added.is_empty() {
        commands.push(BoardCommand::AddItems { items: added });
    }
    if !removed.is_empty() {
        commands.push(BoardCommand::DeleteItems { items: removed });
    }
    commands
}

fn is_on(zone: &Zone, layer: CopperLayer) -> bool {
    parse_pcb_layer(&zone.layer).and_then(|l| l.copper()) == Some(layer)
}

/// `zone` with `shape` for its outline and holes, named `id`; every other
/// setting kept.
fn reshaped(zone: &Zone, id: Uuid, shape: AreaShape) -> Zone {
    Zone {
        uuid: id,
        outline: shape.outline,
        holes: shape.holes,
        ..zone.clone()
    }
}

#[cfg(test)]
#[path = "pour_edit_tests.rs"]
mod tests;
