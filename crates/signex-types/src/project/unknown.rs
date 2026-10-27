//! Forward-compatible capture of JSON keys this build does not recognise in
//! a `.snxprj` (#788).
//!
//! The `.snxsch` / `.snxpcb` TOML formats use
//! [`UnknownFields`](crate::format::UnknownFields) (raw TOML values) for the
//! same job. `.snxprj` is JSON, which has `null` — a value TOML cannot
//! represent — so it needs its own bag of [`serde_json::Value`]s.
//!
//! Placed on a struct as `#[serde(flatten)]`, serde routes every key the
//! struct does not declare into the bag and writes it back beside the known
//! fields on save, so `load -> save` keeps keys written by a newer build.
//! `BTreeMap` keeps the key order deterministic so saves stay diff-stable.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Keys a build does not recognise, keyed by name, values kept as raw JSON.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UnknownJson(BTreeMap<String, Value>);

impl UnknownJson {
    /// `true` when no foreign keys were captured.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Read-only view of a captured key.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.0.get(key)
    }

    /// Take every key of `other` this bag lacks, so a replacement section can
    /// keep what the section it replaces held from a newer build.
    pub fn fill_missing_from(&mut self, other: &UnknownJson) {
        for (key, value) in &other.0 {
            self.0.entry(key.clone()).or_insert_with(|| value.clone());
        }
    }

    /// Keep `value` under `key`, for a known key whose value this build
    /// cannot interpret.
    pub(crate) fn insert(&mut self, key: String, value: Value) {
        self.0.insert(key, value);
    }

    /// Drop a captured key, for a section an explicit edit now replaces.
    pub(crate) fn remove(&mut self, key: &str) {
        self.0.remove(key);
    }
}
