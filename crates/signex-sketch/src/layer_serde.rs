//! Serde for the `layer` attribute of a sketch marker.
//!
//! Writes and reads only the board-layer token
//! ([`signex_types::format::pcb_layer_token`], e.g. `TopSilkscreen`).
//! Anything else, including a retired spelling such as `top_silk` or an
//! index outside the layer range such as `Mechanical33`, is a read error
//! naming the attribute and the bad value.

use serde::de::Error;
use serde::{Deserialize, Deserializer, Serializer};
use signex_types::format::{parse_pcb_layer, pcb_layer_token};
use signex_types::pcb_layer::PcbLayer;

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
