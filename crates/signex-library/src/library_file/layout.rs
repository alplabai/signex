//! A `.snxlib` that holds keys this build does not model keeps the text it
//! was read from (#1435). A write edits that text in place, changing only what
//! the typed content changed since the read, so the unknown keys, the key
//! order and every untouched row come back byte-for-byte.

use std::fmt;
use std::sync::Arc;

use toml::Value;
use toml_edit::{ArrayOfTables, DocumentMut, Item, Table};

use super::{LibraryFile, LibraryFileError};

/// The text a [`LibraryFile`] was parsed from, kept only when it holds keys
/// the typed structs drop. Ignored by `==`: equal content is equal however it
/// is laid out.
#[derive(Clone, Default)]
pub struct SourceLayout(Option<Arc<Source>>);

#[derive(Clone)]
struct Source {
    /// The text, editable; written back with the edit applied.
    doc: DocumentMut,
    /// The same text, untyped.
    text: toml::Table,
    /// What the typed content serialised to when `doc` was read. A path where
    /// the content still serialises to this was not edited.
    base: toml::Table,
}

impl PartialEq for SourceLayout {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl fmt::Debug for SourceLayout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self.0 {
            Some(_) => "SourceLayout(kept)",
            None => "SourceLayout(none)",
        })
    }
}

impl SourceLayout {
    /// Keep `text` (untyped: `untyped`) if it holds a key `file`, the typed
    /// read of it, does not model.
    pub(super) fn of(
        text: &str,
        untyped: &toml::Table,
        file: &LibraryFile,
    ) -> Result<Self, LibraryFileError> {
        if !has_unknown_keys(untyped, file)? {
            return Ok(Self::default());
        }
        Ok(Self(Some(Arc::new(Source {
            doc: parse_doc(text)?,
            text: untyped.clone(),
            base: toml::from_str(&file.write_canonical()?)?,
        }))))
    }

    /// The kept text with the edit from its read to `canonical` (the current
    /// content as [`LibraryFile::write_canonical`] writes it) applied, or
    /// `None` when no text is kept.
    pub(super) fn render(&self, canonical: &str) -> Result<Option<String>, LibraryFileError> {
        let Some(src) = &self.0 else {
            return Ok(None);
        };
        let new_doc = parse_doc(canonical)?;
        let new: toml::Table = toml::from_str(canonical)?;
        let mut doc = src.doc.clone();
        merge_table(
            doc.as_table_mut(),
            &src.text,
            Some(&src.base),
            &new,
            new_doc.as_table(),
            &mut Vec::new(),
        );
        Ok(Some(doc.to_string()))
    }

    /// Rename class `old` to `new` in the kept text, so the class keeps its
    /// unknown keys and its place.
    pub(super) fn rename_class(&mut self, old: &str, new: &str) {
        let Some(src) = self.0.as_mut() else {
            return;
        };
        let src = Arc::make_mut(src);
        let classes = src.doc.as_table_mut().get_mut("classes");
        if let Some(aot) = classes.and_then(Item::as_array_of_tables_mut) {
            for entry in aot.iter_mut() {
                if let Some(Item::Value(key)) = entry.get_mut("key")
                    && key.as_str() == Some(old)
                {
                    let decor = key.decor().clone();
                    *key = new.into();
                    *key.decor_mut() = decor;
                }
            }
        }
        rename_class_in(&mut src.text, old, new);
        rename_class_in(&mut src.base, old, new);
    }
}

fn parse_doc(text: &str) -> Result<DocumentMut, LibraryFileError> {
    text.parse().map_err(
        |e: toml_edit::TomlError| LibraryFileError::WrittenDocumentUnreadable {
            reason: e.to_string(),
        },
    )
}

fn rename_class_in(doc: &mut toml::Table, old: &str, new: &str) {
    let Some(Value::Array(classes)) = doc.get_mut("classes") else {
        return;
    };
    for entry in classes {
        if let Value::Table(t) = entry
            && t.get("key").and_then(Value::as_str) == Some(old)
        {
            t.insert("key".into(), Value::String(new.into()));
        }
    }
}

/// Whether `doc` (the untyped text) has a key the typed read `file` dropped:
/// in the header, in a `[tables.<name>]` block, or in a cached price record.
fn has_unknown_keys(doc: &toml::Table, file: &LibraryFile) -> Result<bool, LibraryFileError> {
    let header = Value::Table(toml::Table::try_from(&file.manifest)?);
    for (key, value) in doc.iter().filter(|(k, _)| *k != "tables") {
        if header
            .get(key)
            .is_none_or(|known| has_unknown(value, known))
        {
            return Ok(true);
        }
    }
    let Some(Value::Table(tables)) = doc.get("tables") else {
        return Ok(false);
    };
    for (name, body) in tables {
        let (Value::Table(body), Some(table)) = (body, file.tables.get(name)) else {
            continue;
        };
        let modelled = |k: &str| matches!(k, "tsv" | "column_types" | "pricing");
        if body.keys().any(|k| !modelled(k)) {
            return Ok(true);
        }
        for (row, distributor, sku, record) in price_records(body.get("pricing")) {
            let typed = table
                .pricing
                .get(row)
                .and_then(|d| d.get(distributor))
                .and_then(|s| s.get(sku));
            if let Some(typed) = typed
                && has_unknown(record, &Value::try_from(typed)?)
            {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// Every `(row, distributor, sku, record)` under a `pricing` value.
fn price_records(pricing: Option<&Value>) -> impl Iterator<Item = (&str, &str, &str, &Value)> {
    pricing.into_iter().flat_map(entries).flat_map(|(row, d)| {
        entries(d).flat_map(move |(dist, s)| {
            entries(s).map(move |(sku, rec)| (row.as_str(), dist.as_str(), sku.as_str(), rec))
        })
    })
}

/// The entries of `v` if it is a table; none otherwise.
fn entries(v: &Value) -> impl Iterator<Item = (&String, &Value)> {
    v.as_table().into_iter().flatten()
}

/// Whether `doc` has a key that `known` (the same data re-serialised from the
/// typed structs) lacks, at any depth. Arrays are compared element by element:
/// both come from one text, in one order.
fn has_unknown(doc: &Value, known: &Value) -> bool {
    match (doc, known) {
        (Value::Table(d), Value::Table(k)) => d
            .iter()
            .any(|(key, v)| k.get(key).is_none_or(|kv| has_unknown(v, kv))),
        (Value::Array(d), Value::Array(k)) => d.iter().zip(k).any(|(a, b)| has_unknown(a, b)),
        _ => false,
    }
}

/// Bring `dst` (kept text, untyped `src`) to `new` (current content, editable
/// `new_t`). `base` is what the content was when `dst` was read: a key in
/// `dst` that `base` and `new` both lack is unknown and stays; a key `base`
/// had and `new` lacks was removed by the edit and goes, with whatever unknown
/// keys sat under it; a key `new` adds is appended unless `base` already had
/// it unchanged (an omitted default).
fn merge_table(
    dst: &mut Table,
    src: &toml::Table,
    base: Option<&toml::Table>,
    new: &toml::Table,
    new_t: &Table,
    path: &mut Vec<String>,
) {
    let keys: Vec<String> = dst.iter().map(|(k, _)| k.to_owned()).collect();
    for key in keys {
        path.push(key.clone());
        let base_v = base.and_then(|b| b.get(&key));
        match (new.get(&key), new_t.get(&key), dst.get_mut(&key)) {
            (Some(nv), Some(ni), Some(d)) => merge_item(d, src.get(&key), base_v, nv, ni, path),
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
    if emptied {
        dst.set_implicit(false);
    }
    for (key, nv) in new {
        let unchanged_default = !emptied && base.and_then(|b| b.get(key)) == Some(nv);
        if dst.contains_key(key) || unchanged_default {
            continue;
        }
        if let Some((k, item)) = new_t.get_key_value(key) {
            dst.insert_formatted(k, detach(item));
        }
    }
}

fn merge_item(
    dst: &mut Item,
    src: Option<&Value>,
    base: Option<&Value>,
    new: &Value,
    new_item: &Item,
    path: &mut Vec<String>,
) {
    match (dst, new_item, src, new) {
        (Item::Table(d), Item::Table(n), Some(Value::Table(s)), Value::Table(nv)) => {
            merge_table(d, s, base.and_then(Value::as_table), nv, n, path);
        }
        (
            Item::ArrayOfTables(d),
            Item::ArrayOfTables(n),
            Some(Value::Array(s)),
            Value::Array(nv),
        ) if *path == ["classes"] => {
            let base = base.and_then(Value::as_array).map(Vec::as_slice);
            merge_classes(d, s, base, nv, n, path);
        }
        (dst, ..) => {
            // A string (the TSV block) is checked against the text itself, so
            // a row the reader repaired is written repaired.
            let unchanged = src == Some(new) || (!new.is_str() && base == Some(new));
            if !unchanged {
                replace(dst, new_item);
            }
        }
    }
}

/// `[[classes]]`, matched by class `key` so a removed or reordered class never
/// hands its unknown keys to another.
fn merge_classes(
    dst: &mut ArrayOfTables,
    src: &[Value],
    base: Option<&[Value]>,
    new: &[Value],
    new_aot: &ArrayOfTables,
    path: &mut Vec<String>,
) {
    let key_of = |v: &Value| v.get("key").and_then(Value::as_str).map(str::to_owned);
    let find = |list: &[Value], key: &Option<String>| {
        list.iter().position(|v| key.is_some() && key_of(v) == *key)
    };
    let mut out = ArrayOfTables::new();
    let mut kept_positions = Vec::new();
    for (nv, nt) in new.iter().zip(new_aot.iter()) {
        let key = key_of(nv);
        let kept = find(src, &key).and_then(|i| Some((dst.get(i)?, src[i].as_table()?)));
        match (kept, nv.as_table()) {
            (Some((t, s)), Some(nvt)) => {
                let b = base
                    .and_then(|b| find(b, &key).map(|j| &b[j]))
                    .and_then(Value::as_table);
                let mut t = t.clone();
                merge_table(&mut t, s, b, nvt, nt, path);
                kept_positions.push(t.position());
                out.push(t);
            }
            _ => out.push(detach_table(nt)),
        }
    }
    // Tables are written in position order: a reorder must not be undone by
    // the positions the kept entries had.
    if !kept_positions.is_sorted()
        && let Some(first) = kept_positions.iter().flatten().min().copied()
    {
        out.iter_mut().for_each(|t| t.set_position(first));
    }
    *dst = out;
}

/// Under `[tables.<name>.pricing]` the reader drops a row, distributor or
/// record it cannot use (it is fetched again); such a key is not kept.
fn reader_drops(path: &[String]) -> bool {
    matches!(path, [t, _, p, ..] if t == "tables" && p == "pricing") && path.len() <= 6
}

fn replace(dst: &mut Item, new_item: &Item) {
    match (dst, new_item) {
        (Item::Value(d), Item::Value(n)) => {
            let decor = d.decor().clone();
            *d = n.clone();
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
