//! The rows of a report: one per Update PCB row, its Back Annotate pair
//! joined to it, then the Back Annotate rows nothing pairs with, each with
//! its REL-01 default direction.

use signex_net::cross_probe::PartOccurrence;
use signex_types::pcb::PadNet;

use super::{
    CompareAction, CompareJump, CompareKind, CompareObject, CompareReport, CompareRow,
    CompareValue, Compared, Direction,
};
use crate::pcb::eco::back::{BackBlock, BackChange, BackChangeList, BackKey, BackRow};
use crate::pcb::eco::change::{EcoChange, EcoChangeList, EcoKey, EcoRow};

/// The report of `eco` and `back`, read from the same board and design,
/// with the REL-01 default directions.
pub fn compare(eco: &EcoChangeList, back: &BackChangeList) -> CompareReport {
    let mut rows: Vec<(CompareRow, bool)> = eco.rows.iter().map(eco_row).collect();
    for back_row in &back.rows {
        let paired = rows
            .iter_mut()
            .find(|(row, _)| pairs_with(row, back_row.key));
        match paired {
            Some((row, _)) => pair(row, &back_row.change, back_row.key, &back_row.blocked),
            None => rows.push((back_only_row(back_row), true)),
        }
    }
    rows.sort_by_key(|(row, _)| row.kind);
    let mut report = CompareReport {
        rows: Vec::with_capacity(rows.len()),
        compared: Compared::default(),
        eco: eco.clone(),
        back: back.clone(),
    };
    for (mut row, starts_on) in rows {
        row.direction = default_direction(&row, starts_on);
        report.rows.push(row);
    }
    report.sync_ticks();
    report
}

/// REL-01: destructive and board-originated rows, and rows Update PCB starts
/// unticked, are skipped; the schematic wins where it can, else the board.
fn default_direction(row: &CompareRow, pcb_starts_on: bool) -> Direction {
    let skipped_kind = matches!(
        row.kind,
        CompareKind::ExtraOnBoard | CompareKind::FootprintMismatch
    );
    if skipped_kind || (row.pcb.is_some() && !pcb_starts_on) {
        return Direction::Skip;
    }
    [Direction::UpdatePcb, Direction::UpdateSchematic]
        .into_iter()
        .find(|d| row.allows(*d))
        .unwrap_or(Direction::Skip)
}

/// The Back Annotate row keyed `key` changes the same field of the same
/// footprint as `row`'s Update PCB row.
fn pairs_with(row: &CompareRow, key: BackKey) -> bool {
    if row.back.is_some() {
        return false;
    }
    matches!(
        (&row.pcb, key),
        (Some(EcoKey::SetDesignator(a)), BackKey::Designator(b))
            | (Some(EcoKey::SetValue(a)), BackKey::Value(b))
            | (Some(EcoKey::ChangeFootprint(a)), BackKey::Footprint(b))
            if *a == b
    )
}

fn text(s: &str) -> Option<CompareValue> {
    Some(CompareValue::Text(s.to_string()))
}

fn net_value(net: &Option<PadNet>) -> Option<CompareValue> {
    Some(
        net.as_ref()
            .map_or(CompareValue::NoNet, |n| CompareValue::Text(n.name.clone())),
    )
}

fn net_name(net: &Option<PadNet>) -> Option<String> {
    net.as_ref().map(|n| n.name.clone())
}

fn designator(s: &str) -> CompareObject {
    CompareObject::Designator(s.trim().to_string())
}

/// A row from one Update PCB row, and whether that row starts ticked.
fn eco_row(eco: &EcoRow) -> (CompareRow, bool) {
    let mut row = CompareRow {
        kind: CompareKind::ValueChange,
        object: CompareObject::Designator(String::new()),
        schematic: None,
        board: None,
        pcb: Some(eco.key.clone()),
        back: None,
        pcb_blocked: eco.blocked.clone(),
        back_blocked: None,
        pcb_action: None,
        schematic_action: None,
        direction: Direction::Skip,
        removes_from_board: false,
        jump: CompareJump::default(),
    };
    let (kind, action) = match &eco.change {
        EcoChange::AddFootprint { footprint } => {
            row.object = designator(&footprint.reference);
            row.schematic = Some(CompareValue::Part {
                value: footprint.value.clone(),
                footprint: footprint.footprint_id.clone(),
            });
            if let EcoKey::AddFootprint { sheet, part } = &eco.key {
                row.jump.part = Some(PartOccurrence {
                    sheet: sheet.clone(),
                    symbol: *part,
                });
            }
            (CompareKind::MissingOnBoard, CompareAction::AddToBoard)
        }
        EcoChange::Link {
            footprint,
            reference,
            link,
        } => {
            row.object = designator(reference);
            (row.schematic, row.board) = (text(reference.trim()), text(reference.trim()));
            row.jump.footprint = Some(*footprint);
            row.jump.part = link.units.first().map(|unit| PartOccurrence {
                sheet: link.sheet.clone(),
                symbol: *unit,
            });
            (CompareKind::MissingOnBoard, CompareAction::LinkFootprint)
        }
        EcoChange::RemoveFootprint {
            footprint,
            reference,
        } => {
            row.object = designator(reference);
            row.board = text(reference.trim());
            row.jump.footprint = Some(*footprint);
            row.removes_from_board = true;
            (CompareKind::ExtraOnBoard, CompareAction::RemoveFromBoard)
        }
        EcoChange::ChangeFootprint {
            footprint,
            reference,
            from,
            with,
            ..
        } => {
            row.object = designator(reference);
            (row.schematic, row.board) = (text(&with.footprint_id), text(from));
            row.jump.footprint = Some(*footprint);
            (CompareKind::FootprintMismatch, CompareAction::SwapFootprint)
        }
        EcoChange::SetDesignator {
            footprint,
            from,
            to,
        } => {
            row.object = designator(to);
            (row.schematic, row.board) = (text(to), text(from));
            row.jump.footprint = Some(*footprint);
            (CompareKind::ValueChange, CompareAction::SetDesignator)
        }
        EcoChange::SetValue {
            footprint,
            reference,
            from,
            to,
        } => {
            row.object = designator(reference);
            (row.schematic, row.board) = (text(to), text(from));
            row.jump.footprint = Some(*footprint);
            (CompareKind::ValueChange, CompareAction::SetValue)
        }
        EcoChange::AddNet { name, .. } => {
            row.object = CompareObject::Net(name.clone());
            row.schematic = text(name);
            row.jump.schematic_net = Some(name.clone());
            (CompareKind::NetDifference, CompareAction::AddNet)
        }
        EcoChange::RemoveNet { name, .. } => {
            row.object = CompareObject::Net(name.clone());
            row.board = text(name);
            row.jump.board_net = Some(name.clone());
            row.removes_from_board = true;
            (CompareKind::NetDifference, CompareAction::RemoveNet)
        }
        EcoChange::RenameNet { from, to, .. } => {
            row.object = CompareObject::Net(to.clone());
            (row.schematic, row.board) = (text(to), text(from));
            row.jump.schematic_net = Some(to.clone());
            row.jump.board_net = Some(from.clone());
            (CompareKind::NetDifference, CompareAction::RenameNet)
        }
        EcoChange::PadNet {
            footprint,
            reference,
            pad_number,
            from,
            to,
            ..
        } => {
            row.object = CompareObject::Pin {
                reference: reference.trim().to_string(),
                pad: pad_number.clone(),
            };
            (row.schematic, row.board) = (net_value(to), net_value(from));
            row.jump.footprint = Some(*footprint);
            row.jump.schematic_net = net_name(to);
            row.jump.board_net = net_name(from);
            let action = match (from, to) {
                (None, _) => CompareAction::ConnectPin,
                (Some(_), None) => CompareAction::DisconnectPin,
                (Some(_), Some(_)) => CompareAction::MovePin,
            };
            (CompareKind::NetDifference, action)
        }
    };
    row.kind = kind;
    row.pcb_action = Some(action);
    (row, eco.default_on)
}

/// Add the Back Annotate row `key` to `row`, its pair.
fn pair(row: &mut CompareRow, change: &BackChange, key: BackKey, blocked: &Option<BackBlock>) {
    row.back = Some(key);
    row.back_blocked = blocked.clone();
    row.schematic_action = Some(back_action(change));
    if let Some(part) = back_part(change) {
        row.jump.part = Some(part);
    }
}

fn back_action(change: &BackChange) -> CompareAction {
    match change {
        BackChange::Designator { .. } => CompareAction::TakeBoardDesignator,
        BackChange::Value { .. } => CompareAction::TakeBoardValue,
        BackChange::Footprint { .. } => CompareAction::TakeBoardFootprint,
    }
}

fn back_part(change: &BackChange) -> Option<PartOccurrence> {
    match change {
        BackChange::Designator { sheet, units, .. } | BackChange::Value { sheet, units, .. } => {
            units.first().map(|unit| PartOccurrence {
                sheet: sheet.clone(),
                symbol: *unit,
            })
        }
        BackChange::Footprint { .. } => None,
    }
}

/// A row from a Back Annotate row no Update PCB row pairs with.
fn back_only_row(back: &BackRow) -> CompareRow {
    let (kind, object, schematic, board, footprint) = match &back.change {
        BackChange::Designator {
            footprint,
            from,
            to,
            ..
        } => (CompareKind::ValueChange, from, from, to, *footprint),
        BackChange::Value {
            footprint,
            reference,
            from,
            to,
            ..
        } => (CompareKind::ValueChange, reference, from, to, *footprint),
        BackChange::Footprint {
            footprint,
            reference,
            board,
            schematic,
        } => (
            CompareKind::FootprintMismatch,
            reference,
            schematic,
            board,
            *footprint,
        ),
    };
    let mut row = CompareRow {
        kind,
        object: designator(object),
        schematic: text(schematic),
        board: text(board),
        pcb: None,
        back: None,
        pcb_blocked: None,
        back_blocked: None,
        pcb_action: None,
        schematic_action: None,
        direction: Direction::Skip,
        removes_from_board: false,
        jump: CompareJump {
            footprint: Some(footprint),
            ..CompareJump::default()
        },
    };
    pair(&mut row, &back.change, back.key, &back.blocked);
    row
}
