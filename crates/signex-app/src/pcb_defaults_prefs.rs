//! The PCB Defaults preference (#2868): the dimension defaults a new
//! project is seeded with — line width, text height, decimals, arrows,
//! text, extension lines and number format. Each project then keeps its
//! own (`ProjectDefaults::dimension`), which new dimensions start from; a
//! project without them, or a board in none, follows this preference.
//! Stored in `prefs.json` under [`KEY`]; edited as a draft in Preferences ▸
//! PCB Defaults (`app::handlers::preferences::pcb_defaults`) and committed
//! on Save.

use std::collections::BTreeMap;

use signex_types::pcb_dimension_style::DimensionDefaults;

use crate::panels::pcb_primitive_properties::PrimitiveField;

/// The `prefs.json` key of the defaults, one JSON object.
pub const KEY: &str = "pcb_dimension_defaults";

/// The defaults in `prefs.json`; missing or unreadable reads as the look
/// dimensions had before these settings.
pub fn from_prefs(json: Option<&serde_json::Value>) -> DimensionDefaults {
    json.and_then(|json| json.get(KEY))
        .and_then(|value| serde_json::from_value::<DimensionDefaults>(value.clone()).ok())
        .filter(DimensionDefaults::is_sound)
        .unwrap_or_default()
}

/// Write the defaults into a `prefs.json` object.
pub fn write_into(
    defaults: &DimensionDefaults,
    prefs: &mut serde_json::Map<String, serde_json::Value>,
) {
    if let Ok(value) = serde_json::to_value(defaults) {
        prefs.insert(KEY.to_string(), value);
    }
}

/// The Preferences draft of the defaults: the values, the text typed in
/// each number field until Save or Discard, and why Save would refuse a
/// field's text.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PcbDefaultsDraft {
    pub defaults: DimensionDefaults,
    pub texts: BTreeMap<PrimitiveField, String>,
    pub refused: BTreeMap<PrimitiveField, String>,
}

impl PcbDefaultsDraft {
    /// A draft of the saved `defaults`, nothing typed.
    pub fn of(defaults: &DimensionDefaults) -> Self {
        Self {
            defaults: defaults.clone(),
            ..Self::default()
        }
    }

    /// Whether Save would change the saved `defaults` or refuse a field.
    pub fn differs_from(&self, saved: &DimensionDefaults) -> bool {
        self.defaults != *saved || !self.refused.is_empty()
    }
}

#[cfg(test)]
#[path = "pcb_defaults_prefs_tests.rs"]
mod tests;
