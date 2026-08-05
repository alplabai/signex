//! Keyboard Shortcuts pane handlers — every `PrefMsg::Keymap*` arm.
//!
//! Split out of the flat `handlers/preferences.rs` when it crossed the
//! 1000-line god-file cap (#603). Pure code motion: every arm body below
//! is byte-identical to what it was in that file.
//!
//! The caller routes here by listing all 19 `PrefMsg::Keymap*` variants
//! explicitly rather than with a `_ =>` catch-all, so its match stays
//! exhaustive over `PrefMsg`: adding a non-keymap variant is a compile
//! error there instead of a message silently arriving here.

use iced::Task;

use super::super::super::*;
use crate::preferences::PrefMsg;

impl Signex {
    /// Handle a Keyboard Shortcuts pane message.
    ///
    /// Only reachable for the variants the caller lists, so the
    /// fall-through arm below is dead by construction. It reports rather
    /// than panicking anyway: if that routing list and this match ever
    /// drift apart, a logged line beats taking the UI thread down over a
    /// misrouted message.
    pub(super) fn handle_keymap_pref_message(
        &mut self,
        msg: crate::preferences::PrefMsg,
    ) -> Task<Message> {
        match msg {
            PrefMsg::KeymapSearchChanged(query) => {
                // Pure view filter — does not touch the editor model or the
                // dirty flag, so opening/searching never marks unsaved work.
                self.ui_state.preferences_keymap_search = query;
            }
            PrefMsg::KeymapProfileSelected(id) => {
                match self
                    .ui_state
                    .preferences_keymap_editor
                    .set_active_profile(id)
                {
                    Ok(()) => {
                        self.ui_state.preferences_keymap_status.clear();
                        self.recompute_preferences_dirty();
                    }
                    Err(error) => {
                        self.ui_state.preferences_keymap_status =
                            format!("Could not switch profile: {error}");
                    }
                }
            }
            PrefMsg::KeymapCreateCustomProfile => {
                // Pick the first free `custom-N` id so repeated forks
                // don't collide with an existing draft.
                let profiles = self.ui_state.preferences_keymap_editor.profiles();
                let next_index = profiles
                    .iter()
                    .filter(|profile| profile.kind == crate::keymap::ShortcutProfileKind::Custom)
                    .count()
                    + 1;
                let mut candidate = next_index;
                loop {
                    let id = format!("custom-{candidate}");
                    if profiles.iter().all(|profile| profile.id != id) {
                        let name = format!("Custom {candidate}");
                        match self
                            .ui_state
                            .preferences_keymap_editor
                            .create_custom_from_active(id, name)
                        {
                            Ok(()) => {
                                self.ui_state.preferences_keymap_status =
                                    "Created a custom profile draft.".to_string();
                                self.recompute_preferences_dirty();
                            }
                            Err(error) => {
                                self.ui_state.preferences_keymap_status =
                                    format!("Could not create profile: {error}");
                            }
                        }
                        break;
                    }
                    candidate += 1;
                }
            }
            PrefMsg::KeymapDeleteActiveProfile => {
                let active_id = self
                    .ui_state
                    .preferences_keymap_editor
                    .profiles()
                    .into_iter()
                    .find(|profile| profile.active)
                    .map(|profile| profile.id);
                if let Some(id) = active_id {
                    match self
                        .ui_state
                        .preferences_keymap_editor
                        .delete_custom_profile(&id)
                    {
                        Ok(()) => {
                            self.ui_state.preferences_keymap_status =
                                "Deleted the custom profile draft.".to_string();
                            self.recompute_preferences_dirty();
                        }
                        Err(error) => {
                            self.ui_state.preferences_keymap_status =
                                format!("Could not delete profile: {error}");
                        }
                    }
                }
            }
            PrefMsg::KeymapRestoreFromBackup => {
                let Some(bak) = crate::keymap::existing_backup_profiles_path() else {
                    self.refresh_keymap_backup();
                    self.ui_state.preferences_keymap_status =
                        "There is no keyboard-shortcuts backup to restore from.".to_string();
                    return Task::none();
                };
                // Read it through the normal loader rather than copying
                // the file back: the backup holds the file that failed to
                // load, so putting it back would reproduce the same
                // failure on the next launch.
                let restored = match crate::keymap::read_backup_profiles_at(&bak) {
                    Ok(restored) => restored,
                    Err(error) => {
                        tracing::error!(
                            target: "signex::keymap",
                            path = %bak.display(),
                            error = %error,
                            "the keyboard-shortcuts backup could not be read; both files were \
                             left exactly as they are"
                        );
                        self.ui_state.preferences_keymap_status = format!(
                            "Could not restore from the backup: {error}. Both files were left \
                             as they are."
                        );
                        return Task::none();
                    }
                };
                let Some(live) = crate::keymap::config_path() else {
                    self.ui_state.preferences_keymap_status =
                        "Could not restore from the backup: there is no config directory to \
                         write to."
                            .to_string();
                    return Task::none();
                };
                match crate::keymap::restore_profiles_at(&live, &restored.set) {
                    Ok(previous) => {
                        self.ui_state.keymap_profiles = restored.set.clone();
                        self.ui_state.preferences_keymap_editor =
                            crate::keymap::KeymapEditorModel::new(restored.set);
                        self.ui_state.keymap_load_error = None;
                        self.refresh_keymap_backup();
                        self.recompute_preferences_dirty();
                        let kept = previous
                            .map(|p| {
                                format!(
                                    " The shortcuts file it replaced was kept as {}.",
                                    p.display()
                                )
                            })
                            .unwrap_or_default();
                        // A dangling `active_profile` is the one failure
                        // the restore recovers from rather than refusing,
                        // so say it happened — the user's active profile
                        // is not the one the backup asked for.
                        let reset = restored
                            .active_profile_reset
                            .map(|id| {
                                format!(
                                    " The backup's active profile \"{id}\" no longer exists, so \
                                     a built-in one is active."
                                )
                            })
                            .unwrap_or_default();
                        tracing::info!(
                            target: "signex::keymap",
                            path = %bak.display(),
                            "keyboard-shortcut profiles were restored from the backup on the \
                             user's request"
                        );
                        self.ui_state.preferences_keymap_status =
                            format!("Keyboard shortcuts restored from the backup.{kept}{reset}");
                    }
                    Err(error) => {
                        tracing::error!(
                            target: "signex::keymap",
                            path = %live.display(),
                            error = %error,
                            "the restored profiles could not be written; nothing was changed"
                        );
                        self.ui_state.preferences_keymap_status = format!(
                            "Could not restore from the backup: the profiles were read but \
                             could not be written ({error})."
                        );
                    }
                }
            }
            PrefMsg::KeymapDiscardBackup => {
                let Some(bak) = crate::keymap::existing_backup_profiles_path() else {
                    self.refresh_keymap_backup();
                    self.ui_state.preferences_keymap_status =
                        "There is no keyboard-shortcuts backup to discard.".to_string();
                    return Task::none();
                };
                match crate::keymap::discard_profile_backup_at(&bak) {
                    Ok(_) => {
                        self.refresh_keymap_backup();
                        tracing::info!(
                            target: "signex::keymap",
                            path = %bak.display(),
                            "the keyboard-shortcuts backup was deleted on the user's request"
                        );
                        self.ui_state.preferences_keymap_status = format!(
                            "Deleted the keyboard-shortcuts backup at {}.",
                            bak.display()
                        );
                    }
                    Err(error) => {
                        tracing::error!(
                            target: "signex::keymap",
                            path = %bak.display(),
                            error = %error,
                            "the keyboard-shortcuts backup could not be deleted"
                        );
                        self.ui_state.preferences_keymap_status =
                            format!("Could not delete the backup at {}: {error}.", bak.display());
                    }
                }
            }
            PrefMsg::KeymapImportProfile => {
                return Task::future(async {
                    let picked = rfd::AsyncFileDialog::new()
                        .set_title("Import Signex Keyboard Shortcuts")
                        .add_filter("Signex Keyboard Shortcuts", &["toml"])
                        .pick_file()
                        .await;
                    if let Some(f) = picked {
                        let bytes = f.read().await;
                        let source = String::from_utf8_lossy(&bytes).to_string();
                        Message::Preferences(PreferencesMsg::Inner(PrefMsg::KeymapProfileLoaded(
                            source,
                        )))
                    } else {
                        Message::Noop
                    }
                });
            }
            PrefMsg::KeymapProfileLoaded(source) => {
                match crate::keymap::import_custom_profile(&source).and_then(|profile| {
                    self.ui_state
                        .preferences_keymap_editor
                        .insert_custom_profile(profile)
                }) {
                    Ok(()) => {
                        self.ui_state.preferences_keymap_status =
                            "Imported a custom keyboard shortcut profile.".to_string();
                        self.recompute_preferences_dirty();
                    }
                    Err(error) => {
                        self.ui_state.preferences_keymap_status =
                            format!("Could not import keyboard shortcuts: {error}");
                    }
                }
            }
            PrefMsg::KeymapExportProfile => {
                let profile = self
                    .ui_state
                    .preferences_keymap_editor
                    .active_profile()
                    .clone();
                match crate::keymap::export_custom_profile(&profile) {
                    Ok(source) => {
                        let filename = format!("{}.toml", profile.id);
                        // Same reset as the theme export: the status line
                        // must describe this attempt, not the last one.
                        self.ui_state.preferences_keymap_status.clear();
                        return Task::future(async move {
                            let picked = rfd::AsyncFileDialog::new()
                                .set_title("Export Signex Keyboard Shortcuts")
                                .add_filter("Signex Keyboard Shortcuts", &["toml"])
                                .set_file_name(&filename)
                                .save_file()
                                .await;
                            let Some(f) = picked else {
                                return Message::Noop;
                            };
                            let path = f.path().to_path_buf();
                            // The serialization failure below already
                            // reported through `preferences_keymap_status`;
                            // the write failure was the half that vanished
                            // (#533). Same `io::Result<()>` shape as the
                            // theme export — see that arm for why there is
                            // no short write to handle.
                            let outcome = match f.write(source.as_bytes()).await {
                                Ok(()) => Ok(path),
                                Err(error) => {
                                    Err(format!("{path}: {error}", path = path.display()))
                                }
                            };
                            Message::Preferences(PreferencesMsg::Inner(
                                PrefMsg::KeymapExportFinished(outcome),
                            ))
                        });
                    }
                    Err(error) => {
                        self.ui_state.preferences_keymap_status =
                            format!("Could not export keyboard shortcuts: {error}");
                    }
                }
            }
            PrefMsg::KeymapExportFinished(result) => {
                self.ui_state.preferences_keymap_status = match result {
                    Ok(path) => format!("Exported keyboard shortcuts to {}.", path.display()),
                    Err(detail) => format!("Could not export keyboard shortcuts: {detail}"),
                };
            }
            PrefMsg::KeymapBindingChanged {
                command,
                context,
                trigger,
            } => {
                match self
                    .ui_state
                    .preferences_keymap_editor
                    .edit_active_trigger(command, context, trigger)
                {
                    Ok(()) => {
                        self.ui_state.preferences_keymap_status.clear();
                        self.recompute_preferences_dirty();
                    }
                    Err(error) => {
                        self.ui_state.preferences_keymap_status =
                            format!("Invalid shortcut: {error}");
                        self.recompute_preferences_dirty();
                    }
                }
            }
            PrefMsg::KeymapRecorderOpen {
                command,
                label,
                context,
                trigger,
            } => {
                self.ui_state.preferences_keymap_recorder = Some(
                    crate::app::KeymapRecorderState::new(command, label, context, trigger),
                );
                self.ui_state.preferences_keymap_status.clear();
            }
            PrefMsg::KeymapRecorderCancel => {
                self.ui_state.preferences_keymap_recorder = None;
            }
            PrefMsg::KeymapRecorderStart => {
                if let Some(recorder) = &mut self.ui_state.preferences_keymap_recorder {
                    recorder.recording = true;
                    recorder.strokes.clear();
                    recorder.modifiers = crate::keymap::Modifiers::default();
                }
            }
            PrefMsg::KeymapRecorderStop => {
                if let Some(recorder) = &mut self.ui_state.preferences_keymap_recorder {
                    recorder.recording = false;
                    recorder.modifiers = crate::keymap::Modifiers::default();
                }
            }
            PrefMsg::KeymapRecorderClear => {
                if let Some(recorder) = &mut self.ui_state.preferences_keymap_recorder {
                    recorder.strokes.clear();
                    recorder.modifiers = crate::keymap::Modifiers::default();
                    recorder.recording = true;
                }
            }
            PrefMsg::KeymapRecorderModifiersChanged(modifiers) => {
                if let Some(recorder) = &mut self.ui_state.preferences_keymap_recorder
                    && recorder.recording
                {
                    recorder.modifiers = modifiers;
                }
            }
            PrefMsg::KeymapRecorderKeyPressed(stroke) => {
                if let Some(recorder) = &mut self.ui_state.preferences_keymap_recorder
                    && recorder.recording
                {
                    // Wrap back to a single stroke once the chord is
                    // full so a fourth press starts a fresh capture.
                    if recorder.strokes.len() >= crate::app::KeymapRecorderState::MAX_STROKES {
                        recorder.strokes.clear();
                    }
                    recorder.strokes.push(stroke);
                    recorder.modifiers = crate::keymap::Modifiers::default();
                }
            }
            PrefMsg::KeymapRecorderApply => {
                let Some(recorder) = self.ui_state.preferences_keymap_recorder.clone() else {
                    return Task::none();
                };
                if recorder.strokes.is_empty() {
                    self.ui_state.preferences_keymap_status =
                        "Record at least one keystroke before applying.".to_string();
                    return Task::none();
                }
                let trigger = recorder.trigger_text();
                match self.ui_state.preferences_keymap_editor.edit_active_trigger(
                    recorder.command,
                    recorder.context,
                    trigger,
                ) {
                    Ok(()) => {
                        self.ui_state.preferences_keymap_status.clear();
                        self.recompute_preferences_dirty();
                        self.ui_state.preferences_keymap_recorder = None;
                    }
                    Err(error) => {
                        self.ui_state.preferences_keymap_status =
                            format!("Invalid shortcut: {error}");
                        self.recompute_preferences_dirty();
                    }
                }
            }
            other => {
                debug_assert!(
                    false,
                    "handle_keymap_pref_message received a non-keymap variant: {other:?}"
                );
                tracing::error!(
                    target: "signex::preferences",
                    message = ?other,
                    "a Preferences message reached the keyboard-shortcuts handler that does \
                     not own it; it was dropped"
                );
            }
        }

        Task::none()
    }
}
