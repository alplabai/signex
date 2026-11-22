//! What Move, Delete and Duplicate check before they run again (#2840): the
//! state of each target as the recorded command found and left it. A move
//! also reads the sheet's connectivity live, so it guards a fingerprint of
//! it. Rules: `docs/COMMAND_LOG.md`, "Replay".

use std::collections::BTreeMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use serde::Serialize;
use serde_json::Value;
use signex_engine::Command;
use signex_engine::pcb::BoardReplayGuard;
use signex_net::pt_key;
use signex_net::uf::Key;
use signex_types::schematic::{Point, SchematicSheet, SelectedItem, SelectedKind};
use uuid::Uuid;

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

/// A fingerprint of what a move reads besides its targets: everything that
/// makes a net (wires, buses and entries, dots, labels, no-connects, symbols
/// and their library pins, sheet symbols with their entries, harnesses). A
/// hash, so a row does not keep a copy of the sheet.
fn network(sheet: &SchematicSheet) -> Option<u64> {
    let bodies: BTreeMap<_, _> = sheet.lib_symbols.iter().collect();
    let parts = (
        (&sheet.wires, &sheet.buses, &sheet.bus_entries),
        (&sheet.junctions, &sheet.labels, &sheet.no_connects),
        (&sheet.symbols, &bodies, &sheet.child_sheets),
        (&sheet.no_erc_directives, &sheet.harness),
    );
    let bytes = serde_json::to_vec(&parts).ok()?;
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    Some(hasher.finish())
}

/// The points that place each object Copy carries of `kind`; empty for a
/// kind Copy leaves out.
fn anchors_of(sheet: &SchematicSheet, kind: SelectedKind) -> Vec<(Uuid, Vec<Point>)> {
    fn at<T>(items: &[T], of: impl Fn(&T) -> (Uuid, Vec<Point>)) -> Vec<(Uuid, Vec<Point>)> {
        items.iter().map(of).collect()
    }
    match kind {
        SelectedKind::Symbol => at(&sheet.symbols, |x| (x.uuid, vec![x.position])),
        SelectedKind::Wire => at(&sheet.wires, |x| (x.uuid, vec![x.start, x.end])),
        SelectedKind::Bus => at(&sheet.buses, |x| (x.uuid, vec![x.start, x.end])),
        SelectedKind::Label => at(&sheet.labels, |x| (x.uuid, vec![x.position])),
        SelectedKind::Junction => at(&sheet.junctions, |x| (x.uuid, vec![x.position])),
        SelectedKind::NoConnect => at(&sheet.no_connects, |x| (x.uuid, vec![x.position])),
        SelectedKind::NoErcDirective => {
            at(&sheet.no_erc_directives, |x| (x.uuid, vec![x.position]))
        }
        SelectedKind::TextNote => at(&sheet.text_notes, |x| (x.uuid, vec![x.position])),
        SelectedKind::HarnessConnector => {
            at(&sheet.harness.connectors, |x| (x.uuid, vec![x.position]))
        }
        SelectedKind::HarnessWire => at(&sheet.harness.wires, |x| (x.uuid, vec![x.start, x.end])),
        SelectedKind::HarnessEntry
        | SelectedKind::Directive
        | SelectedKind::BusEntry
        | SelectedKind::SheetPin
        | SelectedKind::ChildSheet
        | SelectedKind::Drawing
        | SelectedKind::SymbolRefField
        | SelectedKind::SymbolValField => Vec::new(),
    }
}

fn sorted_keys(points: impl Iterator<Item = Point>) -> Vec<Key> {
    let mut keys: Vec<Key> = points.map(|p| pt_key(&p)).collect();
    keys.sort_unstable();
    keys
}

/// Whether Duplicate's copy of one of `items`, `offset` away on both axes,
/// would land exactly on an object of its kind already there.
pub(super) fn copy_would_stack(
    sheet: &SchematicSheet,
    items: &[SelectedItem],
    offset: f64,
) -> bool {
    items.iter().any(|item| {
        let all = anchors_of(sheet, item.kind);
        let Some((_, points)) = all.iter().find(|(id, _)| *id == item.uuid) else {
            return false;
        };
        let shifted = points
            .iter()
            .map(|p| Point::new(p.x + offset, p.y + offset));
        let wanted = sorted_keys(shifted);
        (all.iter()).any(|(_, other)| sorted_keys(other.iter().copied()) == wanted)
    })
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

#[cfg(test)]
mod tests {
    use signex_types::corner::DragWiringMode;
    use signex_types::schematic::Wire;

    use super::*;

    const WIRE: Uuid = Uuid::from_u128(0x70);

    fn wire(y: f64) -> Wire {
        serde_json::from_value(serde_json::json!({
            "uuid": WIRE,
            "start": {"x": 0.0, "y": y},
            "end": {"x": 10.0, "y": y},
        }))
        .expect("a wire")
    }

    /// A guard of a move of the wire from y 0 to y 5.
    fn moved_wire_guard() -> (SchematicSheet, SheetTargetGuard) {
        let found = SchematicSheet {
            wires: vec![wire(0.0)],
            ..Default::default()
        };
        let command = Command::MoveSelection {
            items: vec![SelectedItem::new(WIRE, SelectedKind::Wire)],
            dx: 0.0,
            dy: 5.0,
            keep_wires: true,
            wiring: DragWiringMode::Auto,
        };
        let mut guard = SheetTargetGuard::for_move(&found, &[command]).expect("a move");
        let left = SchematicSheet {
            wires: vec![wire(5.0)],
            ..Default::default()
        };
        guard.record_after(&left);
        assert!(guard.holds(&found) && guard.holds(&left));
        (found, guard)
    }

    /// The review's case: an entry on the span would get a new path.
    #[test]
    fn a_move_is_refused_after_a_sheet_entry_lands_on_its_wire() {
        let (mut live, guard) = moved_wire_guard();
        live.child_sheets.push(
            serde_json::from_value(serde_json::json!({
                "uuid": Uuid::from_u128(0x71),
                "name": "Child",
                "filename": "child.snxsch",
                "position": {"x": 3.0, "y": -10.0},
                "size": [10.0, 10.0],
                "pins": [{
                    "uuid": Uuid::from_u128(0x72),
                    "name": "A",
                    "direction": "input",
                    "position": {"x": 5.0, "y": 0.0},
                    "auto_generated": false,
                }],
            }))
            .expect("a sheet symbol"),
        );

        assert!(!guard.holds(&live));
    }

    /// The review's case: the wire would leave the label and the net its name.
    #[test]
    fn a_move_is_refused_after_a_net_label_lands_on_its_wire() {
        let (mut live, guard) = moved_wire_guard();
        live.labels.push(
            serde_json::from_value(serde_json::json!({
                "uuid": Uuid::from_u128(0x73),
                "text": "VCC",
                "position": {"x": 5.0, "y": 0.0},
                "label_type": "net",
            }))
            .expect("a label"),
        );

        assert!(!guard.holds(&live));
    }

    #[test]
    fn a_copy_stacks_only_on_an_object_of_its_kind_at_the_offset() {
        let item = SelectedItem::new(WIRE, SelectedKind::Wire);
        let mut sheet = SchematicSheet {
            wires: vec![wire(0.0)],
            ..Default::default()
        };
        assert!(!copy_would_stack(&sheet, &[item], 5.0));

        let mut copy = wire(5.0);
        copy.uuid = Uuid::from_u128(0x74);
        copy.start.x += 5.0;
        copy.end.x += 5.0;
        sheet.wires.push(copy);

        assert!(copy_would_stack(&sheet, &[item], 5.0));
    }
}
