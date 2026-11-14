//! Keyboard shortcut profile model and runtime lookup.
//!
//! The shape follows the useful parts of Zed's keymap architecture while
//! staying native to Signex: TOML profiles, EDA-oriented built-ins, stable
//! command ids, context-aware lookup, and editor-friendly conflict reporting.

mod backup;
mod binding;
mod catalog;
mod command;
mod display;
mod editor;
mod mod_migration;
mod physical;
mod platform;
mod profile;

pub use backup::{
    back_up_profile_file, back_up_profile_file_at, discard_profile_backup_at,
    existing_backup_profiles_path, restore_profiles_at,
};
pub use binding::{
    KeyBindingSource, KeyParseError, KeyStroke, KeyToken, Modifiers, ShortcutBinding,
    ShortcutBindingAction, ShortcutContext, ShortcutTrigger, mac_stroke_caps,
};
/// A hard-coded shortcut spelling as this OS shows it: `Mod+S` is `⌘S` on
/// macOS and `Ctrl+S` elsewhere. Text that is not a stroke is returned as is.
/// For hints that cannot ask the active keymap (fallbacks, prose).
pub fn shown(spelling: &str) -> String {
    shown_in(spelling, &signex_i18n::I18n::en_us())
}

/// [`shown`] with the key words in the language of `i18n`.
pub fn shown_in(spelling: &str, i18n: &signex_i18n::I18n) -> String {
    ShortcutTrigger::parse(spelling)
        .map(|trigger| trigger.display_text_in(Platform::current(), i18n))
        .unwrap_or_else(|_| spelling.to_string())
}

#[cfg(test)]
pub(crate) use catalog::OPERAND_CONVENTION;
pub use catalog::{
    CommandGroup, CommandMetadata, HeadlessUse, Operand, OperandKind, all_command_ids,
    category_title, command_menu_message, command_menu_text, fallback_label, metadata_for,
    metadata_for_str, offered_in,
};
pub use command::AppCommandId;
pub use editor::{
    BindingSlot, KeymapEditorModel, KeymapEditorProfile, KeymapEditorRow, KeymapEditorSource,
};
pub use mod_migration::{
    ModMigration, ModMove, migrate_active_to_mod, offers_mod_migration, standard_ctrl_rows,
    with_standard_set_on_mod,
};
pub use platform::Platform;
pub use profile::{
    BindingConflict, BuiltInProfile, CompiledKeymap, KeyLookup, ProfileLoadError, RestoredProfiles,
    ShortcutProfile, ShortcutProfileKind, ShortcutProfileSet, TomlShortcutProfile, config_path,
    config_path_for_dir, export_custom_profile, export_custom_profiles, import_custom_profile,
    load_profile_set, load_profile_set_at, read_backup_profiles_at, save_profile_set,
    save_profile_set_at,
};

#[cfg(test)]
mod binding_tests;
#[cfg(test)]
mod catalog_golden_tests;
#[cfg(test)]
mod editor_tests;
#[cfg(test)]
mod menu_command_tests;
#[cfg(test)]
mod mod_content_tests;
#[cfg(test)]
mod mod_migration_tests;
#[cfg(test)]
mod mod_token_tests;
#[cfg(test)]
mod profile_tests;
