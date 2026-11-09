//! The sketch parameter table: `name → source-string`. Resolving the
//! table (parsing and evaluating the expressions) lives in
//! `signex_sketch::parameter`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// User-defined parameter table — `name → source-string`. Source
/// strings carry an optional `=` prefix (Altium-style) and are
/// otherwise the same expression-language input that the parser
/// accepts.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ParameterTable(pub BTreeMap<String, String>);

impl ParameterTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert / overwrite a parameter source string.
    pub fn insert(&mut self, name: impl Into<String>, src: impl Into<String>) {
        self.0.insert(name.into(), src.into());
    }

    pub fn get_raw(&self, name: &str) -> Option<&str> {
        self.0.get(name).map(String::as_str)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &String)> {
        self.0.iter()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
