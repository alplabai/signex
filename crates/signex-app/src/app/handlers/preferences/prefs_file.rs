//! The broken-file banner's actions (#602, mockup row 143): start a fresh
//! `prefs.json`, and show the folder that holds it. Split out of `mod.rs`.

use iced::Task;

use super::super::super::*;

impl Signex {
    /// Move an unloadable `prefs.json` aside so writes resume from a fresh
    /// file. Never moves a healthy file: the user may have repaired it by
    /// hand between the banner being painted and the click, and renaming a
    /// good prefs.json away would be #594 again, by hand.
    ///
    /// Every outcome is reported to the Messages panel as well as to the
    /// dialog: the status string dies with the dialog, and a recovery action
    /// that touches the user's config files has to leave a trace that
    /// outlives the window. No draft is touched, so the dirty flag is not
    /// recomputed.
    pub(super) fn handle_prefs_file_reset(&mut self) {
        let path = crate::fonts::prefs_file_path();
        if crate::fonts::check_prefs_file().is_ok() {
            self.ui_state.prefs_load_error = None;
            tracing::info!(
                target: "signex::prefs",
                path = %path.display(),
                "preferences reset was requested but the file now loads cleanly, \
                 so nothing was moved"
            );
            self.ui_state.preferences_prefs_status =
                "Your preferences file is readable again — nothing was moved.".to_string();
            return;
        }
        match crate::fonts::move_prefs_file_aside() {
            Ok(Some(aside)) => {
                self.ui_state.prefs_load_error = None;
                tracing::info!(
                    target: "signex::prefs",
                    path = %path.display(),
                    kept_as = %aside.display(),
                    "the unreadable preferences file was moved aside on the user's \
                     request and a fresh empty one put in its place; preferences \
                     save again from now on"
                );
                // No drafts are written here, so the fresh file is empty and
                // anything "saved" while writes were refused is only in
                // memory. Say so rather than let the user quit believing it
                // persisted.
                self.ui_state.preferences_prefs_status = format!(
                    "Preferences reset. The unreadable file was kept as {}. \
                     Change and save a setting to write a new one.",
                    aside.display()
                );
            }
            Ok(None) => {
                self.ui_state.prefs_load_error = None;
                // `warn!`: the probe a moment ago said this file was
                // unloadable and the rename found nothing, so something
                // outside this process removed it mid-action.
                tracing::warn!(
                    target: "signex::prefs",
                    path = %path.display(),
                    "the unreadable preferences file was gone before it could be \
                     moved aside; something outside this process removed it"
                );
                self.ui_state.preferences_prefs_status =
                    "There was no preferences file to move aside.".to_string();
            }
            Err(error) => {
                // The flag stays set: nothing changed on disk, so the banner
                // stays up and keeps offering the action.
                tracing::error!(
                    target: "signex::prefs",
                    path = %path.display(),
                    error = %error,
                    "the unreadable preferences file could not be moved aside, so \
                     it was left exactly as it is and preferences still cannot be \
                     saved"
                );
                self.ui_state.preferences_prefs_status = format!(
                    "Could not start a fresh preferences file: the unreadable one \
                     could not be moved aside, so it was left untouched ({error})."
                );
            }
        }
    }

    /// Show the folder that holds `prefs.json` in the file manager, off the
    /// update thread. A launch that fails is reported in Messages.
    pub(super) fn open_prefs_folder(&self) -> Task<Message> {
        let file = crate::fonts::prefs_file_path();
        let folder = file
            .parent()
            .map_or_else(|| file.clone(), |dir| dir.to_path_buf());
        let opener = self.ui_state.io.opener;
        let i18n = self.ui_state.i18n.clone();
        Task::future(async move {
            let shown = folder.clone();
            let outcome = tokio::task::spawn_blocking(move || opener.reveal(&shown)).await;
            let failure = match outcome {
                Ok(Ok(())) => None,
                Ok(Err(error)) => Some(error.to_string()),
                Err(error) => Some(
                    signex_i18n::t!(
                        i18n,
                        signex_i18n::ids::PREFERENCES_FILE_OPEN_FOLDER_UNFINISHED,
                        "error" => error.to_string()
                    )
                    .into_string(),
                ),
            };
            if let Some(reason) = failure {
                let line = signex_i18n::t!(
                    i18n,
                    signex_i18n::ids::PREFERENCES_FILE_OPEN_FOLDER_FAILED,
                    "path" => folder.display().to_string(),
                    "why" => reason
                );
                crate::diagnostics::log_warning(line.into_string());
            }
        })
        .discard()
    }
}
