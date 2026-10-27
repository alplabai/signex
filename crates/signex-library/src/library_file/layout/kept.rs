//! The guard on a layout write: every unknown key of the kept text that the
//! edit did not remove must be in the written text, so a case the merge gets
//! wrong refuses the save instead of losing the key.

use toml::{Table, Value};

use super::has_unknown;
use super::merge::{is_price_record, reader_drops, refetched};

/// The dotted path of the first unknown key of `text` (the kept text; `base`
/// its typed content when read) that `out` (the written text) lost although
/// `new` (the content written) still holds what the key belonged to. A key
/// whose owner the edit removed, a key a typed field took over, a record
/// fetched again, and a key under an array the edit replaced are not
/// expected back.
pub(in crate::library_file) fn lost_unknown_key(
    text: &Table,
    base: &Table,
    new: &Table,
    out: &Table,
) -> Option<String> {
    lost_in_table(text, base, new, Some(out), &mut Vec::new())
}

fn lost_in_table(
    text: &Table,
    base: &Table,
    new: &Table,
    out: Option<&Table>,
    path: &mut Vec<String>,
) -> Option<String> {
    text.iter().find_map(|(key, value)| {
        path.push(key.clone());
        let lost = lost_in(
            value,
            base.get(key),
            new.get(key),
            out.and_then(|o| o.get(key)),
            path,
        );
        path.pop();
        lost
    })
}

fn lost_in(
    text: &Value,
    base: Option<&Value>,
    new: Option<&Value>,
    out: Option<&Value>,
    path: &mut Vec<String>,
) -> Option<String> {
    let (base, new) = match (base, new) {
        (None, None) => {
            let lost = out != Some(text) && !reader_drops(path);
            return lost.then(|| path.join("."));
        }
        (Some(b), Some(n)) => (b, n),
        _ => return None,
    };
    if is_price_record(path) && refetched(Some(base), Some(new)) {
        return None;
    }
    match (text, base, new) {
        (Value::Table(t), Value::Table(b), Value::Table(n)) => {
            lost_in_table(t, b, n, out.and_then(Value::as_table), path)
        }
        (Value::Array(t), Value::Array(b), Value::Array(n)) if *path == ["classes"] => {
            lost_in_classes(t, b, n, out.and_then(Value::as_array), path)
        }
        (Value::Array(_), ..) => {
            let lost = base == new && has_unknown(text, base) && out != Some(text);
            lost.then(|| path.join("."))
        }
        _ => None,
    }
}

fn lost_in_classes(
    text: &[Value],
    base: &[Value],
    new: &[Value],
    out: Option<&Vec<Value>>,
    path: &mut Vec<String>,
) -> Option<String> {
    let key_of = |v: &Value| v.get("key").and_then(Value::as_str).map(str::to_owned);
    let by_key = |list: &[Value], key: &str| {
        list.iter()
            .find(|v| key_of(v).as_deref() == Some(key))
            .cloned()
    };
    text.iter().find_map(|entry| {
        let key = key_of(entry)?;
        let (Some(b), Some(n)) = (by_key(base, &key), by_key(new, &key)) else {
            return None;
        };
        let o = out.and_then(|o| by_key(o, &key));
        let (Value::Table(t), Value::Table(b), Value::Table(n)) = (entry, &b, &n) else {
            return None;
        };
        lost_in_table(t, b, n, o.as_ref().and_then(Value::as_table), path)
    })
}
