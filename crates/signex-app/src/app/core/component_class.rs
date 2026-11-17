//! One entry in the user's component-class list.

/// One entry in the user's component-class list. Persisted as a JSON
/// object `{ "key": "...", "label": "..." }` inside the
/// `component_classes` array in `prefs.json`. `key` is the canonical
/// machine identifier stored on `ComponentRow.class`; `label` is the
/// human-readable name surfaced in pickers.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ComponentClassEntry {
    pub key: String,
    pub label: String,
}
