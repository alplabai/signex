//! The board layer strip's per-user settings (#2394): how the tabs name
//! their layers and the layer sets the user saved. Persisted in `prefs.json`
//! under `pcb_layer_strip`; the board files never carry them.

use signex_types::format::{parse_pcb_layer, pcb_layer_token};
use signex_types::pcb_layer::PcbLayer;

/// How a layer tab names its layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LayerNameStyle {
    /// `TL`, `BL`, `M1`, `TO`.
    Short,
    /// `Top`, `Bottom`, `Mid 1`.
    Medium,
    /// The Layers panel's names: `Top Layer`, `Top Silkscreen`.
    #[default]
    Long,
}

impl LayerNameStyle {
    pub const ALL: [LayerNameStyle; 3] = [Self::Short, Self::Medium, Self::Long];

    /// The `prefs.json` and command-operand token.
    pub fn token(self) -> &'static str {
        match self {
            Self::Short => "short",
            Self::Medium => "medium",
            Self::Long => "long",
        }
    }

    /// The style `token` names, ignoring case.
    pub fn from_token(token: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|style| token.trim().eq_ignore_ascii_case(style.token()))
    }
}

/// How a board picks its active layer: Preferences ▸ Appearance ▸ Layer
/// switcher, saved in `prefs.json` as `pcb_layer_switcher`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LayerSwitcher {
    /// The layer tabs under the board; the status-bar pill is a label.
    #[default]
    Strip,
    /// No tabs; the status-bar pill opens a list of the visible layers.
    Dropdown,
}

impl LayerSwitcher {
    /// The value stored in `prefs.json`.
    pub fn as_pref(self) -> &'static str {
        match self {
            Self::Strip => "strip",
            Self::Dropdown => "dropdown",
        }
    }

    /// Parse a stored value; anything unknown is [`Self::Strip`].
    pub fn from_pref(value: &str) -> Self {
        match value {
            "dropdown" => Self::Dropdown,
            _ => Self::Strip,
        }
    }
}

/// A layer set the user saved: a name and its layers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedLayerSet {
    pub name: String,
    pub layers: Vec<PcbLayer>,
}

/// The strip's settings.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LayerStripPrefs {
    pub name_style: LayerNameStyle,
    /// Numbers in front of the names: copper by its place in the stack,
    /// a mechanical layer by its index.
    pub show_numbers: bool,
    /// In the order they were saved.
    pub saved_sets: Vec<SavedLayerSet>,
}

impl LayerStripPrefs {
    /// The saved set called `name`, compared without surrounding spaces.
    pub fn saved_set(&self, name: &str) -> Option<&SavedLayerSet> {
        let name = name.trim();
        self.saved_sets.iter().find(|set| set.name == name)
    }

    /// Save `layers` as `name`, replacing a set of the same name in place.
    /// Whether it was saved: a blank name is not.
    pub fn save_set(&mut self, name: &str, layers: Vec<PcbLayer>) -> bool {
        let name = name.trim();
        if name.is_empty() {
            return false;
        }
        let set = SavedLayerSet {
            name: name.to_string(),
            layers,
        };
        match self.saved_sets.iter_mut().find(|s| s.name == name) {
            Some(existing) => *existing = set,
            None => self.saved_sets.push(set),
        }
        true
    }

    /// Delete the set called `name`. Whether there was one.
    pub fn delete_set(&mut self, name: &str) -> bool {
        let name = name.trim();
        let before = self.saved_sets.len();
        self.saved_sets.retain(|set| set.name != name);
        self.saved_sets.len() != before
    }

    /// The `prefs.json` value.
    pub fn to_json(&self) -> serde_json::Value {
        let sets: Vec<serde_json::Value> = self
            .saved_sets
            .iter()
            .map(|set| {
                let layers: Vec<String> = set.layers.iter().map(|&l| pcb_layer_token(l)).collect();
                serde_json::json!({ "name": set.name, "layers": layers })
            })
            .collect();
        serde_json::json!({
            "name_style": self.name_style.token(),
            "show_numbers": self.show_numbers,
            "sets": sets,
        })
    }

    /// Read the `prefs.json` value. Anything missing or unknown reads as
    /// the default; a set with no name is dropped, a layer this build does
    /// not know is left out of its set.
    pub fn from_json(json: Option<&serde_json::Value>) -> Self {
        let Some(json) = json else {
            return Self::default();
        };
        let name_style = json["name_style"]
            .as_str()
            .and_then(LayerNameStyle::from_token)
            .unwrap_or_default();
        let mut prefs = Self {
            name_style,
            show_numbers: json["show_numbers"].as_bool().unwrap_or(false),
            saved_sets: Vec::new(),
        };
        for set in json["sets"].as_array().into_iter().flatten() {
            let layers = set["layers"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|token| parse_pcb_layer(token.as_str()?))
                .collect();
            if let Some(name) = set["name"].as_str() {
                prefs.save_set(name, layers);
            }
        }
        prefs
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::pcb_layer::{CopperLayer, TechnicalLayer};

    const TOP: PcbLayer = PcbLayer::Copper(CopperLayer::Top);
    const OVERLAY: PcbLayer = PcbLayer::Technical(TechnicalLayer::SilkscreenTop);

    #[test]
    fn the_settings_round_trip_through_json() {
        let mut prefs = LayerStripPrefs {
            name_style: LayerNameStyle::Short,
            show_numbers: true,
            saved_sets: Vec::new(),
        };
        assert!(prefs.save_set(" Assembly ", vec![TOP, OVERLAY]));
        assert!(
            !prefs.save_set("  ", vec![TOP]),
            "a blank name is not saved"
        );
        let json = prefs.to_json();
        assert_eq!(LayerStripPrefs::from_json(Some(&json)), prefs);
        assert_eq!(prefs.saved_set("Assembly").map(|s| s.layers.len()), Some(2));
    }

    #[test]
    fn saving_a_name_again_replaces_it_and_delete_removes_it() {
        let mut prefs = LayerStripPrefs::default();
        prefs.save_set("Copper", vec![TOP]);
        prefs.save_set("Copper", vec![TOP, OVERLAY]);
        assert_eq!(prefs.saved_sets.len(), 1);
        assert_eq!(prefs.saved_sets[0].layers, [TOP, OVERLAY]);
        assert!(prefs.delete_set("Copper"));
        assert!(!prefs.delete_set("Copper"));
    }

    #[test]
    fn missing_or_broken_values_read_as_the_defaults() {
        assert_eq!(LayerStripPrefs::from_json(None), LayerStripPrefs::default());
        let broken = serde_json::json!({
            "name_style": "tiny",
            "sets": [{ "layers": ["TopCopper"] }, { "name": "X", "layers": ["Nope", "TopCopper"] }]
        });
        let prefs = LayerStripPrefs::from_json(Some(&broken));
        assert_eq!(prefs.name_style, LayerNameStyle::Long);
        assert_eq!(prefs.saved_sets.len(), 1);
        assert_eq!(prefs.saved_sets[0].layers, [TOP]);
    }
}
