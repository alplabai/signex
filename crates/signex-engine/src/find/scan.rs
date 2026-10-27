//! The pure hit scanner.

use super::{DesignatorScope, FindHit, FindKind};
use crate::TextTarget;
use signex_types::schematic::{SchematicSheet, SelectedItem, SelectedKind};

/// Every case-insensitive occurrence of `needle` in `sheet`'s labels, notes,
/// designators (at `scope`'s placements) and values, in that order. An empty
/// needle finds nothing.
pub fn scan_sheet(sheet: &SchematicSheet, needle: &str, scope: &DesignatorScope) -> Vec<FindHit> {
    let needle = needle.trim().to_lowercase();
    if needle.is_empty() {
        return Vec::new();
    }
    let contains = |text: &str| text.to_lowercase().contains(&needle);
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
        if scope.placements.is_empty() {
            designator(None);
        } else {
            for placement in &scope.placements {
                designator(Some(placement));
            }
        }
        if contains(&symbol.value) {
            hits.push(FindHit {
                kind: FindKind::Value,
                item: item(SelectedKind::SymbolValField),
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
