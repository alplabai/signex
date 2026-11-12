//! The one-time macOS question about a custom shortcut profile (#1645): its
//! standard commands are still bound to Control, so ask once whether to move
//! them to Command. The rewrite is pure (`keymap::migrate_active_to_mod`);
//! the file work runs in a `Task`, and every outcome reaches the Messages
//! panel.

use iced::Task;

use crate::app::{KeymapModMigration, Message, Signex, UiMsg};
use crate::fonts::KEYMAP_MOD_PROMPT_ASKED_KEY;
use crate::keymap::{
    KeymapEditorModel, ModMigration, Platform, ShortcutProfileKind, ShortcutProfileSet,
};

const USE_COMMAND: &str = "Use \u{2318}";
const KEEP_CONTROL: &str = "Keep Control";

/// The question, when this launch must ask it: macOS, a custom profile that
/// still has rows to move, and not asked before. Only `Signex::boot` builds
/// it; a headless boot never does, since the dialog is native and the answer
/// is persisted.
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

/// Back the file up, then write the moved set; the result comes back only
/// once both succeeded. A failed backup refuses the write.
fn write_migration(
    source: ShortcutProfileSet,
    moved: ModMigration,
) -> Result<KeymapModMigration, String> {
    let backup = crate::keymap::back_up_profile_file().map_err(|error| {
        format!("the file could not be backed up first, so it was left as is ({error})")
    })?;
    crate::keymap::save_profile_set(&moved.set)
        .map_err(|error| format!("the file could not be written ({error})"))?;
    Ok(KeymapModMigration {
        source,
        set: moved.set,
        skipped: moved.skipped,
        backup: backup.map(|path| path.display().to_string()),
    })
}

/// Save the live profiles over the file: undoes a move that was not adopted.
fn resave_live(live: &ShortcutProfileSet) -> Result<(), String> {
    crate::keymap::save_profile_set(live)
        .map_err(|error| format!("the file could not be written ({error})"))
}

/// Persist that the question was answered. Off the update thread.
fn remember_asked() -> Task<Message> {
    crate::fonts::background_pref_write(KEYMAP_MOD_PROMPT_ASKED_KEY, || {
        crate::fonts::write_keymap_mod_prompt_asked()
    })
}

#[cfg(test)]
#[path = "keymap_mod_prompt_tests.rs"]
mod tests;

impl Signex {
    /// The answer to the prompt. "Keep Control" is remembered at once. "Use ⌘"
    /// starts the backup-and-write task and is remembered only once that
    /// landed, so a failed write asks again next launch.
    pub(crate) fn handle_keymap_mod_prompt_answered(&mut self, use_command: bool) -> Task<Message> {
        if !use_command {
            crate::diagnostics::log_info(
                "Keyboard shortcuts: kept Control for the standard commands; this is asked only once.",
            );
            self.sync_diagnostics_panel_ctx();
            return remember_asked();
        }
        // The set as it is now: the user may have edited it since the launch.
        let source = self.ui_state.keymap_profiles.clone();
        let Some(moved) = crate::keymap::migrate_active_to_mod(&source) else {
            crate::diagnostics::log_info(
                "Keyboard shortcuts: nothing to move to \u{2318}, no standard Control shortcut is left.",
            );
            self.sync_diagnostics_panel_ctx();
            return remember_asked();
        };
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || write_migration(source, moved))
                    .await
                    .map_err(|error| format!("the write task failed ({error})"))
                    .and_then(|result| result)
            },
            |result| Message::Ui(UiMsg::KeymapModMigrationWritten(result)),
        )
    }

    /// The Preferences keymap draft is a copy of the profile as it was when
    /// Preferences opened. With Preferences open, Apply would write that copy
    /// over the migrated file, so move the draft's own standard-set rows to
    /// `Mod` too; its other edits are kept, and an invalid trigger text still
    /// being typed is dropped by the swap. With Preferences closed the draft
    /// is only a stale seed, so it is re-seeded from the live set quietly.
    fn rebase_keymap_draft_onto_mod(&mut self, profile_id: &str) {
        if !self.ui_state.preferences_open {
            self.ui_state.preferences_keymap_editor =
                KeymapEditorModel::new(self.ui_state.keymap_profiles.clone());
            return;
        }
        let editor = &mut self.ui_state.preferences_keymap_editor;
        let Some(profile) = editor.profile_set().profile(profile_id).cloned() else {
            return;
        };
        if profile.kind != ShortcutProfileKind::Custom {
            return;
        }
        let moved = crate::keymap::with_standard_set_on_mod(&profile);
        if moved.moved == 0 {
            return;
        }
        match editor.insert_custom_profile(moved.profile) {
            Ok(()) => crate::diagnostics::log_info(
                "Keyboard shortcuts: the open Preferences draft was moved to \u{2318} too; its other edits are kept.",
            ),
            Err(error) => tracing::error!(
                target: "signex::keymap",
                "The open Preferences draft was not moved to \u{2318} ({error}); Apply there would write Control back"
            ),
        }
    }

    /// The stale move's file was replaced by the live profiles again, or that
    /// failed and the file still holds the older move.
    pub(crate) fn handle_keymap_mod_stale_resaved(
        &mut self,
        result: Result<(), String>,
    ) -> Task<Message> {
        match result {
            Ok(()) => crate::diagnostics::log_info(
                "Keyboard shortcuts: your saved shortcuts were written back over the stale move.",
            ),
            Err(error) => tracing::error!(
                target: "signex::keymap",
                "Stale move not undone: {error}; the file holds older shortcuts"
            ),
        }
        self.sync_diagnostics_panel_ctx();
        Task::none()
    }

    /// The move finished: adopt the new set and remember the question, or
    /// report why the file was left as it was (and ask again next launch).
    pub(crate) fn handle_keymap_mod_migration_written(
        &mut self,
        result: Result<KeymapModMigration, String>,
    ) -> Task<Message> {
        let follow_up = match result {
            Ok(done) if self.ui_state.keymap_profiles != done.source => {
                // An Apply landed between the answer and the write: the moved
                // copy is built from the older set and would undo it.
                // Messages cuts a long line, so the point comes first.
                let backup = done
                    .backup
                    .map(|path| format!("earlier file kept as {path}"))
                    .unwrap_or_else(|| "no earlier file to keep".to_string());
                crate::diagnostics::log_warning(format!(
                    "Move to \u{2318} not adopted: shortcuts changed meanwhile; {backup}; your saved shortcuts are being written back; asked again next launch."
                ));
                // The write already replaced the file with the move of the
                // older set, so put the live profiles back over it.
                let live = self.ui_state.keymap_profiles.clone();
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || resave_live(&live))
                            .await
                            .map_err(|error| format!("the write task failed ({error})"))
                            .and_then(|result| result)
                    },
                    |result| Message::Ui(UiMsg::KeymapModStaleResaved(result)),
                )
            }
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
                if !done.skipped.is_empty() {
                    crate::diagnostics::log_warning(format!(
                        "Keyboard shortcuts: left on Control because their \u{2318} chord is already bound: {}.",
                        done.skipped.join(", ")
                    ));
                }
                remember_asked()
            }
            Err(error) => {
                tracing::error!(
                    target: "signex::keymap",
                    "Shortcuts were not moved to \u{2318}: {error}"
                );
                Task::none()
            }
        };
        self.sync_diagnostics_panel_ctx();
        follow_up
    }
}
