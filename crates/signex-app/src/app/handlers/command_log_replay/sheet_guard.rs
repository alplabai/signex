//! What Move, Delete and Duplicate check before they run again (#2840): the
//! state of each target as the recorded command found and left it. A move
//! also carries wires, buses and bus entries it reads live, so it guards a
//! fingerprint of them. Rules: `docs/COMMAND_LOG.md`, "Replay".

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use serde::Serialize;
use serde_json::Value;
use signex_engine::Command;
use signex_engine::pcb::BoardReplayGuard;
use signex_types::schematic::{SchematicSheet, SelectedItem, SelectedKind};

fn json<T: Serialize>(value: Option<&T>) -> Option<Value> {
    serde_json::to_value(value?).ok()
}

/// The object `item` names on `sheet`, as data; `None` when it is gone. A
/// designator or value field reads its symbol.
fn image(sheet: &SchematicSheet, item: &SelectedItem) -> Option<Value> {
    let id = item.uuid;
    match item.kind {
        SelectedKind::Symbol | SelectedKind::SymbolRefField | SelectedKind::SymbolValField => {
            json(sheet.symbols.iter().find(|x| x.uuid == id))
        }
        SelectedKind::Wire => json(sheet.wires.iter().find(|x| x.uuid == id)),
        SelectedKind::Bus => json(sheet.buses.iter().find(|x| x.uuid == id)),
        SelectedKind::BusEntry => json(sheet.bus_entries.iter().find(|x| x.uuid == id)),
        SelectedKind::Junction => json(sheet.junctions.iter().find(|x| x.uuid == id)),
        SelectedKind::NoConnect => json(sheet.no_connects.iter().find(|x| x.uuid == id)),
        SelectedKind::NoErcDirective => json(sheet.no_erc_directives.iter().find(|x| x.uuid == id)),
        SelectedKind::Label => json(sheet.labels.iter().find(|x| x.uuid == id)),
        SelectedKind::SheetPin => json(
            (sheet.child_sheets.iter())
                .flat_map(|child| &child.pins)
                .find(|x| x.uuid == id),
        ),
        SelectedKind::TextNote => json(sheet.text_notes.iter().find(|x| x.uuid == id)),
        SelectedKind::ChildSheet => json(sheet.child_sheets.iter().find(|x| x.uuid == id)),
        SelectedKind::Drawing => json(sheet.drawings.iter().find(|x| x.uuid() == id)),
        SelectedKind::Directive => json(sheet.directives.iter().find(|x| x.uuid() == id)),
        SelectedKind::HarnessConnector => {
            json(sheet.harness.connectors.iter().find(|x| x.uuid == id))
        }
        SelectedKind::HarnessEntry => json(
            (sheet.harness.connectors.iter())
                .flat_map(|connector| &connector.entries)
                .find(|x| x.uuid == id),
        ),
        SelectedKind::HarnessWire => json(sheet.harness.wires.iter().find(|x| x.uuid == id)),
    }
}

/// A fingerprint of what a move reads besides its targets. A hash, so a row
/// does not keep a copy of every wire.
fn network(sheet: &SchematicSheet) -> Option<u64> {
    let parts = (
        &sheet.wires,
        &sheet.buses,
        &sheet.bus_entries,
        &sheet.junctions,
    );
    let bytes = serde_json::to_vec(&parts).ok()?;
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    Some(hasher.finish())
}

#[derive(Debug, Clone, PartialEq)]
struct GuardedItem {
    item: SelectedItem,
    before: Option<Value>,
    after: Option<Value>,
}

/// The targets of a recorded sheet command, before and after it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SheetTargetGuard {
    targets: Vec<GuardedItem>,
    /// For a move: the fingerprint before it and after it.
    network: Option<(Option<u64>, Option<u64>)>,
}

impl SheetTargetGuard {
    /// `items` as they stand on `sheet`.
    pub(super) fn of_items(sheet: &SchematicSheet, items: &[SelectedItem]) -> Self {
        let mut targets: Vec<GuardedItem> = Vec::with_capacity(items.len());
        for item in items {
            if !targets.iter().any(|guarded| guarded.item == *item) {
                targets.push(GuardedItem {
                    item: *item,
                    before: image(sheet, item),
                    after: None,
                });
            }
        }
        Self {
            targets,
            network: None,
        }
    }

    /// The guard of `commands` when every one is a relative move.
    pub(super) fn for_move(sheet: &SchematicSheet, commands: &[Command]) -> Option<Self> {
        let mut items = Vec::new();
        for command in commands {
            match command {
                Command::MoveSelection { items: moved, .. } => items.extend(moved.iter().copied()),
                Command::MoveEach { moves, .. } => {
                    items.extend(moves.iter().map(|(item, _, _)| *item));
                }
                _ => return None,
            }
        }
        if items.is_empty() {
            return None;
        }
        let mut guard = Self::of_items(sheet, &items);
        guard.network = Some((network(sheet), None));
        Some(guard)
    }

    /// Read the same targets once the command has run.
    pub(super) fn record_after(&mut self, sheet: &SchematicSheet) {
        for guarded in &mut self.targets {
            guarded.after = image(sheet, &guarded.item);
        }
        if let Some((_, after)) = &mut self.network {
            *after = network(sheet);
        }
    }

    /// `self` followed by `later`, in one dispatch.
    pub(super) fn merged(mut self, later: Self) -> Self {
        for next in later.targets {
            match self.targets.iter_mut().find(|t| t.item == next.item) {
                Some(guarded) => guarded.after = next.after,
                None => self.targets.push(next),
            }
        }
        self.network = match (self.network, later.network) {
            (Some((before, _)), Some((_, after))) => Some((before, after)),
            _ => None,
        };
        self
    }

    /// Whether every target is as the command found or left it: a move run
    /// from either lands on a recorded offset, never over a later position.
    pub(super) fn holds(&self, sheet: &SchematicSheet) -> bool {
        let items = self.targets.iter().all(|guarded| {
            let live = image(sheet, &guarded.item);
            live.is_some() && (live == guarded.before || live == guarded.after)
        });
        let wires = self.network.is_none_or(|(before, after)| {
            let live = network(sheet);
            live.is_some() && (live == before || live == after)
        });
        items && wires
    }

    /// Whether every target is exactly as the command found it.
    pub(super) fn holds_as_found(&self, sheet: &SchematicSheet) -> bool {
        self.targets.iter().all(|guarded| {
            let live = image(sheet, &guarded.item);
            live.is_some() && live == guarded.before
        })
    }

    fn is_checkable(&self) -> bool {
        !self.targets.is_empty() && self.targets.iter().all(|t| t.before.is_some())
    }
}

/// What a Delete or Duplicate row found on its recorded targets.
#[derive(Debug, Clone)]
pub(crate) enum TargetGuard {
    Sheet(SheetTargetGuard),
    Board(BoardReplayGuard),
}

impl TargetGuard {
    pub(super) fn is_checkable(&self) -> bool {
        match self {
            Self::Sheet(guard) => guard.is_checkable(),
            Self::Board(guard) => guard.is_checkable(),
        }
    }
}
