//! Ticked rows to board commands, and Validate.

use std::collections::BTreeMap;

use signex_net::board::{BoardNetEdit, PadNetChange, ZoneNetName};
use signex_types::pcb::{NetDef, PcbBoard};

use super::change::{EcoChange, EcoChangeList};
use crate::pcb::{BatchRefusal, BoardCommand, BoardEngine, BoardItem, BoardItemId};

/// The commands the ticked rows of `list` make on `board`, in a fixed order:
/// one delete of every removed footprint, the footprint replacements, the
/// designator and value texts, the links, one add of every new footprint,
/// and one net edit. Empty when nothing is ticked. Execute them as one
/// labelled step at `list.revision`.
pub fn lower(list: &EcoChangeList, board: &PcbBoard) -> Vec<BoardCommand> {
    let mut commands = footprint_commands(list, board);
    if let Some(nets) = net_edit(list, board) {
        commands.push(BoardCommand::SetNets(nets));
    }
    commands
}

/// Why Validate refuses the list.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum EcoRefusal {
    #[error(
        "the board changed since the list was made (list at {list}, board at {board}); refresh it"
    )]
    Stale { list: u64, board: u64 },
    #[error(transparent)]
    Refused(#[from] BatchRefusal),
}

/// Validate: would the ticked rows apply to the engine's board as one step?
/// Runs them on a copy; nothing changes.
pub fn check(list: &EcoChangeList, engine: &BoardEngine) -> Result<(), EcoRefusal> {
    if engine.revision() != list.revision {
        return Err(EcoRefusal::Stale {
            list: list.revision,
            board: engine.revision(),
        });
    }
    engine.check_batch(&lower(list, engine.board()))?;
    Ok(())
}

/// Every command but the net edit, in the fixed order.
pub(super) fn footprint_commands(list: &EcoChangeList, board: &PcbBoard) -> Vec<BoardCommand> {
    let applied = || list.rows.iter().filter(|r| list.applies(r));
    let mut commands = Vec::new();

    let removed: Vec<BoardItemId> = applied()
        .filter_map(|r| match &r.change {
            EcoChange::RemoveFootprint { footprint, .. } => {
                Some(BoardItemId::Footprint(*footprint))
            }
            _ => None,
        })
        .collect();
    if !removed.is_empty() {
        commands.push(BoardCommand::DeleteItems { items: removed });
    }

    for row in applied() {
        if let EcoChange::ChangeFootprint {
            footprint, with, ..
        } = &row.change
        {
            commands.push(BoardCommand::ReplaceFootprint {
                footprint: *footprint,
                with: with.clone(),
            });
        }
    }

    // One text command per footprint, carrying whichever of the designator
    // and the value is ticked and the board's own for the other.
    let mut texts: BTreeMap<usize, (uuid::Uuid, Option<String>, Option<String>)> = BTreeMap::new();
    for row in applied() {
        let (id, reference, value) = match &row.change {
            EcoChange::SetDesignator { footprint, to, .. } => (*footprint, Some(to), None),
            EcoChange::SetValue { footprint, to, .. } => (*footprint, None, Some(to)),
            _ => continue,
        };
        let Some(index) = board.footprints.iter().position(|f| f.uuid == id) else {
            continue;
        };
        let entry = texts.entry(index).or_insert((id, None, None));
        if let Some(r) = reference {
            entry.1 = Some(r.clone());
        }
        if let Some(v) = value {
            entry.2 = Some(v.clone());
        }
    }
    for (index, (id, reference, value)) in texts {
        let fp = &board.footprints[index];
        commands.push(BoardCommand::SetFootprintText {
            footprint: id,
            reference: reference.unwrap_or_else(|| fp.reference.clone()),
            value: value.unwrap_or_else(|| fp.value.clone()),
        });
    }

    for row in applied() {
        if let EcoChange::Link {
            footprint, link, ..
        } = &row.change
        {
            commands.push(BoardCommand::SetFootprintLink {
                footprint: *footprint,
                link: Some(link.clone()),
            });
        }
    }

    let added: Vec<BoardItem> = applied()
        .filter_map(|r| match &r.change {
            EcoChange::AddFootprint { footprint } => {
                Some(BoardItem::Footprint((**footprint).clone()))
            }
            _ => None,
        })
        .collect();
    if !added.is_empty() {
        commands.push(BoardCommand::AddItems { items: added });
    }
    commands
}

/// The net edit of the ticked net rows, or `None` when none applies. The
/// table starts from the board's own and takes each ticked add, remove and
/// rename; a rename also renames the zones on that net.
fn net_edit(list: &EcoChangeList, board: &PcbBoard) -> Option<BoardNetEdit> {
    let mut table: BTreeMap<u32, String> = board
        .nets
        .iter()
        .map(|n| (n.number, n.name.clone()))
        .collect();
    let mut table_changed = false;
    let mut pads = Vec::new();
    let mut zones = Vec::new();
    for row in list.rows.iter().filter(|r| list.applies(r)) {
        match &row.change {
            EcoChange::AddNet { number, name } => {
                table.insert(*number, name.clone());
                table_changed = true;
            }
            EcoChange::RemoveNet { number, .. } => {
                table.remove(number);
                table_changed = true;
            }
            EcoChange::RenameNet { number, to, .. } => {
                table.insert(*number, to.clone());
                table_changed = true;
                zones.extend(board.zones.iter().filter(|z| z.net == *number).map(|z| {
                    ZoneNetName {
                        zone: z.uuid,
                        name: to.clone(),
                    }
                }));
            }
            EcoChange::PadNet {
                footprint,
                pad,
                to,
                locked,
                ..
            } => pads.push(PadNetChange {
                footprint: *footprint,
                pad: *pad,
                net: to.clone(),
                locked: *locked,
            }),
            _ => {}
        }
    }
    (table_changed || !pads.is_empty()).then(|| BoardNetEdit {
        nets: table
            .into_iter()
            .map(|(number, name)| NetDef { number, name })
            .collect(),
        pads,
        zones,
    })
}
