//! Serde for the `layer` attribute of a sketch marker.
//!
//! Writes and reads only the board-layer token
//! ([`crate::format::pcb_layer_token`], e.g. `TopSilkscreen`).
//! Anything else, including a retired spelling such as `top_silk` or an
//! index outside the layer range such as `Mechanical33`, is a read error
//! naming the attribute and the bad value.

use crate::format::{parse_pcb_layer, pcb_layer_token};
use crate::pcb_layer::PcbLayer;
use serde::de::Error;
use serde::{Deserialize, Deserializer, Serializer};

pub fn serialize<S: Serializer>(layer: &PcbLayer, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&pcb_layer_token(*layer))
}

pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<PcbLayer, D::Error> {
    // Any value is accepted here so the error can show what was written.
    let value = serde_json::Value::deserialize(d)?;
    value
        .as_str()
        .and_then(parse_pcb_layer)
        .ok_or_else(|| D::Error::custom(format!("layer attribute: `{value}` is not a board layer")))
}

/// The same tokens for a list of layers.
pub mod list {
    use super::*;
    use serde::ser::SerializeSeq;

    pub fn serialize<S: Serializer>(layers: &[PcbLayer], s: S) -> Result<S::Ok, S::Error> {
        let mut seq = s.serialize_seq(Some(layers.len()))?;
        for layer in layers {
            seq.serialize_element(&pcb_layer_token(*layer))?;
        }
        seq.end()
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<PcbLayer>, D::Error> {
        Vec::<serde_json::Value>::deserialize(d)?
            .into_iter()
            .map(|value| {
                value.as_str().and_then(parse_pcb_layer).ok_or_else(|| {
                    D::Error::custom(format!("layers: `{value}` is not a board layer"))
                })
            })
            .collect()
    }
}
