//! The `[stackup]` section of `.snxpcb`: layer names, optional parallel
//! `layer_types` / `layer_ids` lists, and the optional keyed
//! `[stackup.display_names]` table (layer token -> display name).

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
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    display_names: BTreeMap<&'a str, &'a str>,
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
    display_names: BTreeMap<String, String>,
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
        display_names: layers
            .iter()
            .filter(|l| !l.display_name.is_empty())
            .map(|l| (l.name.as_str(), l.display_name.as_str()))
            .collect(),
        setup: board.setup.as_ref(),
        unknown: &board.unknown.stackup,
    }
}

/// Rebuild the layer list, the setup and the foreign `[stackup]` keys.
pub(super) fn read(
    s: StackRaw,
) -> Result<(Vec<LayerDef>, Option<PcbSetup>, UnknownFields), FormatError> {
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
            display_name: display_names.remove(&name).unwrap_or_default(),
            name,
        })
        .collect();
    Ok((layers, s.setup, s.unknown))
}
