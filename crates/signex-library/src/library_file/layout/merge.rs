//! Applies the typed edit to the kept text: [`merge_table`] brings a kept
//! table to the current content, leaving alone what the edit did not touch.

use toml::Value;
use toml_edit::{Array, ArrayOfTables, Item, Table, TableLike};

/// Bring `dst` (kept text, untyped `src`) to `new` (current content, editable
/// `new_t`). `base` is what the content was when `dst` was read: a key in
/// `dst` that `base` and `new` both lack is unknown and stays; a key `base`
/// had and `new` lacks was removed by the edit and goes, with whatever unknown
/// keys sat under it; a key `new` adds is appended unless `base` already had
/// it unchanged (an omitted default). Returns whether the edit emptied `dst`.
pub(super) fn merge_table(
    dst: &mut dyn TableLike,
    src: &toml::Table,
    base: Option<&toml::Table>,
    new: &toml::Table,
    new_t: &dyn TableLike,
    path: &mut Vec<String>,
) -> bool {
    let keys: Vec<String> = dst.iter().map(|(k, _)| k.to_owned()).collect();
    for key in keys {
        path.push(key.clone());
        let base_v = base.and_then(|b| b.get(&key));
        // A key removed and re-added within one save is restored with its old
        // unknown keys on purpose: the edit cannot tell it from an untouched key.
        match (new.get(&key), new_t.get(&key), dst.get_mut(&key)) {
            (Some(nv), Some(ni), Some(d)) => {
                let was_value = d.is_value();
                merge_item(d, src.get(&key), base_v, nv, ni, path);
                if was_value
                    && !d.is_value()
                    && let Some(mut k) = dst.key_mut(&key)
                {
                    // `key = ` spacing must not leak into a `[header]`.
                    k.leaf_decor_mut().clear();
                }
            }
            _ if base_v.is_some() || reader_drops(path) => {
                dst.remove(&key);
            }
            _ => {}
        }
        path.pop();
    }
    // A table the edit emptied would read back as absent, which can default
    // differently from what it had (`[users]` keeps per-field defaults only
    // when present), so it is written out in full.
    let emptied = dst.is_empty();
    for (key, nv) in new {
        let unchanged_default = !emptied && base.and_then(|b| b.get(key)) == Some(nv);
        if dst.contains_key(key) || unchanged_default {
            continue;
        }
        if let Some(item) = new_t.get(key) {
            dst.insert(key, detach(item));
        }
    }
    emptied
}

fn merge_item(
    dst: &mut Item,
    src: Option<&Value>,
    base: Option<&Value>,
    new: &Value,
    new_item: &Item,
    path: &mut Vec<String>,
) {
    if is_price_record(path) && refetched(base, Some(new)) {
        // A re-fetched price replaces the record: its unknown keys described
        // the old fetch.
        replace(dst, new_item);
        return;
    }
    let base_t = base.and_then(Value::as_table);
    match (src, new) {
        (Some(Value::Table(s)), Value::Table(nv)) if dst.is_table_like() => {
            if let (Some(d), Some(n)) = (dst.as_table_like_mut(), new_item.as_table_like()) {
                let emptied = merge_table(d, s, base_t, nv, n, path);
                show_if_it_has_values(dst, emptied);
                return;
            }
        }
        (Some(Value::Array(s)), Value::Array(nv)) if *path == ["classes"] => {
            if let Some(n) = new_item.as_array_of_tables() {
                let base = base.and_then(Value::as_array).map(Vec::as_slice);
                if merge_classes(dst, s, base, nv, n, path) {
                    return;
                }
            }
        }
        _ => {}
    }
    // A string (the TSV block) is checked against the text itself, so a row
    // the reader repaired is written repaired.
    let unchanged = src == Some(new) || (!new.is_str() && base == Some(new));
    if !unchanged {
        replace(dst, new_item);
    }
}

/// A `[section]` that only held sub-tables is not written as a header of its
/// own; once the edit refilled it with values it must be.
fn show_if_it_has_values(dst: &mut Item, emptied: bool) {
    if let Item::Table(t) = dst
        && emptied
        && t.iter().any(|(_, item)| item.is_value())
    {
        t.set_implicit(false);
    }
}

/// `[[classes]]` or `classes = [{ … }]`, matched by class `key` so a removed
/// or reordered class never hands its unknown keys to another. Returns false
/// when `dst` is neither shape.
fn merge_classes(
    dst: &mut Item,
    src: &[Value],
    base: Option<&[Value]>,
    new: &[Value],
    new_aot: &ArrayOfTables,
    path: &mut Vec<String>,
) -> bool {
    let key_of = |v: &Value| v.get("key").and_then(Value::as_str).map(str::to_owned);
    let find = |list: &[Value], key: &Option<String>| {
        list.iter().position(|v| key.is_some() && key_of(v) == *key)
    };
    let inline = dst
        .as_array()
        .is_some_and(|a| a.iter().all(toml_edit::Value::is_inline_table));
    if !inline && !dst.is_array_of_tables() {
        return false;
    }
    let mut tables: Vec<Item> = Vec::new();
    for (nv, nt) in new.iter().zip(new_aot.iter()) {
        let key = key_of(nv);
        let kept = find(src, &key).and_then(|i| Some((class_entry(dst, i)?, src[i].as_table()?)));
        match (kept, nv.as_table()) {
            (Some((mut entry, s)), Some(nvt)) => {
                let b = base
                    .and_then(|b| find(b, &key).map(|j| &b[j]))
                    .and_then(Value::as_table);
                if let Some(t) = entry.as_table_like_mut() {
                    merge_table(t, s, b, nvt, nt, path);
                }
                tables.push(entry);
            }
            _ => tables.push(Item::Table(detach_table(nt))),
        }
    }
    *dst = if let Some(list) = dst.as_array().filter(|_| inline) {
        inline_classes(list, tables)
    } else {
        classes_section(tables)
    };
    true
}

/// Entry `i` of a `[[classes]]` or inline `classes` list, as an item.
fn class_entry(classes: &Item, i: usize) -> Option<Item> {
    match classes {
        Item::ArrayOfTables(a) => a.get(i).cloned().map(Item::Table),
        Item::Value(toml_edit::Value::Array(a)) => a.get(i).cloned().map(Item::Value),
        _ => None,
    }
}

/// `entries` in place of `list`'s, keeping how `list` itself is written.
fn inline_classes(list: &Array, entries: Vec<Item>) -> Item {
    let mut out = list.clone();
    out.clear();
    for entry in entries {
        match entry {
            Item::Value(v) => out.push_formatted(v),
            Item::Table(t) => out.push(t.into_inline_table()),
            _ => {}
        }
    }
    Item::Value(out.into())
}

fn classes_section(entries: Vec<Item>) -> Item {
    let tables: Vec<Table> = entries
        .into_iter()
        .filter_map(|e| e.into_table().ok())
        .collect();
    let positions: Vec<Option<usize>> = tables.iter().map(Table::position).collect();
    let mut out = ArrayOfTables::new();
    tables.into_iter().for_each(|t| out.push(t));
    // Tables are written in position order: a reorder must not be undone by
    // the positions the kept entries had.
    let kept: Vec<usize> = positions.into_iter().flatten().collect();
    if !kept.is_sorted()
        && let Some(first) = kept.iter().min().copied()
    {
        out.iter_mut().for_each(|t| t.set_position(first));
    }
    Item::ArrayOfTables(out)
}

/// Under `[tables.<name>.pricing]` the reader drops a row, distributor or
/// record it cannot use (it is fetched again); such a key is not kept.
pub(super) fn reader_drops(path: &[String]) -> bool {
    is_under_pricing(path) && path.len() <= 6
}

/// `path` names one cached price record.
pub(super) fn is_price_record(path: &[String]) -> bool {
    is_under_pricing(path) && path.len() == 6
}

fn is_under_pricing(path: &[String]) -> bool {
    matches!(path, [t, _, p, ..] if t == "tables" && p == "pricing")
}

/// The record was fetched again since the read.
pub(super) fn refetched(base: Option<&Value>, new: Option<&Value>) -> bool {
    let at = |v: Option<&Value>| v.and_then(|v| v.get("fetched_at")).cloned();
    at(base) != at(new)
}

fn replace(dst: &mut Item, new_item: &Item) {
    match (dst, new_item) {
        (Item::Value(d), Item::Value(n)) => {
            let decor = d.decor().clone();
            *d = n.clone();
            // The canonical writer spreads an array over lines; written into
            // a hand-laid file it reads better on one.
            match d {
                toml_edit::Value::Array(a) => a.fmt(),
                toml_edit::Value::InlineTable(t) => t.fmt(),
                _ => {}
            }
            *d.decor_mut() = decor;
        }
        (dst, n) => *dst = detach(n),
    }
}

/// `item` without the document positions it had in its own document, so it
/// is written after whatever precedes it in this one.
fn detach(item: &Item) -> Item {
    match item {
        Item::Table(t) => Item::Table(detach_table(t)),
        Item::ArrayOfTables(a) => {
            let mut out = ArrayOfTables::new();
            a.iter().for_each(|t| out.push(detach_table(t)));
            Item::ArrayOfTables(out)
        }
        other => other.clone(),
    }
}

fn detach_table(t: &Table) -> Table {
    let mut out = Table::new();
    out.set_implicit(t.is_implicit());
    out.set_dotted(t.is_dotted());
    *out.decor_mut() = t.decor().clone();
    for (key, item) in t.iter() {
        if let Some(k) = t.key(key) {
            out.insert_formatted(k, detach(item));
        }
    }
    out
}
