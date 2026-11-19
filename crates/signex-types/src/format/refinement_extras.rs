//! The `[extras.sheet]` keys of Schematic Refinement content (`snxsch/1.3`):
//! `harness_connectors`, `harness_wires`, `harness_entry_types` and `unions`.
//! The keys themselves are fields of `SheetExtras`; this module turns the
//! entry-type table into uuids, filters it on save, and drops references to
//! objects that are gone on load. Format and rules:
//! `docs/architecture/snxsch-refinement-objects.md`.

use std::collections::{BTreeMap, HashSet};

use uuid::Uuid;

use super::{FormatError, SheetLoadRepair};
use crate::schematic::{SchematicSheet, union_member_kinds};

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

/// On load, after uuid repair: an entry type whose sheet pin is gone and a
/// union member that is no object a union can hold
/// ([`union_member_kinds`], the rule the engine keeps too) are dropped,
/// each with a load repair so Messages says so. A union left with no member
/// is kept.
pub(in crate::format) fn drop_dangling_references(
    sheet: &mut SchematicSheet,
) -> Vec<SheetLoadRepair> {
    let mut repairs = Vec::new();
    let pins = sheet_pin_ids(sheet);
    sheet.harness.entry_types.retain(|id, harness_type| {
        let kept = pins.contains(id);
        if !kept {
            repairs.push(SheetLoadRepair::HarnessEntryTypeDropped {
                pin: *id,
                harness_type: harness_type.clone(),
            });
        }
        kept
    });
    if sheet.unions.is_empty() {
        return repairs;
    }
    let kinds = union_member_kinds(sheet);
    for union in &mut sheet.unions {
        let name = union.name.clone();
        union.members.retain(|member| {
            let kept = kinds.contains_key(member);
            if !kept {
                repairs.push(SheetLoadRepair::UnionMemberDropped {
                    union: name.clone(),
                    member: *member,
                });
            }
            kept
        });
    }
    repairs
}
