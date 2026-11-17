//! Board layer strip preference IO (#2394): the tab name style and the
//! saved layer sets, one `prefs.json` key.

use super::*;
use crate::layer_strip_prefs::LayerStripPrefs;

/// The `prefs.json` key of the layer strip's settings.
pub const LAYER_STRIP_PREF_KEY: &str = "pcb_layer_strip";

/// Read the layer strip's settings; a missing or broken value reads as
/// the defaults.
pub fn read_layer_strip_pref_from(json: Option<&serde_json::Value>) -> LayerStripPrefs {
    LayerStripPrefs::from_json(json.map(|json| &json[LAYER_STRIP_PREF_KEY]))
}

/// Persist the layer strip's settings without clobbering other keys.
pub fn write_layer_strip_pref(prefs: &LayerStripPrefs) {
    write_layer_strip_pref_at(&prefs_path(), prefs)
}

pub fn write_layer_strip_pref_at(path: &Path, prefs: &LayerStripPrefs) {
    let value = prefs.to_json();
    update_prefs_json(path, LAYER_STRIP_PREF_KEY, |map| {
        map.insert(LAYER_STRIP_PREF_KEY.to_string(), value);
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layer_strip_prefs::LayerNameStyle;

    /// What is written is what the next launch reads.
    #[test]
    fn the_layer_strip_settings_survive_a_write_and_a_read() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let path = dir.path().join("prefs.json");
        let mut prefs = LayerStripPrefs {
            name_style: LayerNameStyle::Medium,
            ..LayerStripPrefs::default()
        };
        prefs.save_set("Signal only", Vec::new());
        write_layer_strip_pref_at(&path, &prefs);
        let json = read_prefs_json(&path);
        assert_eq!(read_layer_strip_pref_from(json.as_ref()), prefs);
    }
}
