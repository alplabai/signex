//! What a schematic Save As does to the project that listed the sheet (#1824).
//!
//! Save As moves the tab and engine to the new file and leaves the old file
//! on disk untouched. Without this step the project would keep listing the
//! old file, so reopening it would show the unedited sheet and the edit would
//! live in a file the project does not know about.
//!
//! The rule mirrors Rename (`dock/project_navigation/rename_apply.rs`): a
//! Save As target inside the project folder takes the old file's place in the
//! page list and as `schematic_root`, open parent sheets re-point their
//! child-sheet references, and the `.snxprj` is written at once (atomically,
//! off the UI thread) so Discard All or a crash cannot leave it naming the
//! old file. A target outside the project folder is a copy that is not part
//! of the project: the project is left alone and Messages says so.

use std::path::Path;

use iced::Task;

use super::super::super::*;
use super::save::file_label;
use crate::app::state::library_entry::entry_resolves_to;
use crate::app::state::{LoadedProject, ProjectId};

/// Point `loaded`'s page list and schematic root at `new_entry` where they
/// name `from`. A page that already lists the target takes over, so the old
/// entry is dropped instead of duplicating it. Returns whether anything
/// changed.
fn repoint_entries(loaded: &mut LoadedProject, from: &Path, to: &Path, new_entry: &str) -> bool {
    let fold = loaded.case_fold;
    let dir = loaded.dir().to_path_buf();
    let resolves = |entry: &str, target: &Path| entry_resolves_to(&dir, entry, target, fold);
    let target_listed = loaded
        .data
        .sheets
        .iter()
        .any(|sheet| resolves(&sheet.filename, to));
    let old_stem = from.file_stem().map(|s| s.to_string_lossy().into_owned());
    let new_stem = to
        .file_stem()
        .map_or_else(String::new, |s| s.to_string_lossy().into_owned());
    let before = loaded.data.sheets.len();
    let mut changed = false;
    let mut kept = Vec::with_capacity(before);
    for mut sheet in std::mem::take(&mut loaded.data.sheets) {
        if !resolves(&sheet.filename, from) {
            kept.push(sheet);
            continue;
        }
        changed = true;
        if target_listed {
            continue;
        }
        // A name the user customised is theirs; only the default follows.
        if Some(&sheet.name) == old_stem.as_ref() {
            sheet.name = new_stem.clone();
        }
        sheet.filename = new_entry.to_string();
        kept.push(sheet);
    }
    loaded.data.sheets = kept;
    if loaded
        .data
        .schematic_root
        .as_deref()
        .is_some_and(|root| resolves(root, from))
    {
        loaded.data.schematic_root = Some(new_entry.to_string());
        changed = true;
    }
    changed
}

impl Signex {
    /// The schematic `from` was saved as `to`. `owner` is the project that
    /// held `from` before the move. Returns the write of the project file.
    pub(super) fn follow_save_as_in_project(
        &mut self,
        owner: Option<ProjectId>,
        from: &Path,
        to: &Path,
    ) -> Task<Message> {
        let Some(project_id) = owner else {
            return Task::none();
        };
        let Some(idx) = self
            .document_state
            .projects
            .iter()
            .position(|p| p.id == project_id)
        else {
            return Task::none();
        };
        let dir = self.document_state.projects[idx].dir().to_path_buf();
        let Ok(relative) = to.strip_prefix(&dir) else {
            crate::diagnostics::log_warning(format!(
                "[save-as] {} is outside the project folder, so it is not part of the \
                 project, which still lists {}",
                file_label(to),
                file_label(from)
            ));
            return Task::none();
        };
        let new_entry = relative
            .components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        let changed = repoint_entries(&mut self.document_state.projects[idx], from, to, &new_entry);
        // References are re-pointed even when the sheet was only reachable as
        // a child: the parents named a file that no longer holds its sheet.
        let stale_parents = self.repoint_open_parents_after_save_as(idx, from, to);
        if !stale_parents.is_empty() {
            crate::diagnostics::log_warning(format!(
                "[save-as] {} still reference {}; re-point them to {} by hand",
                stale_parents.join(", "),
                file_label(from),
                file_label(to)
            ));
        }
        if !changed {
            return Task::none();
        }
        let project_file = self.document_state.projects[idx].path.clone();
        self.document_state.mark_document_dirty(&project_file);
        match self.begin_project_save(project_id) {
            Ok(task) => task,
            Err(error) => {
                self.report_save_failure("Save As could not update the project file", &error);
                Task::none()
            }
        }
    }
}
