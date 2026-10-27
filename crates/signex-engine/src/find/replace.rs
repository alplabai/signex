//! The replacement builder: hits in, `Command`s out.

use super::{FindHit, FindKind};
use crate::{Command, TextTarget};
use signex_types::schematic::{InstancePath, SchematicSheet};

/// The commands that apply `replacement` for `needle` at `hit`, computed
/// from `sheet` as it is *now*. The hit's own text is never read: it dates
/// from the scan, and a replace that trusted it would write over an edit
/// made since. Empty when the item is gone or no longer contains `needle`.
///
/// A designator goes where a Properties edit of it would (#1151, #1200):
/// - at a non-root placement of a sheet placed more than once (or not known
///   to be placed once), the instance entry of that placement, so the other
///   placements keep their designators;
/// - at the only placement, `reference`, dropping that placement's entry;
/// - with no placement or the root chain, `reference`.
pub fn replace_commands(
    sheet: &SchematicSheet,
    hit: &FindHit,
    needle: &str,
    replacement: &str,
) -> Vec<Command> {
    let needle = needle.trim();
    let Some(live) = live_text(sheet, hit) else {
        return Vec::new();
    };
    let value = replace_all_ci(&live, needle, replacement);
    if value == live {
        return Vec::new();
    }
    match (hit.kind, &hit.target) {
        (FindKind::Designator, TextTarget::SymbolReference(symbol)) => {
            designator_commands(sheet, *symbol, hit.placement.as_ref(), hit.single, value)
        }
        (_, target) => vec![Command::UpdateText {
            target: *target,
            value,
        }],
    }
}

fn live_text(sheet: &SchematicSheet, hit: &FindHit) -> Option<String> {
    match hit.target {
        TextTarget::Label(id) => sheet
            .labels
            .iter()
            .find(|l| l.uuid == id)
            .map(|l| l.text.clone()),
        TextTarget::TextNote(id) => sheet
            .text_notes
            .iter()
            .find(|n| n.uuid == id)
            .map(|n| n.text.clone()),
        TextTarget::SymbolValue(id) => sheet
            .symbols
            .iter()
            .find(|s| s.uuid == id)
            .map(|s| s.value.clone()),
        TextTarget::SymbolReference(id) => {
            let symbol = sheet.symbols.iter().find(|s| s.uuid == id)?;
            Some(match &hit.placement {
                Some(placement) => symbol.designator_in(placement).to_string(),
                None => symbol.reference.clone(),
            })
        }
    }
}

/// The commands that set `symbol`'s designator to `value` at `placement`,
/// the one rule a Find/Replace and a Properties edit both follow (#1151,
/// #1200):
/// - at a non-root placement of a sheet placed more than once (or not known
///   to be placed once), the instance entry of that placement, so the other
///   placements keep their designators;
/// - at the only placement (`single`), `reference`, dropping that
///   placement's entry if it has one, so a simple design keeps a simple file;
/// - with no placement or the root chain (the engine rejects an entry for
///   it), `reference`.
pub fn designator_commands(
    sheet: &SchematicSheet,
    symbol: uuid::Uuid,
    placement: Option<&InstancePath>,
    single: bool,
    value: String,
) -> Vec<Command> {
    let set_reference = Command::UpdateText {
        target: TextTarget::SymbolReference(symbol),
        value: value.clone(),
    };
    let Some(placement) = placement.filter(|p| !p.is_root()).cloned() else {
        return vec![set_reference];
    };
    if !single {
        return vec![Command::SetSymbolDesignatorIn {
            symbol,
            placement,
            reference: value,
        }];
    }
    let has_entry = sheet
        .symbols
        .iter()
        .find(|s| s.uuid == symbol)
        .is_some_and(|s| s.has_instance_entry_in(&placement));
    if has_entry {
        vec![
            set_reference,
            Command::SetSymbolDesignatorIn {
                symbol,
                placement,
                reference: String::new(),
            },
        ]
    } else {
        vec![set_reference]
    }
}

/// Replace every occurrence of `needle` in `haystack` with `replacement`,
/// preserving the rest of the string. Case is ignored under the same
/// per-char rule the scan uses, so positions stay valid for any script. An
/// empty needle is a no-op (the find layer never produces an empty query).
pub fn replace_all_ci(haystack: &str, needle: &str, replacement: &str) -> String {
    let needle = super::fold::folded(needle);
    if needle.is_empty() {
        return haystack.to_string();
    }
    let chars: Vec<char> = haystack.chars().collect();
    let hay = super::fold::folded(haystack);
    let mut out = String::with_capacity(haystack.len());
    let mut i = 0;
    while i < chars.len() {
        if super::fold::matches_at(&hay, i, &needle) {
            out.push_str(replacement);
            i += needle.len();
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}
