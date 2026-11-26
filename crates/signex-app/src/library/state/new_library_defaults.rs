//! The shipped [`NewLibraryDefaults`], read through the `fonts` preference
//! readers. The type itself lives in `app::core::new_library_defaults`.

use crate::app::core::new_library_defaults::NewLibraryDefaults;

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
