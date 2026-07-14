//! Keyboard shortcut profile model and runtime lookup.
//!
//! The shape follows the useful parts of Zed's keymap architecture while
//! staying native to Signex: TOML profiles, EDA-oriented built-ins, stable
//! command ids, context-aware lookup, and editor-friendly conflict reporting.

mod binding;
mod catalog;
mod command;
mod editor;
mod profile;

pub use binding::{
    KeyBindingSource, KeyParseError, KeyStroke, KeyToken, Modifiers, ShortcutBinding,
    ShortcutBindingAction, ShortcutContext, ShortcutTrigger,
};
pub use catalog::{CommandGroup, CommandMetadata, fallback_label, metadata_for};
pub use command::AppCommandId;
pub use editor::{KeymapEditorModel, KeymapEditorProfile, KeymapEditorRow, KeymapEditorSource};
pub use profile::{
    BindingConflict, BuiltInProfile, CompiledKeymap, KeyLookup, ProfileLoadError, ShortcutProfile,
    ShortcutProfileKind, ShortcutProfileSet, TomlShortcutProfile, config_path, config_path_for_dir,
    export_custom_profile, export_custom_profiles, import_custom_profile, load_profile_set,
    load_profile_set_at, save_profile_set, save_profile_set_at,
};

#[cfg(test)]
mod binding_tests;
#[cfg(test)]
mod editor_tests;
#[cfg(test)]
mod menu_command_tests;
#[cfg(test)]
mod profile_tests;
