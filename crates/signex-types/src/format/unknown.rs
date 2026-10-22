//! Forward-compatible capture of TOML keys this build does not recognise.
//!
//! A `.snxsch` / `.snxpcb` written by a newer Signex may carry keys the
//! current schema has never heard of. Without a catch-all, serde drops
//! them at parse time and the next save silently destroys them.
//! [`UnknownFields`] is that catch-all.
//!
//! # Contract
//!
//! - **Populated** by placing it on a container struct as
//!   `#[serde(flatten)] unknown: UnknownFields`. Serde routes every key
//!   the struct does not declare into the bag — scalars and nested
//!   tables alike.
//! - **Re-emitted verbatim** on save: the same flatten attribute makes
//!   serde write the bag's entries back out beside the known fields, so
//!   `load -> save` is lossless for foreign keys. `BTreeMap` keeps key
//!   order deterministic, so saves stay diff-stable.
//! - **Consulted by write-elision**: any `is_default()`-style check that
//!   drops an all-default entry must also require
//!   [`UnknownFields::is_empty`], otherwise an entry holding only
//!   foreign keys would be elided and the keys lost.
//!
//! Model types that carry a bag (`SchematicSheet`, `PcbBoard`, ...)
//! annotate the field with
//! `#[serde(default, skip_serializing_if = "UnknownFields::is_empty")]`
//! so other serialisations (JSON snapshots) are unchanged when it is empty.

use std::collections::BTreeMap;
use std::ops::Deref;

use serde::{Deserialize, Serialize};

/// Keys a build does not recognise, keyed by name, values kept as raw TOML.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UnknownFields(BTreeMap<String, toml::Value>);

impl UnknownFields {
    /// `true` when no foreign keys were captured.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Foreign keys carried by a [`SchematicSheet`](crate::schematic::SchematicSheet),
/// kept apart by the `.snxsch` section they were found in.
///
/// One flat bag per sheet would let a manifest key and an
/// `[extras.sheet]` key of the same name overwrite each other, so each
/// section owns its own bag and is re-emitted into the section it came
/// from. (Per-symbol foreign keys live on the symbol itself.)
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SheetUnknown {
    /// Top-level manifest keys (beside `format`, `schematic_id`, ...).
    #[serde(default, skip_serializing_if = "UnknownFields::is_empty")]
    pub manifest: UnknownFields,
    /// Keys inside `[extras.sheet]`.
    #[serde(default, skip_serializing_if = "UnknownFields::is_empty")]
    pub extras: UnknownFields,
}

impl SheetUnknown {
    /// `true` when no section holds a foreign key.
    pub fn is_empty(&self) -> bool {
        self.manifest.is_empty() && self.extras.is_empty()
    }
}

/// Foreign keys carried by a [`PcbBoard`](crate::pcb::PcbBoard), kept
/// apart by the `.snxpcb` section they were found in, for the same
/// reason as [`SheetUnknown`]: a manifest key and a `[stackup]` key of
/// the same name must not overwrite each other. Per-footprint and
/// per-pad foreign keys live on the `Footprint` / `Pad` themselves.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BoardUnknown {
    /// Top-level manifest keys (beside `format`, `pcb_id`, ...).
    #[serde(default, skip_serializing_if = "UnknownFields::is_empty")]
    pub manifest: UnknownFields,
    /// Keys inside `[stackup]`.
    #[serde(default, skip_serializing_if = "UnknownFields::is_empty")]
    pub stackup: UnknownFields,
    /// Keys inside `[nets]`.
    #[serde(default, skip_serializing_if = "UnknownFields::is_empty")]
    pub nets: UnknownFields,
    /// Keys inside `[extras.board]`.
    #[serde(default, skip_serializing_if = "UnknownFields::is_empty")]
    pub extras: UnknownFields,
}

impl BoardUnknown {
    /// `true` when no section holds a foreign key.
    pub fn is_empty(&self) -> bool {
        self.manifest.is_empty()
            && self.stackup.is_empty()
            && self.nets.is_empty()
            && self.extras.is_empty()
    }
}

impl Deref for UnknownFields {
    type Target = BTreeMap<String, toml::Value>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Serialize, Deserialize)]
    struct Probe {
        known: String,
        #[serde(flatten)]
        unknown: UnknownFields,
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct Inner {
        a: u32,
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct WithTable {
        name: String,
        inner: Inner,
        #[serde(flatten)]
        unknown: UnknownFields,
    }

    #[test]
    fn unknown_scalar_survives() {
        let src = "known = \"x\"\nfuture_flag = true\nfuture_n = 7\n";
        let p: Probe = toml::from_str(src).unwrap();
        assert_eq!(p.unknown.len(), 2);
        let out = toml::to_string_pretty(&p).unwrap();
        let again: Probe = toml::from_str(&out).unwrap();
        assert_eq!(again.unknown, p.unknown);
        assert!(out.contains("future_flag = true"));
    }

    #[test]
    fn unknown_nested_table_survives() {
        let src = "known = \"x\"\n[vendor]\nkey = \"v\"\n[vendor.deep]\nn = 1\n";
        let p: Probe = toml::from_str(src).unwrap();
        let out = toml::to_string_pretty(&p).unwrap();
        let again: Probe = toml::from_str(&out).unwrap();
        assert_eq!(again.unknown, p.unknown);
        assert!(again.unknown.contains_key("vendor"));
    }

    #[test]
    fn table_valued_known_field_plus_unknowns_reparses() {
        let src = "name = \"n\"\nscalar = 3\n[inner]\na = 1\n[extra_table]\nz = \"q\"\n";
        let p: WithTable = toml::from_str(src).unwrap();
        assert_eq!(p.inner.a, 1);
        let out = toml::to_string_pretty(&p).unwrap();
        let again: WithTable = toml::from_str(&out).unwrap();
        assert_eq!(again.unknown, p.unknown);
        assert_eq!(again.unknown.len(), 2);
    }

    #[test]
    fn empty_bag_emits_nothing() {
        let p = Probe {
            known: "x".into(),
            unknown: UnknownFields::default(),
        };
        assert!(p.unknown.is_empty());
        assert_eq!(toml::to_string_pretty(&p).unwrap(), "known = \"x\"\n");
    }
}
