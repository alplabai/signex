//! Library Browser class-admin handlers — the sidebar inline
//! "+ Class", rename-class and delete-class flows that edit the
//! library's `[[classes]]` block.
//!
//! Extracted verbatim from the library dispatcher (`dispatch/library`);
//! pure code motion, zero behaviour change.

use super::*;

impl Signex {
    /// Open the inline create-class form.
    pub(in crate::app::dispatch::library) fn handle_browser_begin_add_class(
        &mut self,
        library_path: std::path::PathBuf,
    ) -> Task<Message> {
        if let Some(s) = self.library.library_browsers.get_mut(&library_path) {
            s.adding_class = Some(crate::library::state::NewClassDraft::default());
            s.class_error = None;
        }
        Task::none()
    }

    /// Live-edit of the new-class key buffer.
    pub(in crate::app::dispatch::library) fn handle_browser_set_new_class_key(
        &mut self,
        library_path: std::path::PathBuf,
        value: String,
    ) -> Task<Message> {
        if let Some(s) = self.library.library_browsers.get_mut(&library_path)
            && let Some(d) = s.adding_class.as_mut()
        {
            d.key = value;
            d.error = None;
        }
        Task::none()
    }

    /// Live-edit of the new-class label buffer.
    pub(in crate::app::dispatch::library) fn handle_browser_set_new_class_label(
        &mut self,
        library_path: std::path::PathBuf,
        value: String,
    ) -> Task<Message> {
        if let Some(s) = self.library.library_browsers.get_mut(&library_path)
            && let Some(d) = s.adding_class.as_mut()
        {
            d.label = value;
            d.error = None;
        }
        Task::none()
    }

    /// Cancel the inline create-class form.
    pub(in crate::app::dispatch::library) fn handle_browser_cancel_add_class(
        &mut self,
        library_path: std::path::PathBuf,
    ) -> Task<Message> {
        if let Some(s) = self.library.library_browsers.get_mut(&library_path) {
            s.adding_class = None;
        }
        Task::none()
    }

    /// Append the new class to the library's `[[classes]]` block via
    /// `add_library_class` and refresh.
    pub(in crate::app::dispatch::library) fn handle_browser_confirm_add_class(
        &mut self,
        library_path: std::path::PathBuf,
    ) -> Task<Message> {
        let Some(state) = self.library.library_browsers.get(&library_path).cloned() else {
            return Task::none();
        };
        let Some(draft) = state.adding_class.clone() else {
            return Task::none();
        };
        let key = draft.key.trim().to_string();
        let label = draft.label.trim().to_string();
        if key.is_empty() || label.is_empty() {
            if let Some(s) = self.library.library_browsers.get_mut(&library_path)
                && let Some(d) = s.adding_class.as_mut()
            {
                d.error = Some("Both key and label are required.".into());
            }
            return Task::none();
        }
        let library_id = match self.library.library_at(&library_path) {
            Some(lib) => lib.library_id,
            None => return Task::none(),
        };
        let adapter = match self.library.set.get(library_id) {
            Some(a) => a,
            None => return Task::none(),
        };
        // Atomic add — `add_library_class` does the
        // duplicate-key check + push inside one
        // `mutate_library_file` borrow on LocalGitAdapter
        // (and the trait default falls back to the legacy
        // two-step path for adapters without single-borrow
        // support).
        if let Err(error) = adapter.add_library_class(
            signex_library::ClassEntry {
                key: key.clone(),
                label,
            },
            &format!("add class {key}"),
        ) {
            if let Some(s) = self.library.library_browsers.get_mut(&library_path)
                && let Some(d) = s.adding_class.as_mut()
            {
                d.error = Some(error.to_string());
            }
            return Task::none();
        }
        if let Some(s) = self.library.library_browsers.get_mut(&library_path) {
            s.adding_class = None;
            s.class_error = None;
        }
        Task::none()
    }

    /// Per-row `×` delete — drops the matching class from the library's
    /// `[[classes]]` block.
    pub(in crate::app::dispatch::library) fn handle_browser_delete_class(
        &mut self,
        library_path: std::path::PathBuf,
        key: String,
    ) -> Task<Message> {
        let library_id = match self.library.library_at(&library_path) {
            Some(lib) => lib.library_id,
            None => return Task::none(),
        };
        let adapter = match self.library.set.get(library_id) {
            Some(a) => a,
            None => return Task::none(),
        };
        if let Err(error) = adapter.remove_library_class(&key, &format!("delete class {key}")) {
            if let Some(s) = self.library.library_browsers.get_mut(&library_path) {
                s.class_error = Some(error.to_string());
            }
        }
        Task::none()
    }

    /// Sidebar `✎` rename for a class row — flips it into edit mode.
    pub(in crate::app::dispatch::library) fn handle_browser_begin_rename_class(
        &mut self,
        library_path: std::path::PathBuf,
        key: String,
    ) -> Task<Message> {
        let library_id = match self.library.library_at(&library_path) {
            Some(lib) => lib.library_id,
            None => return Task::none(),
        };
        let label = self
            .library
            .set
            .get(library_id)
            .and_then(|adapter| adapter.library_classes().into_iter().find(|c| c.key == key))
            .map(|c| c.label)
            .unwrap_or_default();
        if let Some(s) = self.library.library_browsers.get_mut(&library_path) {
            s.renaming_class = Some((key.clone(), key, label));
            s.class_error = None;
        }
        Task::none()
    }

    /// Live-edit of the rename-class key buffer.
    pub(in crate::app::dispatch::library) fn handle_browser_set_rename_class_key(
        &mut self,
        library_path: std::path::PathBuf,
        value: String,
    ) -> Task<Message> {
        if let Some(s) = self.library.library_browsers.get_mut(&library_path)
            && let Some((_, k, _)) = s.renaming_class.as_mut()
        {
            *k = value;
            s.class_error = None;
        }
        Task::none()
    }

    /// Live-edit of the rename-class label buffer.
    pub(in crate::app::dispatch::library) fn handle_browser_set_rename_class_label(
        &mut self,
        library_path: std::path::PathBuf,
        value: String,
    ) -> Task<Message> {
        if let Some(s) = self.library.library_browsers.get_mut(&library_path)
            && let Some((_, _, l)) = s.renaming_class.as_mut()
        {
            *l = value;
            s.class_error = None;
        }
        Task::none()
    }

    /// Cancel the inline rename-class form.
    pub(in crate::app::dispatch::library) fn handle_browser_cancel_rename_class(
        &mut self,
        library_path: std::path::PathBuf,
    ) -> Task<Message> {
        if let Some(s) = self.library.library_browsers.get_mut(&library_path) {
            s.renaming_class = None;
            s.class_error = None;
        }
        Task::none()
    }

    /// Confirm — writes the renamed class via `rename_library_class`.
    pub(in crate::app::dispatch::library) fn handle_browser_confirm_rename_class(
        &mut self,
        library_path: std::path::PathBuf,
    ) -> Task<Message> {
        let Some(state) = self.library.library_browsers.get(&library_path).cloned() else {
            return Task::none();
        };
        let Some((orig, new_key, new_label)) = state.renaming_class.clone() else {
            return Task::none();
        };
        let new_key = new_key.trim().to_string();
        let new_label = new_label.trim().to_string();
        if new_key.is_empty() || new_label.is_empty() {
            if let Some(s) = self.library.library_browsers.get_mut(&library_path) {
                s.class_error = Some("Both key and label are required.".into());
            }
            return Task::none();
        }
        let library_id = match self.library.library_at(&library_path) {
            Some(lib) => lib.library_id,
            None => return Task::none(),
        };
        let adapter = match self.library.set.get(library_id) {
            Some(a) => a,
            None => return Task::none(),
        };
        // Atomic rename — `rename_library_class` does the
        // existence check + duplicate check + replace inside
        // a single `mutate_library_file` borrow on the
        // LocalGitAdapter (and falls back to the trait
        // default's two-step on adapters without
        // single-borrow support).
        if let Err(error) = adapter.rename_library_class(
            &orig,
            signex_library::ClassEntry {
                key: new_key.clone(),
                label: new_label,
            },
            &format!("rename class {orig} → {new_key}"),
        ) {
            if let Some(s) = self.library.library_browsers.get_mut(&library_path) {
                s.class_error = Some(error.to_string());
            }
            return Task::none();
        }
        if let Some(s) = self.library.library_browsers.get_mut(&library_path) {
            s.renaming_class = None;
            s.class_error = None;
        }
        Task::none()
    }
}
