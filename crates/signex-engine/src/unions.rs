//! Named unions (#158): which union an object belongs to, the selection a
//! click on a member grows into, and the lock a locked union puts on its
//! members. The commands are in `exec/unions.rs`; the rules are in
//! `docs/SCHEMATIC_UNIONS.md`.
//!
//! An object is in at most one union, and a union lives on one sheet: it is
//! stored in that sheet's `unions` list and names its members by id.

use std::collections::{HashMap, HashSet};

use signex_types::schematic::{SchUnion, SchematicSheet, SelectedItem, SelectedKind};
use uuid::Uuid;

/// Why a union command was refused. Nothing is changed then.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum UnionRefusal {
    #[error("a union needs a name")]
    NameEmpty,
    #[error("a union needs at least one object on this sheet")]
    NoMembers,
    #[error("an object of the union is not on this sheet; a union holds objects of one sheet")]
    MemberNotOnSheet,
    #[error("{} already in the union {}; an object is in one union at most", objects_word(.objects), .unions.join(", "))]
    MemberInAnotherUnion { objects: usize, unions: Vec<String> },
    #[error("no union {0} is on this sheet")]
    NotFound(Uuid),
    #[error("a union with id {0} is already on this sheet")]
    IdInUse(Uuid),
    #[error(
        "the union {name} is locked and cannot be {action}; unlock it in the Unions panel first"
    )]
    Locked { name: String, action: &'static str },
}

fn objects_word(count: &usize) -> String {
    if *count == 1 {
        "an object is".to_string()
    } else {
        format!("{count} objects are")
    }
}

/// Every object of `sheet` a union can hold, by id, with the kind it is
/// selected as. A sheet pin belongs to its sheet symbol and a symbol field
/// to its symbol, so neither is a member of its own.
#[must_use]
pub fn object_kinds(sheet: &SchematicSheet) -> HashMap<Uuid, SelectedKind> {
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
    out
}

/// The union `id` of `sheet`.
#[must_use]
pub fn union_by_id(sheet: &SchematicSheet, id: Uuid) -> Option<&SchUnion> {
    sheet.unions.iter().find(|u| u.uuid == id)
}

/// The union `member` belongs to, if any. A symbol field counts as its
/// symbol (it carries the symbol's id).
#[must_use]
pub fn union_of(sheet: &SchematicSheet, member: Uuid) -> Option<&SchUnion> {
    sheet.unions.iter().find(|u| u.members.contains(&member))
}

/// The members of `union` as selection items, in the union's order; a member
/// the sheet no longer holds is left out.
#[must_use]
pub fn member_items(sheet: &SchematicSheet, union: &SchUnion) -> Vec<SelectedItem> {
    let kinds = object_kinds(sheet);
    union
        .members
        .iter()
        .filter_map(|id| kinds.get(id).map(|kind| SelectedItem::new(*id, *kind)))
        .collect()
}

/// `items` grown to whole unions: an item that is a member of a union brings
/// every member of it along. Order is kept and nothing is listed twice.
#[must_use]
pub fn expand_to_unions(sheet: &SchematicSheet, items: &[SelectedItem]) -> Vec<SelectedItem> {
    if sheet.unions.is_empty() {
        return items.to_vec();
    }
    let mut out: Vec<SelectedItem> = Vec::with_capacity(items.len());
    let mut seen: HashSet<SelectedItem> = HashSet::new();
    let mut grown: HashSet<Uuid> = HashSet::new();
    let mut kinds: Option<HashMap<Uuid, SelectedKind>> = None;
    for item in items {
        if seen.insert(*item) {
            out.push(*item);
        }
        let Some(union) = union_of(sheet, item.uuid) else {
            continue;
        };
        if !grown.insert(union.uuid) {
            continue;
        }
        let kinds = kinds.get_or_insert_with(|| object_kinds(sheet));
        for id in &union.members {
            let Some(kind) = kinds.get(id) else { continue };
            let member = SelectedItem::new(*id, *kind);
            // The symbol itself stands for its fields.
            let field_of_it = matches!(
                item.kind,
                SelectedKind::SymbolRefField | SelectedKind::SymbolValField
            ) && item.uuid == *id;
            if !field_of_it && seen.insert(member) {
                out.push(member);
            }
        }
    }
    out
}

/// The unions any of `items` belongs to, in sheet order.
#[must_use]
pub fn unions_of_items(sheet: &SchematicSheet, items: &[SelectedItem]) -> Vec<Uuid> {
    let ids: HashSet<Uuid> = items.iter().map(|i| i.uuid).collect();
    sheet
        .unions
        .iter()
        .filter(|u| u.members.iter().any(|m| ids.contains(m)))
        .map(|u| u.uuid)
        .collect()
}

/// Whether every one of `items` is a member of the union `union`, and there
/// is at least one: the selection is the union (a click selected it).
#[must_use]
pub fn selection_is_union(union: &SchUnion, items: &[SelectedItem]) -> bool {
    !union.members.is_empty()
        && union
            .members
            .iter()
            .all(|m| items.iter().any(|i| i.uuid == *m))
}

/// Tags as stored: trimmed, without a leading `#`, no empty ones, each once,
/// in the order given.
#[must_use]
pub fn normalized_tags(tags: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for tag in tags {
        let tag = tag.trim().trim_start_matches('#').trim();
        if !tag.is_empty() && !out.iter().any(|t| t == tag) {
            out.push(tag.to_string());
        }
    }
    out
}

/// Add every member of a locked union to `locked`, under the name a refusal
/// gives it ("union <name>"), unless the object is locked itself.
pub(crate) fn insert_locked_members(sheet: &SchematicSheet, locked: &mut HashMap<Uuid, String>) {
    for union in sheet.unions.iter().filter(|u| u.locked) {
        for id in &union.members {
            locked
                .entry(*id)
                .or_insert_with(|| format!("union {}", union.name));
        }
    }
}
