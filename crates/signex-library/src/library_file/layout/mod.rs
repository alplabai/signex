//! A `.snxlib` that holds keys this build does not model keeps the text it
//! was read from (#1435). A write edits that text in place, changing only what
//! the typed content changed since the read, so the unknown keys, the key
//! order and every untouched row come back byte-for-byte.

use std::fmt;
use std::sync::Arc;

use toml::Value;
use toml_edit::{DocumentMut, Item};

use super::{LibraryFile, LibraryFileError};

mod kept;
mod merge;

pub(super) use kept::lost_unknown_key;

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
        // clones the whole kept document per write; edit in place
        // under `&mut self` if large libraries make this show up.
        let mut doc = src.doc.clone();
        merge::merge_table(
            doc.as_table_mut(),
            &src.text,
            Some(&src.base),
            &new,
            new_doc.as_table(),
            &mut Vec::new(),
        );
        let out = doc.to_string();
        let written: toml::Table = toml::from_str(&out)?;
        if let Some(path) = lost_unknown_key(&src.text, &src.base, &new, &written) {
            return Err(LibraryFileError::WrittenDocumentDiffers {
                table: format!("(unknown key {path})"),
            });
        }
        Ok(Some(out))
    }

    /// Rename table `old` to `new` in the kept text, so the table keeps its
    /// unknown keys and its place.
    pub(super) fn rename_table(&mut self, old: &str, new: &str) {
        let Some(src) = self.0.as_mut() else {
            return;
        };
        let src = Arc::make_mut(src);
        let tables = src.doc.as_table_mut().get_mut("tables");
        if let Some(tables) = tables.and_then(Item::as_table_like_mut)
            && let Some(body) = tables.remove(old)
        {
            tables.insert(new, body);
        }
        for doc in [&mut src.text, &mut src.base] {
            if let Some(Value::Table(tables)) = doc.get_mut("tables")
                && let Some(body) = tables.remove(old)
            {
                tables.insert(new.into(), body);
            }
        }
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
        if let Some(Item::Value(toml_edit::Value::Array(list))) =
            src.doc.as_table_mut().get_mut("classes")
        {
            for entry in list.iter_mut() {
                if let Some(table) = entry.as_inline_table_mut()
                    && let Some(key) = table.get_mut("key")
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
