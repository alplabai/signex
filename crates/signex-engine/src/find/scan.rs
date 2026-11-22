//! The pure hit scanner.

use super::{DesignatorScope, FindHit, FindKind, FindQuery};
use crate::TextTarget;
use signex_types::schematic::{SchematicSheet, SelectedItem, SelectedKind};

/// Every item of `sheet` whose text `query` matches under its options:
/// labels, notes, designators (at `scope`'s placements) and values, in that
/// order. An empty query finds nothing.
pub fn scan_sheet(
    sheet: &SchematicSheet,
    query: &FindQuery,
    scope: &DesignatorScope,
) -> Vec<FindHit> {
    if query.is_empty() {
        return Vec::new();
    }
    let contains = |text: &str| query.is_match(text);
    let mut hits = Vec::new();

    for label in sheet.labels.iter().filter(|l| contains(&l.text)) {
        hits.push(FindHit {
            kind: FindKind::NetLabel,
            item: SelectedItem::new(label.uuid, SelectedKind::Label),
            target: TextTarget::Label(label.uuid),
            text: label.text.clone(),
            placement: None,
            single: false,
            position: label.position,
        });
    }
    for note in sheet.text_notes.iter().filter(|n| contains(&n.text)) {
        hits.push(FindHit {
            kind: FindKind::TextNote,
            item: SelectedItem::new(note.uuid, SelectedKind::TextNote),
            target: TextTarget::TextNote(note.uuid),
            text: note.text.clone(),
            placement: None,
            single: false,
            position: note.position,
        });
    }
    for symbol in &sheet.symbols {
        let item = |kind| SelectedItem::new(symbol.uuid, kind);
        let mut designator = |placement: Option<&signex_types::schematic::InstancePath>| {
            let text = placement.map_or(symbol.reference.as_str(), |p| symbol.designator_in(p));
            if contains(text) {
                hits.push(FindHit {
                    kind: FindKind::Designator,
                    item: item(SelectedKind::SymbolRefField),
                    target: TextTarget::SymbolReference(symbol.uuid),
                    text: text.to_string(),
                    placement: placement.cloned(),
                    single: scope.single,
                    position: symbol.position,
                });
            }
        };
        // A power port's designator is never drawn; its text is the net name,
        // which the Value hit finds (#1799).
        if symbol.is_power {
            // no designator hit
        } else if scope.placements.is_empty() {
            designator(None);
        } else {
            for placement in &scope.placements {
                designator(Some(placement));
            }
        }
        if contains(&symbol.value) {
            // A power port's net name is part of the port, so a hit selects
            // the port (docs/DECISIONS.md, 2026-11-07).
            let kind = if symbol.is_power {
                SelectedKind::Symbol
            } else {
                SelectedKind::SymbolValField
            };
            hits.push(FindHit {
                kind: FindKind::Value,
                item: item(kind),
                target: TextTarget::SymbolValue(symbol.uuid),
                text: symbol.value.clone(),
                placement: None,
                single: false,
                position: symbol.position,
            });
        }
    }
    hits
}

/// The designator of the part a Value hit belongs to, at `scope`'s
/// placements (`R5`, or `R5, R105` for a sheet placed twice). `None` for any
/// other hit and for a power port, whose designator is never drawn.
pub fn value_part_name(
    sheet: &SchematicSheet,
    hit: &FindHit,
    scope: &DesignatorScope,
) -> Option<String> {
    if hit.kind != FindKind::Value {
        return None;
    }
    let symbol = sheet
        .symbols
        .iter()
        .find(|s| s.uuid == hit.item.uuid && !s.is_power)?;
    let read: Vec<&str> = if scope.placements.is_empty() {
        vec![symbol.reference.as_str()]
    } else {
        scope
            .placements
            .iter()
            .map(|p| symbol.designator_in(p))
            .collect()
    };
    let mut names: Vec<&str> = Vec::new();
    for name in read.into_iter().map(str::trim) {
        if !name.is_empty() && !names.contains(&name) {
            names.push(name);
        }
    }
    (!names.is_empty()).then(|| names.join(", "))
}
