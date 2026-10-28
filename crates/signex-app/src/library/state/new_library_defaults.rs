//! Preference-backed defaults a newly opened or created library starts from.
//!
//! Held in the model so a library opened right after Save Preferences sees
//! the committed values, not whatever the background `prefs.json` write has
//! reached so far (#1351).

use crate::fonts::ComponentClassEntry;
use crate::render_config::PinSelectionMode;

/// `symbol_grid_size_mm`, `symbol_pin_selection` and `component_classes` as
/// last loaded or saved.
#[derive(Debug, Clone)]
pub struct NewLibraryDefaults {
    pub grid_size_mm: f32,
    pub pin_selection: PinSelectionMode,
    pub component_classes: Vec<ComponentClassEntry>,
}

impl Default for NewLibraryDefaults {
    /// The shipped defaults, with no disk read. Bootstrap replaces them with
    /// the values it already parsed from `prefs.json`, and `Save` keeps them
    /// current afterwards.
    fn default() -> Self {
        Self {
            grid_size_mm: crate::fonts::read_symbol_grid_size_mm_pref_from(None),
            pin_selection: crate::fonts::read_symbol_pin_selection_pref_from(None),
            component_classes: crate::fonts::read_component_classes_pref_from(None),
        }
    }
}
