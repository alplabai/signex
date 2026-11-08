//! Versioned embedded library symbols (#2209).
//!
//! `SchematicSheet::lib_symbols` holds one entry per distinct *version* of a
//! library symbol, so a sheet can draw an old and a new body of one part side
//! by side. The key is a **body ref**, `"<lib_id>#<8 hex>"`; a placed
//! [`Symbol`] names the body it draws with in `Symbol::body_ref`, while
//! `Symbol::lib_id` stays the identity of the library symbol.
//!
//! # Hash spec
//!
//! The 8 hex digits are FNV-1a (32-bit, offset basis `0x811c9dc5`, prime
//! `0x01000193`) over the canonical text of the body:
//!
//! * the body serialised with serde to a `serde_json::Value` tree;
//! * objects written with their keys sorted bytewise, no whitespace;
//! * integers and strings as JSON; a float as a fixed 9-decimal number with
//!   `-0` written as `0`, so no platform or shortest-repr difference counts;
//! * arrays in their order (graphics and pins are ordered by the library).
//!
//! No `HashMap` iteration order enters the text. The ref is an identity, not
//! a checksum: two different bodies under one ref are a detected error
//! ([`BodyRefCollision`]), never a silent merge.
//!
//! An embedded body without a `body_ref` on its instances is drawn by
//! `lib_id` (see [`Symbol::body_key`]); that is what a symbol built in code
//! without a library behind it uses.

use super::{LibSymbol, SchematicSheet, Symbol};

/// FNV-1a 32-bit over `bytes`.
fn fnv1a32(bytes: &[u8]) -> u32 {
    bytes.iter().fold(0x811c_9dc5_u32, |h, b| {
        (h ^ u32::from(*b)).wrapping_mul(0x0100_0193)
    })
}

/// Append the canonical text of `value` (see the module doc) to `out`.
fn canonical(value: &serde_json::Value, out: &mut String) {
    use serde_json::Value;
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            out.push('{');
            for (i, key) in keys.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&Value::String(key.clone()).to_string());
                out.push(':');
                canonical(&map[key], out);
            }
            out.push('}');
        }
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                canonical(item, out);
            }
            out.push(']');
        }
        Value::Number(n) if n.is_f64() => {
            let f = n.as_f64().unwrap_or(0.0);
            let f = if f == 0.0 { 0.0 } else { f };
            out.push_str(&format!("{f:.9}"));
        }
        other => out.push_str(&other.to_string()),
    }
}

impl LibSymbol {
    /// The 8 hex digits that tell this body from other versions of the same
    /// library symbol. See the module doc for the spec.
    pub fn content_hash(&self) -> String {
        let value = serde_json::to_value(self).unwrap_or(serde_json::Value::Null);
        let mut text = String::new();
        canonical(&value, &mut text);
        format!("{:08x}", fnv1a32(text.as_bytes()))
    }

    /// The key this body is embedded under: `"<lib_id>#<hash>"`.
    pub fn body_ref(&self) -> String {
        format!("{}#{}", self.id, self.content_hash())
    }
}

/// The library symbol id a body ref (or a plain lib id) belongs to.
pub fn lib_id_of_body_ref(body_ref: &str) -> &str {
    body_ref
        .rsplit_once('#')
        .map_or(body_ref, |(lib_id, _)| lib_id)
}

/// Two different bodies were registered under one body ref.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BodyRefCollision {
    pub body_ref: String,
}

impl std::fmt::Display for BodyRefCollision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "two different library symbol bodies share the version id {}",
            self.body_ref
        )
    }
}

impl std::error::Error for BodyRefCollision {}

impl Symbol {
    /// The key of the embedded body this symbol draws with: its `body_ref`,
    /// or its `lib_id` when it names none.
    pub fn body_key(&self) -> &str {
        if self.body_ref.is_empty() {
            &self.lib_id
        } else {
            &self.body_ref
        }
    }
}

impl SchematicSheet {
    /// The embedded body `symbol` draws with.
    pub fn body_of(&self, symbol: &Symbol) -> Option<&LibSymbol> {
        self.lib_symbols.get(symbol.body_key())
    }

    /// Embed `body` under its own body ref and return the ref. An identical
    /// body already there is reused; a different one under the same ref is a
    /// [`BodyRefCollision`].
    pub fn embed_body(&mut self, body: LibSymbol) -> Result<String, BodyRefCollision> {
        let key = body.body_ref();
        match self.lib_symbols.get(&key) {
            Some(existing) if *existing == body => {}
            Some(_) => return Err(BodyRefCollision { body_ref: key }),
            None => {
                self.lib_symbols.insert(key.clone(), body);
            }
        }
        Ok(key)
    }

    /// One-time upgrade of a sheet saved before versioned bodies (#2209): a
    /// body keyed by plain `lib_id` is re-keyed to its body ref and the
    /// instances that drew with it name that ref.
    ///
    /// This is the single explicit exception to the no-legacy rule. It is
    /// removable: delete this function and its call in the sheet decoder once
    /// no old-form file is expected, and files without `body_ref` then draw
    /// by `lib_id` like a symbol built in code.
    pub fn upgrade_unversioned_bodies(&mut self) {
        let old: Vec<String> = self
            .lib_symbols
            .iter()
            .filter(|(key, body)| **key == body.id)
            .map(|(key, _)| key.clone())
            .collect();
        for key in old {
            let Some(body) = self.lib_symbols.remove(&key) else {
                continue;
            };
            let new_key = body.body_ref();
            self.lib_symbols.insert(new_key.clone(), body);
            for sym in self.symbols.iter_mut().filter(|s| s.body_ref.is_empty()) {
                if sym.lib_id == key {
                    sym.body_ref = new_key.clone();
                }
            }
        }
    }
}
