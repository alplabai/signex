//! Preference-backed defaults a newly opened or created library starts from.
//!
//! Held in the model so a library opened right after Save Preferences sees
//! the committed values, not whatever the background `prefs.json` write has
//! reached so far (#1351). The shipped `Default` lives in
//! `library/state/new_library_defaults.rs`.

use crate::app::core::component_class::ComponentClassEntry;
use crate::app::core::pin_selection_mode::PinSelectionMode;

/// `symbol_grid_size_mm`, `symbol_pin_selection` and `component_classes` as
/// last loaded or saved.
#[derive(Debug, Clone)]
pub struct NewLibraryDefaults {
    pub grid_size_mm: f32,
    pub pin_selection: PinSelectionMode,
    pub component_classes: Vec<ComponentClassEntry>,
}
