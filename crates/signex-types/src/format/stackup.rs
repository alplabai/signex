//! The `[stackup]` section of `.snxpcb`: layer names, optional parallel
//! `layer_types` / `layer_ids` lists, and the optional keyed
//! `[stackup.display_names]` table (layer token -> display name).
//!
//! Layer tokens must be unique: two layers with one token are invalid, and
//! only the first reads the table's entry for that token.

use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Serialize};

use super::{FormatError, UnknownFields};
use crate::pcb::{LayerDef, PcbBoard, PcbSetup};

#[derive(Serialize)]
pub(super) struct StackBlock<'a> {
    layers: Vec<String>,
    /// Parallel to `layers`; omitted when every type is empty.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    layer_types: Vec<String>,
    /// Parallel to `layers`; omitted when every id equals its index.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    layer_ids: Vec<u8>,
    /// Layer token -> display name, only for layers that have one. Keyed, not
    /// parallel, so a build that reorders or drops layers cannot misalign it.
    /// Entries this build does not own (a token naming no layer, a value that
    /// is not a string) ride along unchanged.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    display_names: BTreeMap<String, toml::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    setup: Option<&'a PcbSetup>,
    #[serde(flatten)]
    unknown: &'a UnknownFields,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(super) struct StackRaw {
    #[serde(default)]
    layers: Vec<String>,
    /// Parallel to `layers`; absent in 1.0 files.
    #[serde(default)]
    layer_types: Vec<String>,
    /// Parallel to `layers`; absent when every id equals its index.
    #[serde(default)]
    layer_ids: Vec<u8>,
    /// Absent in files that predate display names.
    #[serde(default)]
    display_names: BTreeMap<String, toml::Value>,
    #[serde(default)]
    setup: Option<PcbSetup>,
    /// Foreign keys inside `[stackup]`.
    #[serde(flatten)]
    unknown: UnknownFields,
}

pub(super) fn block(board: &PcbBoard) -> StackBlock<'_> {
    let layers = &board.layers;
    StackBlock {
        layers: layers.iter().map(|l| l.name.clone()).collect(),
        layer_types: if layers.iter().any(|l| !l.layer_type.is_empty()) {
            layers.iter().map(|l| l.layer_type.clone()).collect()
        } else {
            Vec::new()
        },
        layer_ids: if layers
            .iter()
            .enumerate()
            .any(|(i, l)| usize::from(l.id) != i)
        {
            layers.iter().map(|l| l.id).collect()
        } else {
            Vec::new()
        },
        display_names: display_names(board),
        setup: board.setup.as_ref(),
        unknown: &board.unknown.stackup,
    }
}

/// Kept entries first, then the layers' own names, which win on a clash.
fn display_names(board: &PcbBoard) -> BTreeMap<String, toml::Value> {
    let mut names: BTreeMap<String, toml::Value> = board
        .unknown
        .stackup_display_names
        .entries()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    for layer in board.layers.iter().filter(|l| !l.display_name.is_empty()) {
        names.insert(
            layer.name.clone(),
            toml::Value::String(layer.display_name.clone()),
        );
    }
    names
}

/// What `[stackup]` holds once read.
pub(super) struct StackRead {
    pub layers: Vec<LayerDef>,
    pub setup: Option<PcbSetup>,
    pub unknown: UnknownFields,
    /// `display_names` entries no layer owns, kept for the next save.
    pub display_names: UnknownFields,
}

/// Rebuild the layer list, the setup and the foreign `[stackup]` keys.
pub(super) fn read(s: StackRaw) -> Result<StackRead, FormatError> {
    if s.layer_types.len() > s.layers.len() {
        return Err(FormatError::LayerTypesExceedLayers {
            layers: s.layers.len(),
            types: s.layer_types.len(),
        });
    }
    let n_layers = s.layers.len();
    // An absent list means positional ids; a present one must match the layer
    // count and be duplicate-free, or the ids would be silently dropped on
    // the next save.
    if !s.layer_ids.is_empty() {
        if s.layer_ids.len() != n_layers {
            return Err(FormatError::LayerIdsLengthMismatch {
                layers: n_layers,
                ids: s.layer_ids.len(),
            });
        }
        let mut seen = HashSet::new();
        if let Some(&id) = s.layer_ids.iter().find(|id| !seen.insert(**id)) {
            return Err(FormatError::DuplicateLayerId { id });
        }
    }
    let mut display_names = s.display_names;
    let layers = s
        .layers
        .into_iter()
        .enumerate()
        .map(|(i, name)| LayerDef {
            // Validated above: empty means positional.
            id: s.layer_ids.get(i).copied().unwrap_or(i as u8),
            layer_type: s.layer_types.get(i).cloned().unwrap_or_default(),
            display_name: take_name(&mut display_names, &name),
            name,
        })
        .collect();
    let mut kept = UnknownFields::default();
    for (token, value) in display_names {
        kept.insert_value(&token, value);
    }
    Ok(StackRead {
        layers,
        setup: s.setup,
        unknown: s.unknown,
        display_names: kept,
    })
}

/// The string display name stored for `token`; any other value stays put.
fn take_name(names: &mut BTreeMap<String, toml::Value>, token: &str) -> String {
    match names.get(token) {
        Some(toml::Value::String(name)) => {
            let name = name.clone();
            names.remove(token);
            name
        }
        _ => String::new(),
    }
}
