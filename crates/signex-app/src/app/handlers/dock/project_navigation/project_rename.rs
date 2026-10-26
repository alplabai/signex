//! Project rename: the `.snxprj` file stem is the project name. One flow,
//! reached from the project tree's Rename and the Project tab's Rename…
//! (#1202); no save path renames anything.

use std::path::{Path, PathBuf};

use super::*;

/// Why `stem` cannot be a project name (the `.snxprj` file stem), or `None`
/// when it can.
fn project_stem_problem(stem: &str) -> Option<&'static str> {
    if stem.trim().is_empty() {
        Some("Name cannot be empty.")
    } else if stem.contains(['/', '\\', '.']) {
        Some("Name cannot contain '/', '\\', or '.'.")
    } else {
        None
    }
}

/// Move `from` to `to`. When `to` already exists as the same file (a
/// case-only rename on a case-insensitive volume), go through a temporary
/// name so the volume records the new spelling.
fn rename_project_file(from: &Path, to: &Path) -> Result<(), String> {
    if !to.exists() {
        return std::fs::rename(from, to).map_err(|e| format!("Rename failed: {e}"));
    }
    if !same_file::is_same_file(from, to).unwrap_or(false) {
        return Err("A project with that name already exists.".to_string());
    }
    let temp = from.with_extension(format!("snxprj.rename-{}", uuid::Uuid::new_v4()));
    std::fs::rename(from, &temp).map_err(|e| format!("Rename failed: {e}"))?;
    if let Err(e) = std::fs::rename(&temp, to) {
        // Put the original name back; report both if that fails too.
        return Err(match std::fs::rename(&temp, from) {
            Ok(()) => format!("Rename failed: {e}"),
            Err(back) => format!(
                "Rename failed: {e}; the project file is now {} ({back})",
                temp.display()
            ),
        });
    }
    Ok(())
}

impl Signex {
    /// Project-root rename — `state.target_path` is the project's
    /// `.snxprj`; the buffer is the new *stem*.
    pub(super) fn handle_project_rename_submit(
        &mut self,
        state: &crate::app::RenameDialogState,
    ) -> iced::Task<Message> {
        let new_stem = state.buffer.trim();
        if let Some(problem) = project_stem_problem(new_stem) {
            self.set_rename_error(problem);
            return iced::Task::none();
        }
        let current_stem = state.target_path.file_stem().and_then(|s| s.to_str());
        if current_stem == Some(new_stem) {
            self.ui_state.rename_dialog = None;
            return iced::Task::none();
        }
        let Some(idx) = self
            .document_state
            .projects
            .iter()
            .position(|p| p.path == state.target_path)
        else {
            self.set_rename_error("The project is no longer open.");
            return iced::Task::none();
        };
        if let Err(reason) = self.move_project_file(idx, new_stem) {
            self.set_rename_error(&reason);
            return iced::Task::none();
        }
        self.ui_state.rename_dialog = None;
        iced::Task::none()
    }

    /// Rename the `.snxprj` of `projects[idx]` to `<new_stem>.snxprj` in the
    /// same folder and move everything keyed on its path. Companion
    /// schematic / PCB files keep their names; `schematic_root` / `pcb_file`
    /// still point at them. Returns the new path, or why nothing moved.
    ///
    /// Refuses while a write of the project file is in flight instead of
    /// waiting for it: that write runs only after `update` returns, so
    /// waiting here would hang the app, and letting it land after the move
    /// would recreate the old file.
    pub(crate) fn move_project_file(
        &mut self,
        idx: usize,
        new_stem: &str,
    ) -> Result<PathBuf, String> {
        let from = self
            .document_state
            .projects
            .get(idx)
            .map(|p| p.path.clone())
            .ok_or_else(|| "The project is no longer open.".to_string())?;
        let dir = from
            .parent()
            .ok_or_else(|| "Project file has no parent directory.".to_string())?;
        let to = dir.join(format!("{new_stem}.snxprj"));
        if self.document_state.document_saves.is_saving(&from) {
            return Err(
                "The project file is still being saved; try again in a moment.".to_string(),
            );
        }
        rename_project_file(&from, &to)?;
        // The tree's file facts were probed under the old name.
        self.document_state.project_fs.invalidate();
        self.rekey_renamed_project(idx, &from, &to, new_stem);
        crate::diagnostics::log_info(format!(
            "[rename] Project file {} renamed to {}",
            from.display(),
            to.display()
        ));
        Ok(to)
    }

    /// Point everything keyed on the old `.snxprj` path at the new one: the
    /// project record and its name, tabs, engines, the active path, the
    /// dirty marker, save bookkeeping, page setup, undocked windows and
    /// File > Open Recent.
    fn rekey_renamed_project(&mut self, idx: usize, from: &Path, to: &Path, new_stem: &str) {
        if let Some(loaded) = self.document_state.projects.get_mut(idx) {
            loaded.path = to.to_path_buf();
            loaded.data.name = new_stem.to_string();
        }
        for tab in self.document_state.tabs.iter_mut() {
            if tab.path == from {
                tab.path = to.to_path_buf();
                tab.title = if tab.kind.as_project().is_some() {
                    crate::app::project_tab::project_tab_title(to)
                } else {
                    new_stem.to_string()
                };
            }
        }
        if let Some(engine) = self.document_state.engines.remove(from) {
            self.document_state.engines.insert(to.to_path_buf(), engine);
        }
        if self.document_state.active_path.as_deref() == Some(from) {
            self.document_state.active_path = Some(to.to_path_buf());
        }
        if self.document_state.dirty_paths.remove(from) {
            self.document_state.dirty_paths.insert(to.to_path_buf());
        }
        self.document_state
            .document_saves
            .forget_where(|path| path == from);
        self.rekey_page_setup(from, to);
        self.rekey_undocked_windows(from, to);
        self.retarget_recent_project(from, to);
        self.refresh_panel_ctx();
    }
}

#[cfg(test)]
mod tests {
    use super::project_stem_problem;

    #[test]
    fn a_stem_must_be_non_empty_and_free_of_separators_and_dots() {
        assert_eq!(project_stem_problem("Oscar"), None);
        assert_eq!(project_stem_problem("Motor controller"), None);
        assert!(project_stem_problem("").is_some());
        assert!(project_stem_problem("   ").is_some());
        for bad in ["a/b", "a\\b", "a.b"] {
            assert!(project_stem_problem(bad).is_some(), "{bad}");
        }
    }
}
