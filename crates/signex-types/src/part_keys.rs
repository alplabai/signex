//! The names a part's fields go by in an expression (#1986), and how those
//! names follow the part when its designator changes.
//!
//! A part is named by its designator while that designator names it alone
//! (`R1.Value`; the units of one multi-unit part share it). A part with no
//! number yet (`R?`), or one whose designator another part also carries,
//! is named by its uuid (`_u<32 hex digits>.Value`), so a reference keeps
//! pointing at the same part through Reset Designators, a duplicate typed
//! by hand, or a renumbering, and returns to the designator once that names
//! the part again. [`follow_part_keys`] does the renaming; annotation and a
//! designator edit call it in the same undo step.

use std::collections::HashMap;

use uuid::Uuid;

use crate::schematic::Symbol;
use crate::variables::{FieldText, classify};

/// The prefix of a part key made from a uuid.
const UUID_KEY: &str = "_u";

/// Whether `reference` names a part: not blank and holding no `?`.
fn is_numbered(reference: &str) -> bool {
    !reference.is_empty() && !reference.contains('?')
}

/// Whether `other` is another part than `symbol` under the same designator
/// (not a unit of the same part).
fn is_other_part(symbol: &Symbol, other: &Symbol) -> bool {
    other.uuid != symbol.uuid
        && other.reference == symbol.reference
        && (other.lib_id != symbol.lib_id || other.unit == symbol.unit)
}

/// The uuid key of a part: `_u` and its 32 hex digits.
pub fn uuid_key(id: Uuid) -> String {
    format!("{UUID_KEY}{}", id.simple())
}

/// The key `symbol`'s fields are named under among `symbols` (its sheet):
/// its designator while that names it alone, else its uuid key.
pub fn part_key(symbol: &Symbol, symbols: &[Symbol]) -> String {
    let alone = !symbols.iter().any(|other| is_other_part(symbol, other));
    if is_numbered(&symbol.reference) && alone {
        symbol.reference.clone()
    } else {
        uuid_key(symbol.uuid)
    }
}

/// The part of `symbols` that `key` names: a designator (also as an export
/// copy prints it, with the leading `-` of the full-codes standard), else a
/// uuid key.
pub fn part_named<'a>(symbols: &'a [Symbol], key: &str) -> Option<&'a Symbol> {
    if let Some(hex) = key.strip_prefix(UUID_KEY)
        && let Ok(id) = Uuid::try_parse(hex)
    {
        return symbols.iter().find(|s| s.uuid == id);
    }
    if !is_numbered(key) {
        return None;
    }
    symbols
        .iter()
        .find(|s| s.reference == key || s.reference.strip_prefix('-') == Some(key))
}

/// `text` with the part references of `renames` (old key to new) renamed,
/// for an `=` expression; `None` when nothing changes. Only a name followed
/// by `.` is a part reference; a unit after a number (`10mA`) is never one.
pub fn rename_part_references(text: &str, renames: &HashMap<String, String>) -> Option<String> {
    if renames.is_empty() || !matches!(classify(text), FieldText::Expression(_)) {
        return None;
    }
    let mut out = String::with_capacity(text.len());
    let mut changed = false;
    let mut rest = text;
    let mut previous: Option<char> = None;
    while let Some(c) = rest.chars().next() {
        let starts_name = (c.is_ascii_alphabetic() || c == '_')
            && !previous.is_some_and(|p| p.is_ascii_alphanumeric() || p == '_' || p == '.');
        if !starts_name {
            out.push(c);
            previous = Some(c);
            rest = &rest[c.len_utf8()..];
            continue;
        }
        let end = rest
            .find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
            .unwrap_or(rest.len());
        let name = &rest[..end];
        match renames.get(name) {
            Some(new) if rest[end..].starts_with('.') => {
                out.push_str(new);
                changed = true;
            }
            _ => out.push_str(name),
        }
        previous = name.chars().last();
        rest = &rest[end..];
    }
    changed.then_some(out)
}

/// Rename the part references in every field expression of `symbol` (its
/// value, footprint and user fields); returns whether any changed.
pub fn rename_part_references_in(symbol: &mut Symbol, renames: &HashMap<String, String>) -> bool {
    let mut changed = false;
    let mut rename = |text: &mut String| {
        if let Some(renamed) = rename_part_references(text, renames) {
            *text = renamed;
            changed = true;
        }
    };
    rename(&mut symbol.value);
    rename(&mut symbol.footprint);
    symbol.fields.values_mut().for_each(&mut rename);
    for property in &mut symbol.custom_properties {
        rename(&mut property.value);
    }
    changed
}

/// After `symbols` changed designators (annotation, a designator edit),
/// rewrite every field expression on the sheet so a reference keeps naming
/// the same part: each part's key in `before` becomes its key now. Returns
/// whether any field changed.
pub fn follow_part_keys(symbols: &mut [Symbol], before: &[Symbol]) -> bool {
    let old_keys: HashMap<Uuid, String> = before
        .iter()
        .map(|symbol| (symbol.uuid, part_key(symbol, before)))
        .collect();
    let mut renames: HashMap<String, String> = HashMap::new();
    for symbol in symbols.iter() {
        let Some(old) = old_keys.get(&symbol.uuid) else {
            continue;
        };
        let new = part_key(symbol, symbols);
        if *old != new {
            renames.entry(old.clone()).or_insert(new);
        }
    }
    let mut changed = false;
    for symbol in symbols.iter_mut() {
        changed |= rename_part_references_in(symbol, &renames);
    }
    changed
}

#[cfg(test)]
#[path = "part_keys_tests.rs"]
mod tests;
