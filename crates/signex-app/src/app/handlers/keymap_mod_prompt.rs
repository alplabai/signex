//! The one-time macOS question about a custom shortcut profile (#1645): its
//! standard commands are still bound to Control, so ask once whether to move
//! them to Command. The rewrite is pure (`keymap::migrate_active_to_mod`);
//! the file work runs in a `Task`, and every outcome reaches the Messages
//! panel.

use iced::Task;

use crate::app::{KeymapModMigration, Message, Signex, UiMsg};
use crate::keymap::{Platform, ShortcutProfileKind, ShortcutProfileSet};

const PREFS_KEY: &str = "keymap_mod_prompt_asked";
const USE_COMMAND: &str = "Use \u{2318}";
const KEEP_CONTROL: &str = "Keep Control";

/// The question, when this launch must ask it: macOS, a custom profile that
/// still has standard-set `Ctrl` rows, and not asked before. Built so that
/// a boot that only constructs the app opens no dialog until the runtime
/// runs the task.
pub(in crate::app) fn keymap_mod_prompt_boot(
    set: &ShortcutProfileSet,
    prefs: Option<&serde_json::Value>,
    platform: Platform,
) -> Task<Message> {
    if !crate::keymap::offers_mod_migration(set, platform)
        || crate::fonts::read_keymap_mod_prompt_asked_from(prefs)
    {
        return Task::none();
    }
    let description = format!(
        "Your keyboard shortcut profile \"{}\" binds standard commands (Save, Undo, Copy, \
         ...) to Control. On a Mac these use \u{2318}.\n\n\
         \"{USE_COMMAND}\" rewrites those shortcuts to \u{2318} and keeps a backup of your \
         shortcuts file. \"{KEEP_CONTROL}\" leaves the profile as it is.\n\n\
         You are only asked once.",
        set.active_profile().name
    );
    Task::perform(
        async move {
            crate::io_ports::ask_two(
                "Use \u{2318} for your shortcuts?",
                &description,
                USE_COMMAND,
                KEEP_CONTROL,
            )
            .await
        },
        |use_command| Message::Ui(UiMsg::KeymapModPromptAnswered { use_command }),
    )
}

/// Back the file up, then write the moved set; the profile set to adopt
/// comes back only once both succeeded. A failed backup refuses the write.
fn write_migration(moved: ShortcutProfileSet) -> Result<KeymapModMigration, String> {
    let backup = crate::keymap::back_up_profile_file().map_err(|error| {
        format!("the file could not be backed up first, so it was left as is ({error})")
    })?;
    crate::keymap::save_profile_set(&moved)
        .map_err(|error| format!("the file could not be written ({error})"))?;
    Ok(KeymapModMigration {
        set: moved,
        backup: backup.map(|path| path.display().to_string()),
    })
}

#[cfg(test)]
#[path = "keymap_mod_prompt_tests.rs"]
mod tests;

impl Signex {
    /// The answer to the prompt: remember that it was asked, and for "Use ⌘"
    /// start the backup-and-write task.
    pub(crate) fn handle_keymap_mod_prompt_answered(&mut self, use_command: bool) -> Task<Message> {
        let asked = crate::fonts::background_pref_write(PREFS_KEY, || {
            crate::fonts::write_keymap_mod_prompt_asked()
        });
        if !use_command {
            crate::diagnostics::log_info(
                "Keyboard shortcuts: kept Control for the standard commands; this is asked only once.",
            );
            self.sync_diagnostics_panel_ctx();
            return asked;
        }
        // The set as it is now: the user may have edited it since the launch.
        let Some(moved) = crate::keymap::migrate_active_to_mod(&self.ui_state.keymap_profiles)
        else {
            crate::diagnostics::log_info(
                "Keyboard shortcuts: nothing to move to \u{2318}, no standard Control shortcut is left.",
            );
            self.sync_diagnostics_panel_ctx();
            return asked;
        };
        let write = Task::perform(
            async move {
                tokio::task::spawn_blocking(move || write_migration(moved))
                    .await
                    .map_err(|error| format!("the write task failed ({error})"))
                    .and_then(|result| result)
            },
            |result| Message::Ui(UiMsg::KeymapModMigrationWritten(result)),
        );
        Task::batch([asked, write])
    }

    /// The Preferences keymap draft is a copy of the profile as it was when
    /// Preferences opened. Apply would write that copy over the migrated
    /// file, so move the draft's own standard-set rows to `Mod` too; its
    /// other edits are kept. An invalid trigger text the user is still typing
    /// is dropped by the swap.
    fn rebase_keymap_draft_onto_mod(&mut self, profile_id: &str) {
        let editor = &mut self.ui_state.preferences_keymap_editor;
        let Some(profile) = editor.profile_set().profile(profile_id).cloned() else {
            return;
        };
        if profile.kind != ShortcutProfileKind::Custom
            || crate::keymap::standard_ctrl_rows(&profile) == 0
        {
            return;
        }
        match editor.insert_custom_profile(crate::keymap::with_standard_set_on_mod(&profile)) {
            Ok(()) => crate::diagnostics::log_info(
                "Keyboard shortcuts: the open Preferences draft was moved to \u{2318} too; its other edits are kept.",
            ),
            Err(error) => tracing::error!(
                target: "signex::keymap",
                "The open Preferences draft was not moved to \u{2318} ({error}); Apply there would write Control back"
            ),
        }
    }

    /// The move finished: adopt the new set and say so, or report why the
    /// file was left as it was.
    pub(crate) fn handle_keymap_mod_migration_written(
        &mut self,
        result: Result<KeymapModMigration, String>,
    ) -> Task<Message> {
        match result {
            Ok(done) => {
                self.ui_state.set_keymap_profiles(done.set);
                self.ui_state.active_keymap =
                    std::sync::Arc::new(self.ui_state.keymap_profiles.compile_active());
                let migrated = self
                    .ui_state
                    .keymap_profiles
                    .active_profile_id()
                    .to_string();
                self.rebase_keymap_draft_onto_mod(&migrated);
                let kept = done
                    .backup
                    .map(|path| format!(" The previous file is kept as {path}."))
                    .unwrap_or_default();
                crate::diagnostics::log_info(format!(
                    "Keyboard shortcuts: the standard commands now use \u{2318}.{kept}"
                ));
            }
            Err(error) => tracing::error!(
                target: "signex::keymap",
                "Shortcuts were not moved to \u{2318}: {error}"
            ),
        }
        self.sync_diagnostics_panel_ctx();
        Task::none()
    }
}
