use serde::{Deserialize, Serialize};

use crate::schematic::{Point, TextProp};

/// Captures KiCad property metadata without forcing all callers off the legacy
/// key/value field map at once.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SchematicProperty {
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub id: Option<u32>,
    #[serde(default)]
    pub text: Option<TextProp>,
    #[serde(default)]
    pub show_name: Option<bool>,
    #[serde(default)]
    pub do_not_autoplace: Option<bool>,
}

/// Captures KiCad footprint property metadata while preserving the legacy
/// footprint `reference` and `value` compatibility fields.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PcbProperty {
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub position: Option<Point>,
    #[serde(default)]
    pub rotation: f64,
    #[serde(default)]
    pub layer: Option<String>,
    #[serde(default)]
    pub font_size: Option<f64>,
    #[serde(default)]
    pub hidden: bool,
}