//! Unions (#158): a named group of objects on one sheet that select, move
//! and copy together. A union owns no geometry; it lists its members by
//! uuid. File format: `docs/architecture/snxsch-refinement-objects.md`.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{SchematicSheet, SelectedKind};

/// `skip_serializing_if` predicate: a lock is written only when set.
fn is_false(value: &bool) -> bool {
    !*value
}

/// One union of sheet objects.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SchUnion {
    pub uuid: Uuid,
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// The uuids of the member objects. A member that names no object on the
    /// sheet is dropped when the sheet loads, with a load repair.
    #[serde(default)]
    pub members: Vec<Uuid>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub locked: bool,
}

/// Every object of `sheet` a union can hold, by id, with the kind it is
/// selected as: the one membership rule the loader and the engine share. A
/// sheet pin belongs to its sheet symbol and a symbol field to its symbol,
/// so neither is a member of its own, and a harness entry belongs to its
/// connector. A union is not a member of another.
#[must_use]
pub fn union_member_kinds(sheet: &SchematicSheet) -> HashMap<Uuid, SelectedKind> {
    let mut out = HashMap::new();
    let mut put = |id: Uuid, kind: SelectedKind| {
        out.insert(id, kind);
    };
    sheet
        .symbols
        .iter()
        .for_each(|o| put(o.uuid, SelectedKind::Symbol));
    sheet
        .wires
        .iter()
        .for_each(|o| put(o.uuid, SelectedKind::Wire));
    sheet
        .buses
        .iter()
        .for_each(|o| put(o.uuid, SelectedKind::Bus));
    sheet
        .bus_entries
        .iter()
        .for_each(|o| put(o.uuid, SelectedKind::BusEntry));
    sheet
        .junctions
        .iter()
        .for_each(|o| put(o.uuid, SelectedKind::Junction));
    sheet
        .no_connects
        .iter()
        .for_each(|o| put(o.uuid, SelectedKind::NoConnect));
    sheet
        .no_erc_directives
        .iter()
        .for_each(|o| put(o.uuid, SelectedKind::NoErcDirective));
    sheet
        .directives
        .iter()
        .for_each(|o| put(o.uuid(), SelectedKind::Directive));
    sheet
        .labels
        .iter()
        .for_each(|o| put(o.uuid, SelectedKind::Label));
    sheet
        .text_notes
        .iter()
        .for_each(|o| put(o.uuid, SelectedKind::TextNote));
    sheet
        .child_sheets
        .iter()
        .for_each(|o| put(o.uuid, SelectedKind::ChildSheet));
    sheet
        .drawings
        .iter()
        .for_each(|o| put(o.uuid(), SelectedKind::Drawing));
    sheet
        .harness
        .connectors
        .iter()
        .for_each(|o| put(o.uuid, SelectedKind::HarnessConnector));
    sheet
        .harness
        .wires
        .iter()
        .for_each(|o| put(o.uuid, SelectedKind::HarnessWire));
    out
}
