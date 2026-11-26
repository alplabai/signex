//! Bottom-up hierarchy (#1276): a sheet symbol built from a sheet's ports,
//! and the two directions of Synchronize Sheet Entries and Ports with the
//! changes each would make, for the preview the user confirms first.
//!
//! Pure domain rules: the app reads the files and applies the commands. The
//! rename rule and the placement rules: docs/DECISIONS.md, "Bottom-up
//! hierarchy".

use std::collections::BTreeSet;

use signex_net::{SheetPinContacts, pt_key};
use signex_types::object_id::IdSource;
use signex_types::schematic::{
    ChildSheet, FillType, GRID_MM, HAlign, Label, LabelType, Point, SchematicSheet, SelectedItem,
    SelectedKind, SheetPin, VAlign, normalize_port_name, sheet_entry_port, sheet_entry_renamed,
};

use crate::command::{Command, SheetEntryRename, SheetPort, SymbolEntryRenames, TextTarget};
use crate::sheet::{exposed_sheet_ports, normalize_sheet_pin_direction};
use crate::sheet_placement::DEFAULT_CHILD_SHEET_STROKE_MM;

/// Width of a sheet symbol made from a sheet.
const FROM_SHEET_WIDTH_MM: f64 = 10.0 * GRID_MM;
/// Where the column of ports a sync adds to a sheet starts, and the left
/// edge of a sheet symbol placed with no cursor to go by.
const COLUMN_START_MM: f64 = 20.0 * GRID_MM;
/// Pitch of the column of added ports.
const PORT_ROW_MM: f64 = 4.0 * GRID_MM;
/// Gap left under the lowest object for a sheet symbol placed below it.
const BELOW_CONTENT_MM: f64 = 8.0 * GRID_MM;
/// The column of added ports stops looking for a free row here.
const MAX_PORT_ROWS: usize = 4096;

/// Which side a sync writes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SheetSyncDirection {
    /// The sheet's ports win: the sheet symbol's entries follow them.
    #[default]
    SheetToSymbol,
    /// The sheet symbol's entries win: the sheet's hierarchical ports
    /// follow them.
    SymbolToSheet,
}

/// What a sync would change on the side it writes, by name.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SheetSyncPreview {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub renamed: Vec<SheetEntryRename>,
}

impl SheetSyncPreview {
    /// Nothing to change: both sides already agree.
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.renamed.is_empty()
    }

    /// This preview without the removals of `kept`, the names the user left
    /// out; additions and renames stay.
    pub fn leaving_out(&self, kept: &BTreeSet<String>) -> Self {
        Self {
            added: self.added.clone(),
            removed: self
                .removed
                .iter()
                .filter(|name| !kept.contains(*name))
                .cloned()
                .collect(),
            renamed: self.renamed.clone(),
        }
    }
}

/// A name only one side has, with its normalised direction.
struct Unmatched {
    name: String,
    direction: &'static str,
}

fn count_of(list: &[Unmatched], direction: &str) -> usize {
    list.iter().filter(|u| u.direction == direction).count()
}

/// Split the names only one side has into renames and the rest. Per
/// direction, exactly one name gone and exactly one name new is a rename;
/// anything else is ambiguous and stays a removal plus an addition.
fn pair_renames(gone: Vec<Unmatched>, new: Vec<Unmatched>) -> SheetSyncPreview {
    let mut preview = SheetSyncPreview::default();
    let mut paired: Vec<String> = Vec::new();
    for old in &gone {
        let single = count_of(&gone, old.direction) == 1 && count_of(&new, old.direction) == 1;
        match new.iter().find(|n| n.direction == old.direction) {
            Some(partner) if single => {
                preview.renamed.push(SheetEntryRename {
                    from: old.name.clone(),
                    to: partner.name.clone(),
                });
                paired.push(partner.name.clone());
            }
            _ => preview.removed.push(old.name.clone()),
        }
    }
    preview.added = new
        .into_iter()
        .filter(|n| !paired.contains(&n.name))
        .map(|n| n.name)
        .collect();
    preview
}

/// What Sheet to Symbol would change on `symbol`, given the `ports` its
/// sheet exposes: the ports with no entry are added, the auto-generated
/// entries with no port are removed (a hand-placed entry stays, as Sync
/// Sheet Pins keeps it), and a lone pair of one direction is a rename. An
/// entry meets its port by `sheet_entry_port`: on a repeated symbol
/// `Repeat(VOUT)` meets `VOUT` (#2659), on a plain one only a port named
/// `Repeat(VOUT)` does. Removals and renames name the entry as written.
pub fn entry_changes_from_sheet(symbol: &ChildSheet, ports: &[SheetPort]) -> SheetSyncPreview {
    let has_port = |name: &str| ports.iter().any(|port| port.name == name);
    let gone = symbol
        .pins
        .iter()
        .filter(|pin| pin.auto_generated)
        .filter_map(|pin| {
            let name = normalize_port_name(&pin.name)?;
            let port = sheet_entry_port(&symbol.name, name)?;
            (!has_port(port)).then(|| Unmatched {
                name: name.to_string(),
                direction: normalize_sheet_pin_direction(&pin.direction),
            })
        })
        .collect();
    let new = ports
        .iter()
        .filter(|port| {
            !symbol
                .pins
                .iter()
                .any(|pin| sheet_entry_port(&symbol.name, &pin.name) == Some(port.name.as_str()))
        })
        .map(|port| Unmatched {
            name: port.name.clone(),
            direction: normalize_sheet_pin_direction(&port.direction),
        })
        .collect();
    pair_renames(gone, new)
}

/// The command that applies Sheet to Symbol for every sheet symbol of
/// `symbol`'s file: `ports` as `ports_for_pin_sync` gives them, and the
/// renames of `preview`, which was made for `symbol` and renames only its
/// entries.
pub fn sync_from_sheet_command(
    symbol: &ChildSheet,
    ports: Vec<SheetPort>,
    preview: &SheetSyncPreview,
) -> Command {
    let renames = (!preview.renamed.is_empty()).then(|| SymbolEntryRenames {
        symbol: symbol.uuid,
        renames: preview.renamed.clone(),
    });
    Command::ReconcileChildSheetPins {
        child_filename: symbol.filename.clone(),
        ports,
        renames,
    }
}

/// Rename the auto-generated entries of `symbol` in place, keeping position,
/// direction and id, when `renames` names this symbol. A hand-placed entry
/// keeps its name, a rename whose new port already has an entry on the
/// symbol is skipped, and on a repeated symbol a `Repeat(<net>)` entry stays
/// a repeat of the new port; on a plain one it takes the port's name.
pub(crate) fn rename_sheet_entries(symbol: &mut ChildSheet, renames: &SymbolEntryRenames) -> bool {
    if symbol.uuid != renames.symbol {
        return false;
    }
    let mut changed = false;
    for rename in &renames.renames {
        let Some(to) = normalize_port_name(&rename.to) else {
            continue;
        };
        if symbol
            .pins
            .iter()
            .any(|pin| sheet_entry_port(&symbol.name, &pin.name) == Some(to))
        {
            continue;
        }
        let from = rename.from.trim();
        let symbol_name = symbol.name.as_str();
        if let Some(pin) = symbol
            .pins
            .iter_mut()
            .find(|pin| pin.auto_generated && normalize_port_name(&pin.name) == Some(from))
        {
            pin.name = sheet_entry_renamed(symbol_name, &pin.name, to);
            changed = true;
        }
    }
    changed
}

/// The Hierarchical labels of `child` that expose the port `name`.
fn hierarchical_labels<'a>(
    child: &'a SchematicSheet,
    name: &'a str,
) -> impl Iterator<Item = &'a Label> + 'a {
    child.labels.iter().filter(move |label| {
        matches!(label.label_type, LabelType::Hierarchical) && label.port_name() == Some(name)
    })
}

/// What Symbol to Sheet would change on `child`, the sheet `symbol` stands
/// for: an entry with no port adds a Hierarchical port, a port carried by a
/// Hierarchical label with no entry is removed (a Global label is a net of
/// its own and is never touched), and a lone pair of one direction is a
/// rename. Each entry stands for the port `sheet_entry_port` gives it, so on
/// a repeated symbol a `Repeat(VOUT)` entry keeps or adds the port `VOUT`
/// (#2659).
pub fn port_changes_from_symbol(symbol: &ChildSheet, child: &SchematicSheet) -> SheetSyncPreview {
    let ports = exposed_sheet_ports(child);
    let mut entries: Vec<(&str, &'static str)> = Vec::new();
    for pin in &symbol.pins {
        if let Some(name) = sheet_entry_port(&symbol.name, &pin.name)
            && !entries.iter().any(|(seen, _)| *seen == name)
        {
            entries.push((name, normalize_sheet_pin_direction(&pin.direction)));
        }
    }
    let gone = ports
        .iter()
        .filter(|port| {
            !entries.iter().any(|(name, _)| *name == port.name)
                && hierarchical_labels(child, &port.name).next().is_some()
        })
        .map(|port| Unmatched {
            name: port.name.clone(),
            direction: normalize_sheet_pin_direction(&port.direction),
        })
        .collect();
    let new = entries
        .iter()
        .filter(|(name, _)| !ports.iter().any(|port| port.name == *name))
        .map(|&(name, direction)| Unmatched {
            name: name.to_string(),
            direction,
        })
        .collect();
    pair_renames(gone, new)
}

/// The commands that apply Symbol to Sheet to `child`, run as one batch: a
/// renamed port's labels take the new text in place, a removed port's
/// Hierarchical labels are deleted, and an added port is a Hierarchical
/// label with its entry's direction, in a column at the sheet's left where
/// it touches nothing (`SheetPinContacts`). New ids come from `ids`.
pub fn port_sync_commands(
    child: &SchematicSheet,
    symbol: &ChildSheet,
    preview: &SheetSyncPreview,
    ids: &mut dyn IdSource,
) -> Vec<Command> {
    let mut commands = Vec::new();
    for rename in &preview.renamed {
        for label in hierarchical_labels(child, rename.from.as_str()) {
            commands.push(Command::UpdateText {
                target: TextTarget::Label(label.uuid),
                value: rename.to.clone(),
            });
        }
    }
    let doomed: Vec<SelectedItem> = preview
        .removed
        .iter()
        .flat_map(|name| hierarchical_labels(child, name.as_str()))
        .map(|label| SelectedItem::new(label.uuid, SelectedKind::Label))
        .collect();
    if !doomed.is_empty() {
        commands.push(Command::DeleteSelection { items: doomed });
    }
    let contacts = SheetPinContacts::of_sheet(child);
    let mut taken: Vec<Point> = Vec::new();
    for name in &preview.added {
        let direction = symbol
            .pins
            .iter()
            .find(|pin| sheet_entry_port(&symbol.name, &pin.name) == Some(name.as_str()))
            .map_or("bidirectional", |pin| {
                normalize_sheet_pin_direction(&pin.direction)
            });
        let at = free_port_spot(&contacts, &taken);
        taken.push(at);
        commands.push(Command::PlaceLabel {
            label: Label {
                uuid: ids.next_id(),
                shape: direction.to_string(),
                justify: HAlign::default(),
                justify_v: VAlign::default(),
                ..Label::new(name.as_str(), at, LabelType::Hierarchical)
            },
        });
    }
    commands
}

/// The first row of the added-port column that connects to nothing and is
/// not taken by a port added in the same run.
fn free_port_spot(contacts: &SheetPinContacts, taken: &[Point]) -> Point {
    let spot = |row: usize| Point::new(COLUMN_START_MM, COLUMN_START_MM + PORT_ROW_MM * row as f64);
    (0..MAX_PORT_ROWS)
        .map(spot)
        .find(|p| !contacts.hits(p.x, p.y) && !taken.iter().any(|t| pt_key(t) == pt_key(p)))
        .unwrap_or_else(|| spot(MAX_PORT_ROWS))
}

/// A sheet symbol for `child`, referenced as `filename` and named `name`,
/// with its top-left corner at `at`: one auto-generated entry per port the
/// sheet exposes, outputs down the right edge and every other direction
/// down the left, one grid apart, so the symbol and the sheet agree with
/// no sync. Ids come from `ids`.
pub fn sheet_symbol_from_sheet(
    child: &SchematicSheet,
    filename: &str,
    name: &str,
    at: Point,
    ids: &mut dyn IdSource,
) -> ChildSheet {
    let ports = exposed_sheet_ports(child);
    let outputs = ports
        .iter()
        .filter(|port| normalize_sheet_pin_direction(&port.direction) == "output")
        .count();
    let rows = outputs.max(ports.len() - outputs).max(1);
    let size = (FROM_SHEET_WIDTH_MM, GRID_MM * (rows + 1) as f64);
    let uuid = ids.next_id();
    let (mut left, mut right) = (0usize, 0usize);
    let pins = ports
        .iter()
        .map(|port| {
            let direction = normalize_sheet_pin_direction(&port.direction);
            let (x, slot, rotation) = if direction == "output" {
                right += 1;
                (at.x + size.0, right, 180.0)
            } else {
                left += 1;
                (at.x, left, 0.0)
            };
            SheetPin {
                uuid: ids.next_id(),
                name: port.name.clone(),
                direction: direction.to_string(),
                position: Point::new(x, at.y + GRID_MM * slot as f64),
                rotation,
                auto_generated: true,
                user_moved: false,
            }
        })
        .collect();
    ChildSheet {
        uuid,
        name: name.to_string(),
        filename: filename.to_string(),
        position: at,
        size,
        stroke_width: DEFAULT_CHILD_SHEET_STROKE_MM,
        fill: FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins,
        instances: Vec::new(),
    }
}

/// `stem` when no sheet symbol of `parent` has that name, else the first
/// free `stem_2`, `stem_3`, ...
pub fn unused_sheet_symbol_name(parent: &SchematicSheet, stem: &str) -> String {
    let taken = |name: &str| parent.child_sheets.iter().any(|symbol| symbol.name == name);
    if !taken(stem) {
        return stem.to_string();
    }
    (2u32..)
        .map(|n| format!("{stem}_{n}"))
        .find(|candidate| !taken(candidate))
        .unwrap_or_else(|| stem.to_string())
}

/// Where a sheet symbol goes when there is no cursor to put it at: on the
/// grid, a gap under the lowest wire, part, label or sheet symbol of
/// `parent`, or near the top left of an empty sheet.
pub fn sheet_symbol_spot(parent: &SchematicSheet) -> Point {
    let lowest = parent
        .wires
        .iter()
        .flat_map(|wire| [wire.start.y, wire.end.y])
        .chain(parent.symbols.iter().map(|symbol| symbol.position.y))
        .chain(parent.labels.iter().map(|label| label.position.y))
        .chain(
            parent
                .child_sheets
                .iter()
                .map(|symbol| symbol.position.y + symbol.size.1),
        )
        .filter(|y| y.is_finite())
        .reduce(f64::max);
    match lowest {
        Some(y) => Point::new(
            COLUMN_START_MM,
            (y / GRID_MM).ceil() * GRID_MM + BELOW_CONTENT_MM,
        ),
        None => Point::new(COLUMN_START_MM, COLUMN_START_MM),
    }
}
