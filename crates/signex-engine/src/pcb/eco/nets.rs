//! The net rows of the change list: run net sync on the board as it would
//! be after the ticked footprint rows, and list what it changes.
//!
//! Net-table rows and pad rows depend on each other so that whatever the
//! user unticks, no pad is left naming a net the table lacks or names
//! differently: a pad on a new net needs its Add; a Remove needs every pad
//! row that empties the net; a Rename and the pad rows that only follow it
//! need each other, and a pad moving onto the renamed net needs the Rename.
//! A Remove also needs the Remove or Change footprint rows that took the
//! net's pads.

use std::collections::{HashMap, HashSet};

use signex_net::board::{
    BoardNetAssignment, PadLinkKey, PadNetIssue, SymbolPad, SymbolPadKey, assign_pad_nets,
};
use signex_types::net::Terminal;
use signex_types::pcb::{Footprint, Pad, PcbBoard};
use signex_types::schematic::InstancePath;
use uuid::Uuid;

use super::EcoInput;
use super::change::{EcoChange, EcoChangeList, EcoGroup, EcoKey, EcoNote, EcoRow};
use super::lower::footprint_commands;

/// Recompute the net rows of `list` for its current footprint ticks, keeping
/// the ticks of net rows that are still there with the same change. Call
/// after a footprint row is ticked or unticked ([`EcoChangeList::set_on`]
/// says when).
pub fn refresh_nets(list: &mut EcoChangeList, input: &EcoInput) {
    let previous = list.clone();
    list.rows.retain(|r| r.group < EcoGroup::Nets);
    list.notes.retain(|n| !matches!(n, EcoNote::Net(_)));

    let projected = project(input.board, list);
    let pin_maps = input.parts.iter().filter_map(|p| {
        let resolved = p.footprint.as_ref().ok()?;
        (!resolved.pin_map.is_empty()).then(|| (&p.part, resolved.pin_map.clone()))
    });
    let key = HeldBack::new(
        SymbolPadKey::for_board(&projected).with_pin_maps(pin_maps),
        &projected,
        input.parts,
    );
    let assignment = assign_pad_nets(input.netlist, &projected, &key);

    let table = TableChanges::of(&projected, &assignment);
    let mut table_rows = table.rows();
    let pad_rows = pad_rows(list, &projected, &assignment, &table, &mut table_rows);
    removal_needs(list, input.board, &projected, &table, &mut table_rows);

    for row in table_rows.into_iter().chain(pad_rows) {
        let on = match previous.row(&row.key) {
            Some(old) if old.change.same_choice(&row.change) => old.on,
            _ => row.on,
        };
        list.rows.push(EcoRow { on, ..row });
    }
    list.notes.extend(
        assignment
            .issues
            .iter()
            .filter(|issue| !key.covers(issue))
            .filter_map(|issue| issue_text(&projected, issue))
            .map(EcoNote::Net),
    );
}

/// The sync key with the footprints of parts whose pin map is unknown
/// ([`super::EcoPart::pin_map_known`]) held back: they count as unlinked,
/// so their pads keep their nets (following a rename) and are never set
/// pin n on pad n.
struct HeldBack {
    key: SymbolPadKey,
    /// The held parts' units, by sheet.
    units: HashSet<(InstancePath, Uuid)>,
    footprints: HashSet<Uuid>,
}

impl HeldBack {
    fn new(key: SymbolPadKey, board: &PcbBoard, parts: &[super::EcoPart]) -> Self {
        let part_units = |p: &super::EcoPart| -> Vec<(InstancePath, Uuid)> {
            p.part
                .units
                .iter()
                .map(|u| (p.part.sheet.clone(), *u))
                .collect()
        };
        let linked = |f: &Footprint, p: &super::EcoPart| {
            f.schematic_link.as_ref().is_some_and(|l| {
                l.sheet == p.part.sheet && l.units.iter().any(|u| p.part.units.contains(u))
            })
        };
        let mut units: HashSet<(InstancePath, Uuid)> = HashSet::new();
        let mut footprints: HashSet<Uuid> = HashSet::new();
        for p in parts {
            // A part with no placed footprint is held when its map is
            // unknown on its own; a placed one is judged against it.
            let placed: Vec<&Footprint> =
                board.footprints.iter().filter(|f| linked(f, p)).collect();
            if placed.is_empty() {
                if !p.pin_map_known(None) {
                    units.extend(part_units(p));
                }
                continue;
            }
            for f in placed {
                if !p.pin_map_known(Some(f)) {
                    units.extend(part_units(p));
                    footprints.insert(f.uuid);
                }
            }
        }
        Self {
            key,
            units,
            footprints,
        }
    }

    /// Whether `issue` is about a held part, which its note already covers.
    fn covers(&self, issue: &PadNetIssue) -> bool {
        match issue {
            PadNetIssue::TerminalWithoutPad { terminal, .. } => self
                .units
                .contains(&(terminal.occurrence.clone(), terminal.symbol)),
            _ => false,
        }
    }
}

impl PadLinkKey for HeldBack {
    type Key = SymbolPad;

    fn terminal_key(&self, terminal: &Terminal) -> Option<SymbolPad> {
        self.key.terminal_key(terminal)
    }

    fn pad_key(&self, footprint: &Footprint, pad: &Pad) -> Option<SymbolPad> {
        if self.footprints.contains(&footprint.uuid) {
            return None;
        }
        self.key.pad_key(footprint, pad)
    }

    fn footprint_unlinked(&self, footprint: &Footprint) -> bool {
        self.footprints.contains(&footprint.uuid) || self.key.footprint_unlinked(footprint)
    }
}

/// A net removed because a removed or replaced footprint took its pads
/// needs that footprint row: with it unticked, those pads stay on the net.
fn removal_needs(
    list: &EcoChangeList,
    board: &PcbBoard,
    projected: &PcbBoard,
    table: &TableChanges,
    table_rows: &mut [EcoRow],
) {
    for row in list.rows.iter().filter(|r| list.applies(r)) {
        let id = match &row.change {
            EcoChange::RemoveFootprint { footprint, .. }
            | EcoChange::ChangeFootprint { footprint, .. } => *footprint,
            _ => continue,
        };
        let Some(old) = board.footprints.iter().find(|f| f.uuid == id) else {
            continue;
        };
        let now = projected.footprints.iter().find(|f| f.uuid == id);
        for pad in &old.pads {
            let Some(net) = &pad.net else {
                continue;
            };
            let stays = now.is_some_and(|f| f.pads.iter().any(|p| p.number == pad.number));
            if stays || !table.removed.contains_key(&net.number) {
                continue;
            }
            let remove = EcoKey::RemoveNet(net.number);
            if let Some(r) = table_rows.iter_mut().find(|r| r.key == remove)
                && !r.needs.contains(&row.key)
            {
                r.needs.push(row.key.clone());
            }
        }
    }
}

/// How net sync changes the net table of the projected board.
struct TableChanges {
    /// New nets: number -> name.
    added: HashMap<u32, String>,
    /// Renamed nets: number -> (old name, new name).
    renamed: HashMap<u32, (String, String)>,
    /// Removed nets: number -> name.
    removed: HashMap<u32, String>,
}

impl TableChanges {
    fn of(projected: &PcbBoard, assignment: &BoardNetAssignment) -> Self {
        let before: HashMap<u32, &str> = projected
            .nets
            .iter()
            .map(|n| (n.number, n.name.as_str()))
            .collect();
        let after: HashSet<u32> = assignment.edit.nets.iter().map(|n| n.number).collect();
        let mut table = Self {
            added: HashMap::new(),
            renamed: HashMap::new(),
            removed: HashMap::new(),
        };
        for net in &assignment.edit.nets {
            match before.get(&net.number) {
                None => {
                    table.added.insert(net.number, net.name.clone());
                }
                Some(old) if *old != net.name => {
                    table
                        .renamed
                        .insert(net.number, ((*old).to_string(), net.name.clone()));
                }
                Some(_) => {}
            }
        }
        for net in &projected.nets {
            if !after.contains(&net.number) {
                table.removed.insert(net.number, net.name.clone());
            }
        }
        table
    }

    /// The table rows, sorted by key; their `needs` are filled in by
    /// [`pad_rows`].
    fn rows(&self) -> Vec<EcoRow> {
        let mut rows = Vec::new();
        for (number, name) in &self.added {
            rows.push(EcoRow::new(
                EcoKey::AddNet(name.clone()),
                EcoGroup::Nets,
                EcoChange::AddNet {
                    number: *number,
                    name: name.clone(),
                },
                true,
            ));
        }
        for (number, (from, to)) in &self.renamed {
            rows.push(EcoRow::new(
                EcoKey::RenameNet(*number),
                EcoGroup::Nets,
                EcoChange::RenameNet {
                    number: *number,
                    from: from.clone(),
                    to: to.clone(),
                },
                true,
            ));
        }
        for (number, name) in &self.removed {
            rows.push(EcoRow::new(
                EcoKey::RemoveNet(*number),
                EcoGroup::Nets,
                EcoChange::RemoveNet {
                    number: *number,
                    name: name.clone(),
                },
                true,
            ));
        }
        rows.sort_by_key(|r| r.key.clone());
        rows
    }
}

/// The pad rows, each needing the rows it only makes sense with; the table
/// rows in `table_rows` get the pad rows they need in turn.
fn pad_rows(
    list: &EcoChangeList,
    projected: &PcbBoard,
    assignment: &BoardNetAssignment,
    table: &TableChanges,
    table_rows: &mut [EcoRow],
) -> Vec<EcoRow> {
    // The footprint row that brought a projected footprint's pads.
    let brought: HashMap<Uuid, EcoKey> = list
        .rows
        .iter()
        .filter(|r| list.applies(r))
        .filter_map(|r| match &r.change {
            EcoChange::AddFootprint { footprint } => Some((footprint.uuid, r.key.clone())),
            EcoChange::ChangeFootprint { footprint, .. } => Some((*footprint, r.key.clone())),
            _ => None,
        })
        .collect();
    let mut add_needs = |key: &EcoKey, needed: EcoKey| {
        if let Some(row) = table_rows.iter_mut().find(|r| &r.key == key) {
            row.needs.push(needed);
        }
    };

    let mut rows = Vec::new();
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
        let from = pad.net.as_ref().map(|n| n.number);
        let to = change.net.as_ref().map(|n| n.number);
        // A locked pad is the user's: sync only renames its net in step with
        // the table, and only that is listed for it.
        if (pad.net_locked && from != to) || (pad.net == change.net && !change.locked) {
            continue;
        }
        let key = EcoKey::PadNet {
            footprint: fp.uuid,
            pad: pad.uuid,
        };
        let mut needs = Vec::new();
        if let Some(by) = brought.get(&fp.uuid) {
            needs.push(by.clone());
        }
        if let Some(number) = to {
            if let Some(name) = table.added.get(&number) {
                needs.push(EcoKey::AddNet(name.clone()));
            }
            if table.renamed.contains_key(&number) {
                needs.push(EcoKey::RenameNet(number));
                if from == to {
                    // Only following the rename: the rename needs it back.
                    add_needs(&EcoKey::RenameNet(number), key.clone());
                }
            }
        }
        if let Some(number) = from
            && from != to
            && table.removed.contains_key(&number)
        {
            add_needs(&EcoKey::RemoveNet(number), key.clone());
        }
        rows.push(
            EcoRow::new(
                key,
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
    rows
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
