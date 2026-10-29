//! Per-finding waivers with a reason (#1285), stored as
//! `ProjectRules::erc_waivers`.
//!
//! A waiver names a finding by its rule id and the UUIDs of the objects it
//! involves (never by position), and records a fingerprint of those objects'
//! content. Editing, moving or deleting any of them changes the fingerprint,
//! so the waiver lapses and the finding shows again. Design notes:
//! `docs/ERC_RULES.md`, "Waivers".

use std::collections::BTreeSet;

use serde::Serialize;
use sha2::{Digest, Sha256};
use signex_types::project::ErcWaiver;
use signex_types::schematic::{SchematicSheet, SelectedItem, SelectedKind};

use crate::Violation;

/// The identity and content fingerprint a finding would be waived by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaiverKey {
    pub rule_id: String,
    /// Involved object UUIDs, sorted and unique.
    pub objects: Vec<String>,
    pub fingerprint: String,
}

/// Why a waiver could not be made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WaiverError {
    /// The reason is blank.
    MissingReason,
    /// The author is blank.
    MissingAuthor,
    /// The finding names no object on its sheet, so nothing could make the
    /// waiver lapse.
    NotWaivable,
}

impl std::fmt::Display for WaiverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            WaiverError::MissingReason => "a waiver needs a reason",
            WaiverError::MissingAuthor => "a waiver needs an author",
            WaiverError::NotWaivable => "this finding names no object, so it cannot be waived",
        })
    }
}

/// The objects a finding involves: its primary and peer object and, for a
/// net-level finding, every part on the net.
pub fn involved_items(v: &Violation) -> Vec<SelectedItem> {
    let mut items: Vec<SelectedItem> = v.primary.into_iter().chain(v.peer).collect();
    if let Some(net) = &v.net {
        items.extend(
            net.pins
                .iter()
                .map(|p| SelectedItem::new(p.symbol, SelectedKind::Symbol)),
        );
    }
    items.sort_by_key(|i| i.uuid);
    items.dedup_by_key(|i| i.uuid);
    items
}

/// The finding's key on `sheet`, the sheet it is filed under. `None` when it
/// involves no object, or one that is not on `sheet`.
pub fn waiver_key(v: &Violation, sheet: &SchematicSheet) -> Option<WaiverKey> {
    let items = involved_items(v);
    if items.is_empty() {
        return None;
    }
    let rule_id = v.rule_id.to_string();
    let mut hasher = Sha256::new();
    hasher.update(rule_id.as_bytes());
    for item in &items {
        hasher.update(b"\n");
        hasher.update(item.uuid.to_string().as_bytes());
        hasher.update(b":");
        hasher.update(object_content(sheet, item)?.as_bytes());
    }
    Some(WaiverKey {
        rule_id,
        objects: items.iter().map(|i| i.uuid.to_string()).collect(),
        fingerprint: format!("{:x}", hasher.finalize()),
    })
}

/// A new waiver for the finding keyed `key`. Reason and author are trimmed
/// and required.
pub fn new_waiver(
    key: &WaiverKey,
    message: &str,
    reason: &str,
    author: &str,
    date: &str,
) -> Result<ErcWaiver, WaiverError> {
    let (reason, author) = (reason.trim(), author.trim());
    if reason.is_empty() {
        return Err(WaiverError::MissingReason);
    }
    if author.is_empty() {
        return Err(WaiverError::MissingAuthor);
    }
    if key.objects.is_empty() || key.fingerprint.is_empty() {
        return Err(WaiverError::NotWaivable);
    }
    Ok(ErcWaiver {
        rule_id: key.rule_id.clone(),
        objects: key.objects.clone(),
        fingerprint: key.fingerprint.clone(),
        reason: reason.to_string(),
        author: author.to_string(),
        date: date.to_string(),
        message: message.to_string(),
        unknown: Default::default(),
    })
}

/// Add `waiver`, replacing any stored waiver of the same finding. Returns
/// the new list (the input is not changed).
pub fn with_waiver(waivers: &[ErcWaiver], waiver: ErcWaiver) -> Vec<ErcWaiver> {
    let mut out: Vec<ErcWaiver> = waivers
        .iter()
        .filter(|w| !w.same_identity(&waiver.rule_id, &waiver.objects))
        .cloned()
        .collect();
    out.push(waiver);
    out
}

/// The list without the waiver of finding (`rule_id`, `objects`).
pub fn without_waiver(waivers: &[ErcWaiver], rule_id: &str, objects: &[String]) -> Vec<ErcWaiver> {
    waivers
        .iter()
        .filter(|w| !w.same_identity(rule_id, objects))
        .cloned()
        .collect()
}

/// What one ERC run made of the stored waivers.
#[derive(Debug, Default)]
pub struct WaiverSplit {
    /// Findings no waiver hides.
    pub kept: Vec<Violation>,
    /// Findings a waiver hides, with the index of that waiver in the list.
    pub waived: Vec<(Violation, usize)>,
    /// Indices of waivers whose finding is back because an involved object
    /// changed (same rule and objects, different fingerprint).
    pub lapsed: BTreeSet<usize>,
}

/// Split one sheet's findings by `waivers`. Each finding must carry its
/// [`Violation::waiver_key`] (see [`stamp_keys`]). An incomplete waiver (no
/// reason or author) never hides anything.
pub fn split_waived(violations: Vec<Violation>, waivers: &[ErcWaiver]) -> WaiverSplit {
    let mut split = WaiverSplit::default();
    for v in violations {
        let Some(key) = v.waiver_key.as_ref() else {
            split.kept.push(v);
            continue;
        };
        let same = waivers
            .iter()
            .enumerate()
            .find(|(_, w)| w.is_complete() && w.same_identity(&key.rule_id, &key.objects));
        match same {
            Some((i, w)) if w.fingerprint == key.fingerprint => split.waived.push((v, i)),
            Some((i, _)) => {
                split.lapsed.insert(i);
                split.kept.push(v);
            }
            None => split.kept.push(v),
        }
    }
    split
}

/// Fill [`Violation::waiver_key`] for every finding filed under `sheet`.
pub fn stamp_keys(violations: &mut [Violation], sheet: &SchematicSheet) {
    for v in violations {
        v.waiver_key = waiver_key(v, sheet);
    }
}

/// Canonical JSON of the object `item` names on `sheet`: keys sorted, so the
/// fingerprint does not depend on hash-map order.
fn object_content(sheet: &SchematicSheet, item: &SelectedItem) -> Option<String> {
    let id = item.uuid;
    let value = match item.kind {
        SelectedKind::Symbol | SelectedKind::SymbolRefField | SelectedKind::SymbolValField => {
            to_value(sheet.symbols.iter().find(|s| s.uuid == id)?)
        }
        SelectedKind::Wire => to_value(sheet.wires.iter().find(|w| w.uuid == id)?),
        SelectedKind::Bus => to_value(sheet.buses.iter().find(|b| b.uuid == id)?),
        SelectedKind::BusEntry => to_value(sheet.bus_entries.iter().find(|b| b.uuid == id)?),
        SelectedKind::Junction => to_value(sheet.junctions.iter().find(|j| j.uuid == id)?),
        SelectedKind::NoConnect => to_value(sheet.no_connects.iter().find(|n| n.uuid == id)?),
        SelectedKind::NoErcDirective => {
            to_value(sheet.no_erc_directives.iter().find(|n| n.uuid == id)?)
        }
        SelectedKind::Label => to_value(sheet.labels.iter().find(|l| l.uuid == id)?),
        SelectedKind::SheetPin => to_value(
            sheet
                .child_sheets
                .iter()
                .flat_map(|c| c.pins.iter())
                .find(|p| p.uuid == id)?,
        ),
        SelectedKind::ChildSheet => to_value(sheet.child_sheets.iter().find(|c| c.uuid == id)?),
        SelectedKind::TextNote => to_value(sheet.text_notes.iter().find(|t| t.uuid == id)?),
        SelectedKind::Drawing => return None,
    }?;
    let mut out = String::new();
    write_canonical(&value, &mut out);
    Some(out)
}

fn to_value(object: &impl Serialize) -> Option<serde_json::Value> {
    serde_json::to_value(object).ok()
}

fn write_canonical(value: &serde_json::Value, out: &mut String) {
    use serde_json::Value;
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            out.push('{');
            for (i, key) in keys.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&Value::String(key.clone()).to_string());
                out.push(':');
                write_canonical(&map[key], out);
            }
            out.push('}');
        }
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_canonical(item, out);
            }
            out.push(']');
        }
        other => out.push_str(&other.to_string()),
    }
}

#[cfg(test)]
mod tests;
