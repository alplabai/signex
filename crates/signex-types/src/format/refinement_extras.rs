//! The `[extras.sheet]` keys of Schematic Refinement content (`snxsch/1.3`):
//! `harness_connectors`, `harness_wires`, `harness_entry_types` and `unions`.
//! The keys themselves are fields of `SheetExtras`; this module turns the
//! entry-type table into uuids, filters it on save, and drops references to
//! objects that are gone on load. Format and rules:
//! `docs/architecture/snxsch-refinement-objects.md`.

use std::collections::{BTreeMap, HashSet};

use uuid::Uuid;

use super::FormatError;
use crate::schematic::SchematicSheet;

/// The table `harness_entry_types` is stored under, for error messages.
const ENTRY_TYPES_TABLE: &str = "extras.sheet.harness_entry_types";

/// How much of a bad key an error shows.
const KEY_SHOWN_CHARS: usize = 64;

/// The uuids of every sheet pin on the sheet.
fn sheet_pin_ids(sheet: &SchematicSheet) -> HashSet<Uuid> {
    sheet
        .child_sheets
        .iter()
        .flat_map(|child| child.pins.iter().map(|pin| pin.uuid))
        .collect()
}

/// `harness_entry_types` as it is written: by stringified sheet-pin uuid,
/// only for sheet pins that still exist.
pub(in crate::format) fn entry_types_for_write(sheet: &SchematicSheet) -> BTreeMap<String, String> {
    let pins = sheet_pin_ids(sheet);
    sheet
        .harness
        .entry_types
        .iter()
        .filter(|(id, _)| pins.contains(id))
        .map(|(id, harness_type)| (id.to_string(), harness_type.clone()))
        .collect()
}

/// `harness_entry_types` as read. A key that is not a uuid refuses the file,
/// naming the key: dropping it would lose the harness type of an entry.
pub(in crate::format) fn entry_types_from_read(
    table: BTreeMap<String, String>,
) -> Result<BTreeMap<Uuid, String>, FormatError> {
    table
        .into_iter()
        .map(|(key, harness_type)| match Uuid::parse_str(&key) {
            Ok(id) => Ok((id, harness_type)),
            Err(_) => Err(FormatError::KeyNotAUuid {
                table: ENTRY_TYPES_TABLE,
                // Cut short: the key is untrusted text shown in Messages.
                key: key.chars().take(KEY_SHOWN_CHARS).collect(),
            }),
        })
        .collect()
}

/// Every object uuid on the sheet a union can list: placed objects, sheet
/// pins, harness objects and other unions.
fn object_ids(sheet: &SchematicSheet) -> HashSet<Uuid> {
    let mut ids: HashSet<Uuid> = HashSet::new();
    ids.extend(sheet.symbols.iter().map(|o| o.uuid));
    ids.extend(sheet.wires.iter().map(|o| o.uuid));
    ids.extend(sheet.junctions.iter().map(|o| o.uuid));
    ids.extend(sheet.labels.iter().map(|o| o.uuid));
    ids.extend(sheet.child_sheets.iter().map(|o| o.uuid));
    ids.extend(sheet_pin_ids(sheet));
    ids.extend(sheet.no_connects.iter().map(|o| o.uuid));
    ids.extend(sheet.text_notes.iter().map(|o| o.uuid));
    ids.extend(sheet.buses.iter().map(|o| o.uuid));
    ids.extend(sheet.bus_entries.iter().map(|o| o.uuid));
    ids.extend(sheet.drawings.iter().map(|o| o.uuid()));
    ids.extend(sheet.no_erc_directives.iter().map(|o| o.uuid));
    ids.extend(sheet.directives.iter().map(|o| o.uuid()));
    for connector in &sheet.harness.connectors {
        ids.insert(connector.uuid);
        ids.extend(connector.entries.iter().map(|e| e.uuid));
    }
    ids.extend(sheet.harness.wires.iter().map(|o| o.uuid));
    ids.extend(sheet.unions.iter().map(|o| o.uuid));
    ids
}

/// On load, after uuid repair: an entry type whose sheet pin is gone and a
/// union member that names no object on the sheet are dropped, as entries
/// for missing drawings are. A union left with no member is kept.
pub(in crate::format) fn drop_dangling_references(sheet: &mut SchematicSheet) {
    let pins = sheet_pin_ids(sheet);
    sheet.harness.entry_types.retain(|id, _| pins.contains(id));
    if sheet.unions.is_empty() {
        return;
    }
    let ids = object_ids(sheet);
    for union in &mut sheet.unions {
        union.members.retain(|member| ids.contains(member));
    }
}
