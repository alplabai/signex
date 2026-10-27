//! Keys a `.snxlib` holds that this build does not model. They are captured
//! on parse and written back so an unrelated edit never deletes them.

use super::ClassEntry;
use toml::{Table, Value};

/// The part of `doc` that `known` (the same document re-serialised from the
/// typed structs) lacks, same nesting. Arrays are not walked: their elements have no stable
/// identity here, so `[[classes]]` is handled by key in [`class_extras`].
pub(super) fn unknown_of(doc: &Value, known: &Value) -> Option<Value> {
    match (doc, known) {
        (Value::Table(d), Value::Table(k)) => {
            let mut out = Table::new();
            for (key, value) in d {
                match k.get(key) {
                    None => {
                        out.insert(key.clone(), value.clone());
                    }
                    Some(known_value) => {
                        if let Some(sub) = unknown_of(value, known_value) {
                            out.insert(key.clone(), sub);
                        }
                    }
                }
            }
            (!out.is_empty()).then_some(Value::Table(out))
        }
        _ => None,
    }
}

/// Add `extra` into `base`, never replacing a value `base` already has.
pub(super) fn merge_missing(base: &mut Table, extra: &Table) {
    for (key, value) in extra {
        match (base.get_mut(key), value) {
            (None, _) => {
                base.insert(key.clone(), value.clone());
            }
            (Some(Value::Table(b)), Value::Table(e)) => merge_missing(b, e),
            _ => {}
        }
    }
}

/// Unknown keys of each `[[classes]]` entry, keyed by the class `key` (not its
/// position) so removing or reordering classes never moves a key onto another
/// class. `doc` and `classes` are the same list, parsed from one text.
pub(super) fn class_extras(doc: &[Value], classes: &[ClassEntry]) -> Table {
    let mut out = Table::new();
    for (entry, class) in doc.iter().zip(classes) {
        let Ok(known) = Value::try_from(class) else {
            continue;
        };
        if let Some(extra) = unknown_of(entry, &known) {
            out.entry(class.key.clone()).or_insert(extra);
        }
    }
    out
}

/// Put each class's saved unknown keys back into its `[[classes]]` table in
/// `doc`; a class that is gone from the list simply loses them.
pub(super) fn apply_class_extras(doc: &mut Table, by_key: &Table) {
    let Some(Value::Array(entries)) = doc.get_mut("classes") else {
        return;
    };
    for entry in entries {
        let Value::Table(t) = entry else { continue };
        let extra = t
            .get("key")
            .and_then(Value::as_str)
            .and_then(|k| by_key.get(k));
        if let Some(Value::Table(extra)) = extra {
            merge_missing(t, extra);
        }
    }
}
