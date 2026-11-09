//! The net rows of the change list: run net sync on the board as it would
//! be after the ticked footprint rows, and list what it changes.

use std::collections::{HashMap, HashSet};

use signex_net::board::{PadNetIssue, SymbolPadKey, assign_pad_nets};
use signex_types::pcb::PcbBoard;
use uuid::Uuid;

use super::EcoInput;
use super::change::{EcoChange, EcoChangeList, EcoGroup, EcoKey, EcoNote, EcoRow};
use super::lower::footprint_commands;

/// Recompute the net rows of `list` for its current footprint ticks, keeping
/// the ticks of net rows that are still there. Call after a footprint row is
/// ticked or unticked ([`EcoChangeList::set_on`] says when).
pub fn refresh_nets(list: &mut EcoChangeList, input: &EcoInput) {
    let previous = list.clone();
    list.rows.retain(|r| r.group < EcoGroup::Nets);
    list.notes.retain(|n| !matches!(n, EcoNote::Net(_)));

    let projected = project(input.board, list);
    let pin_maps = input.parts.iter().filter_map(|p| {
        let resolved = p.footprint.as_ref().ok()?;
        (!resolved.pin_map.is_empty()).then(|| (&p.part, resolved.pin_map.clone()))
    });
    let key = SymbolPadKey::for_board(&projected).with_pin_maps(pin_maps);
    let assignment = assign_pad_nets(input.netlist, &projected, &key);

    let added: HashMap<Uuid, EcoKey> = list
        .rows
        .iter()
        .filter_map(|r| match &r.change {
            EcoChange::AddFootprint { footprint } => Some((footprint.uuid, r.key.clone())),
            _ => None,
        })
        .collect();

    let before: HashMap<u32, &str> = projected
        .nets
        .iter()
        .map(|n| (n.number, n.name.as_str()))
        .collect();
    let after: HashMap<u32, &str> = assignment
        .edit
        .nets
        .iter()
        .map(|n| (n.number, n.name.as_str()))
        .collect();
    let mut table_rows = Vec::new();
    for net in &assignment.edit.nets {
        match before.get(&net.number) {
            None => table_rows.push(EcoRow::new(
                EcoKey::AddNet(net.number),
                EcoGroup::Nets,
                EcoChange::AddNet {
                    number: net.number,
                    name: net.name.clone(),
                },
                true,
            )),
            Some(old) if *old != net.name => table_rows.push(EcoRow::new(
                EcoKey::RenameNet(net.number),
                EcoGroup::Nets,
                EcoChange::RenameNet {
                    number: net.number,
                    from: (*old).to_string(),
                    to: net.name.clone(),
                },
                true,
            )),
            Some(_) => {}
        }
    }
    for net in &projected.nets {
        if !after.contains_key(&net.number) {
            table_rows.push(EcoRow::new(
                EcoKey::RemoveNet(net.number),
                EcoGroup::Nets,
                EcoChange::RemoveNet {
                    number: net.number,
                    name: net.name.clone(),
                },
                true,
            ));
        }
    }
    table_rows.sort_by_key(|r| r.key.clone());
    let new_numbers: HashSet<u32> = assignment
        .edit
        .nets
        .iter()
        .map(|n| n.number)
        .filter(|n| !before.contains_key(n))
        .collect();

    let mut pad_rows = Vec::new();
    for change in &assignment.edit.pads {
        let Some(fp) = projected
            .footprints
            .iter()
            .find(|f| f.uuid == change.footprint)
        else {
            continue;
        };
        let Some(pad) = fp.pads.iter().find(|p| p.uuid == change.pad) else {
            continue;
        };
        // A locked pad is the user's: it gets no row (net sync keeps it too).
        if pad.net_locked || (pad.net == change.net && !change.locked) {
            continue;
        }
        let mut needs = Vec::new();
        if let Some(add) = added.get(&fp.uuid) {
            needs.push(add.clone());
        }
        if let Some(net) = &change.net
            && new_numbers.contains(&net.number)
        {
            needs.push(EcoKey::AddNet(net.number));
        }
        pad_rows.push(
            EcoRow::new(
                EcoKey::PadNet {
                    footprint: fp.uuid,
                    pad: pad.uuid,
                },
                EcoGroup::PadNets,
                EcoChange::PadNet {
                    footprint: fp.uuid,
                    pad: pad.uuid,
                    reference: fp.reference.clone(),
                    pad_number: pad.number.clone(),
                    from: pad.net.clone(),
                    to: change.net.clone(),
                    locked: change.locked,
                },
                true,
            )
            .needing(needs),
        );
    }

    for row in table_rows.into_iter().chain(pad_rows) {
        let on = previous.row(&row.key).map_or(row.on, |old| old.on);
        list.rows.push(EcoRow { on, ..row });
    }
    list.notes.extend(
        assignment
            .issues
            .iter()
            .filter_map(|issue| issue_text(&projected, issue))
            .map(EcoNote::Net),
    );
}

/// The board with the ticked footprint rows applied. A command the board
/// refuses is skipped here; Validate reports it.
pub(super) fn project(board: &PcbBoard, list: &EcoChangeList) -> PcbBoard {
    let mut projected = board.clone();
    for command in footprint_commands(list, board) {
        let mut scratch = projected.clone();
        if command.apply(&mut scratch).is_ok() {
            projected = scratch;
        }
    }
    projected
}

fn reference_of(board: &PcbBoard, footprint: Uuid) -> String {
    board
        .footprints
        .iter()
        .find(|f| f.uuid == footprint)
        .map_or_else(|| footprint.to_string(), |f| f.reference.clone())
}

fn pad_name(board: &PcbBoard, footprint: Uuid, pad: Uuid) -> String {
    let fp = board.footprints.iter().find(|f| f.uuid == footprint);
    let number = fp
        .and_then(|f| f.pads.iter().find(|p| p.uuid == pad))
        .map_or_else(String::new, |p| p.number.clone());
    format!("{}.{}", reference_of(board, footprint), number)
}

/// The sync issues worth a line in the dialog; informational ones (a locked
/// pad, an unlinked footprint, which the rows already cover) are left out.
fn issue_text(board: &PcbBoard, issue: &PadNetIssue) -> Option<String> {
    Some(match issue {
        PadNetIssue::TerminalWithoutPad { terminal, .. } => {
            format!(
                "pin {}.{} has no pad on the board",
                terminal.reference, terminal.pin
            )
        }
        PadNetIssue::PadOnSeveralNets { footprint, pad, .. } => format!(
            "pad {} is on several nets and keeps its net",
            pad_name(board, *footprint, *pad)
        ),
        PadNetIssue::KeyOnSeveralFootprints { footprints, .. } => {
            let mut refs: Vec<String> =
                footprints.iter().map(|f| reference_of(board, *f)).collect();
            refs.sort();
            format!(
                "footprints {} share one part; none is assigned",
                refs.join(", ")
            )
        }
        PadNetIssue::PadNotOnSymbol { footprint, pad } => format!(
            "pad {} is not on its symbol; it keeps its net and locks",
            pad_name(board, *footprint, *pad)
        ),
        PadNetIssue::PadWithoutTerminal { footprint, pad } => format!(
            "pad {} has no number; it keeps its net",
            pad_name(board, *footprint, *pad)
        ),
        PadNetIssue::NetKeptForCopper { number } => {
            format!("net {number} has no pads but copper still uses it; it is kept")
        }
        PadNetIssue::FootprintNotInSchematic { .. }
        | PadNetIssue::FootprintNotLinked { .. }
        | PadNetIssue::PadNetLocked { .. } => return None,
    })
}
