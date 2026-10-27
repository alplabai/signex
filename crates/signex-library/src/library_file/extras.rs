//! Keys a `.snxlib` holds that this build does not model. They are captured
//! on parse and written back so an unrelated edit never deletes them.

use toml::{Table, Value};

/// The part of `doc` that `known` (the same document re-serialised from the
/// typed structs) lacks, same nesting. Arrays align by index.
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
        (Value::Array(d), Value::Array(k)) => {
            let subs: Vec<Option<Value>> = d
                .iter()
                .enumerate()
                .map(|(i, v)| k.get(i).and_then(|kv| unknown_of(v, kv)))
                .collect();
            if subs.iter().all(Option::is_none) {
                return None;
            }
            Some(Value::Array(
                subs.into_iter()
                    .map(|s| s.unwrap_or_else(|| Value::Table(Table::new())))
                    .collect(),
            ))
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
            (Some(Value::Array(b)), Value::Array(e)) => {
                for (bv, ev) in b.iter_mut().zip(e) {
                    if let (Value::Table(bt), Value::Table(et)) = (bv, ev) {
                        merge_missing(bt, et);
                    }
                }
            }
            _ => {}
        }
    }
}
