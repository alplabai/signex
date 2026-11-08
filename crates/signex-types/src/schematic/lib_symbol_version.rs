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
    /// The canonical text the hash is taken over (see the module doc). Two
    /// bodies are the same version exactly when this text is equal; unlike
    /// `==`, a float difference below the ninth decimal and a NaN do not
    /// make a body differ from itself.
    pub fn canonical_text(&self) -> String {
        let value = serde_json::to_value(self).unwrap_or(serde_json::Value::Null);
        let mut text = String::new();
        canonical(&value, &mut text);
        text
    }

    /// Whether `other` is the same version of the body: equal canonical text.
    pub fn same_version(&self, other: &Self) -> bool {
        self.canonical_text() == other.canonical_text()
    }

    /// The 8 hex digits that tell this body from other versions of the same
    /// library symbol. See the module doc for the spec.
    pub fn content_hash(&self) -> String {
        format!("{:08x}", fnv1a32(self.canonical_text().as_bytes()))
    }

    /// The key this body is embedded under: `"<lib_id>#<hash>"`.
    pub fn body_ref(&self) -> String {
        format!("{}#{}", self.id, self.content_hash())
    }
}

/// The library symbol id a body ref (or a plain lib id) belongs to. Only a
/// suffix of exactly 8 hex digits after the last `#` is a hash; any other
/// `#` belongs to the id.
pub fn lib_id_of_body_ref(body_ref: &str) -> &str {
    match body_ref.rsplit_once('#') {
        Some((lib_id, hash)) if hash.len() == 8 && hash.bytes().all(|b| b.is_ascii_hexdigit()) => {
            lib_id
        }
        _ => body_ref,
    }
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

    /// The ref `body` would be embedded under, or a [`BodyRefCollision`] when
    /// a different body already holds it. Changes nothing, so a caller can
    /// refuse before it rewrites anything.
    pub fn check_body(&self, body: &LibSymbol) -> Result<String, BodyRefCollision> {
        let key = body.body_ref();
        match self.lib_symbols.get(&key) {
            Some(existing) if !existing.same_version(body) => {
                Err(BodyRefCollision { body_ref: key })
            }
            _ => Ok(key),
        }
    }

    /// Embed `body` under its own body ref and return the ref. An identical
    /// body already there is reused; a different one under the same ref is a
    /// [`BodyRefCollision`].
    pub fn embed_body(&mut self, body: LibSymbol) -> Result<String, BodyRefCollision> {
        let key = self.check_body(&body)?;
        self.lib_symbols.entry(key.clone()).or_insert(body);
        Ok(key)
    }

    /// The designators of the parts whose units draw with different body
    /// versions, in sheet order. Such a part reads its pins and reference
    /// letters per unit; Update from library brings the units together.
    pub fn mixed_version_parts(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for part in super::group_into_parts(self.symbols.iter().map(Symbol::part_key)) {
            let mut keys = part.iter().map(|&i| self.symbols[i].body_key());
            let Some(first) = keys.next() else { continue };
            if keys.any(|k| k != first) {
                let reference = self.symbols[part[0]].reference.clone();
                if !out.contains(&reference) {
                    out.push(reference);
                }
            }
        }
        out
    }

    /// Put every embedded body under the key its content gives (its body
    /// ref) and point the instances that drew with the old key at the new
    /// one (#2209).
    ///
    /// Two jobs in one pass. It is the one-time upgrade of a sheet saved
    /// before versioned bodies, where a body is keyed by plain `lib_id` and
    /// the instances name no ref -- the single explicit exception to the
    /// no-legacy rule, removable once no such file is expected. And it heals
    /// a stored key that no longer matches its body, which a later change to
    /// the serde shape of `LibSymbol` would otherwise leave dangling.
    ///
    /// A body whose proper key is taken by a *different* body keeps its old
    /// key; nothing is overwritten.
    pub fn upgrade_unversioned_bodies(&mut self) {
        let stale: Vec<String> = self
            .lib_symbols
            .iter()
            .filter(|(key, body)| **key != body.body_ref())
            .map(|(key, _)| key.clone())
            .collect();
        for key in stale {
            let Some(body) = self.lib_symbols.get(&key) else {
                continue;
            };
            let new_key = body.body_ref();
            if let Some(taken) = self.lib_symbols.get(&new_key)
                && !taken.same_version(body)
            {
                continue;
            }
            let Some(body) = self.lib_symbols.remove(&key) else {
                continue;
            };
            self.lib_symbols.entry(new_key.clone()).or_insert(body);
            for sym in self.symbols.iter_mut().filter(|s| s.body_key() == key) {
                sym.body_ref = new_key.clone();
            }
        }
    }
}
