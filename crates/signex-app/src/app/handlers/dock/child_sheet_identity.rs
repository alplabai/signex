//! Properties-panel Name / File editing for a selected sheet symbol (#794).
//!
//! The text inputs buffer keystrokes; Enter commits one
//! `Command::ModifyChildSheet` through the mutation gateway (undo, dirty,
//! netlist invalidation). A refused edit leaves the document alone and
//! raises an [`ErrorNotice`]. Re-pointing only changes the reference — no
//! file is ever created, moved or renamed here.

use crate::app::project_sheets::ownership_reference;
use crate::app::state::ErrorNotice;
use signex_i18n::{Translatable, ids};

use super::super::super::*;

impl Signex {
    pub(super) fn type_child_sheet_name(&mut self, id: uuid::Uuid, text: &str) {
        self.document_state.panel_ctx.child_sheet_name_buf = Some((id, text.to_string()));
    }

    pub(super) fn type_child_sheet_file(&mut self, id: uuid::Uuid, text: &str) {
        self.document_state.panel_ctx.child_sheet_file_buf = Some((id, text.to_string()));
    }

    /// Commit the buffered Name for sheet `id`. Nothing typed, nothing done.
    pub(super) fn commit_child_sheet_name(&mut self, id: uuid::Uuid) {
        let Some(name) = take_buffer(&mut self.document_state.panel_ctx.child_sheet_name_buf, id)
        else {
            return;
        };
        if let Some(taken) = self.name_used_by_another_open_sheet(id, name.trim()) {
            self.refuse_child_sheet_edit(
                Translatable::new(ids::NOTICE_SHEET_NAME_TAKEN).arg("name", taken),
            );
            return;
        }
        self.apply_child_sheet_edit(id, Some(name), None);
    }

    /// Commit the buffered File reference for sheet `id`.
    pub(super) fn commit_child_sheet_file(&mut self, id: uuid::Uuid) {
        let Some(filename) =
            take_buffer(&mut self.document_state.panel_ctx.child_sheet_file_buf, id)
        else {
            return;
        };
        let trimmed = filename.trim();
        // A blank reference is the engine's to refuse (it has the wording);
        // everything else must stay inside the sheet's folder first, by the
        // same resolver navigation and the netlist use.
        if !trimmed.is_empty() && self.resolve_child_sheet_path(trimmed).is_none() {
            self.refuse_child_sheet_edit(
                Translatable::new(ids::NOTICE_SHEET_FILE_OUTSIDE).arg("file", trimmed),
            );
            return;
        }
        if let Some(taken) = self.file_used_by_another_open_sheet(id, trimmed) {
            self.refuse_child_sheet_edit(
                Translatable::new(ids::NOTICE_SHEET_FILE_TAKEN).arg("file", taken),
            );
            return;
        }
        self.apply_child_sheet_edit(id, None, Some(filename));
    }

    /// Browse… beside File: pick a sheet file off the update thread, starting
    /// in the folder of the sheet the edit acts on. Pressed in an undocked
    /// window, the picked file comes back scoped to that window.
    pub(super) fn browse_child_sheet_file(&mut self, id: uuid::Uuid) -> iced::Task<Message> {
        let dir = (self.document_state.active_path.as_deref())
            .and_then(|path| path.parent().map(std::path::Path::to_path_buf));
        let window = self.interaction_state.acting_window;
        let picker = self.ui_state.io.picker.clone();
        let i18n = self.ui_state.i18n.clone();
        iced::Task::perform(
            async move {
                let mut dialog = picker
                    .dialog()
                    .title_in(&i18n, ids::PROPERTIES_SHEET_BROWSE_TITLE)
                    .filter_in(&i18n, ids::FILE_FILTER_SCHEMATIC, &["snxsch"]);
                if let Some(dir) = dir {
                    dialog = dialog.set_directory(dir);
                }
                dialog.pick_file().await
            },
            move |picked| {
                let browsed =
                    crate::dock::DockMessage::Panel(crate::panels::PanelMsg::SelectionForm(
                        crate::panels::SelectionFormMsg::SheetFileBrowsed(id, picked),
                    ));
                Message::Dock(match window {
                    Some(window) => {
                        crate::dock::DockMessage::ScopedToWindow(window, Box::new(browsed))
                    }
                    None => browsed,
                })
            },
        )
    }

    /// The browsed file becomes the File reference of sheet `id`, relative to
    /// the active sheet's folder, committed as if typed and Entered: the same
    /// checks refuse a file outside the folder or used by another symbol.
    pub(super) fn child_sheet_file_browsed(
        &mut self,
        id: uuid::Uuid,
        picked: Option<&std::path::Path>,
    ) {
        let Some(picked) = picked else {
            return;
        };
        let reference = (self.document_state.active_path.as_deref())
            .and_then(|path| path.parent().map(std::path::Path::to_path_buf))
            .and_then(|dir| {
                picked
                    .strip_prefix(dir)
                    .ok()
                    .map(std::path::Path::to_path_buf)
            })
            .map(|relative| relative.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|| picked.to_string_lossy().into_owned());
        self.type_child_sheet_file(id, &reference);
        self.commit_child_sheet_file(id);
    }

    fn apply_child_sheet_edit(
        &mut self,
        id: uuid::Uuid,
        name: Option<String>,
        filename: Option<String>,
    ) {
        let (_, refusal) = self.apply_engine_command_reporting(
            signex_engine::Command::ModifyChildSheet {
                sheet_id: id,
                name,
                filename,
            },
            true,
            true,
        );
        // The engine words its own refusals (#2060).
        if let Some(reason) = refusal {
            self.refuse_child_sheet_edit(crate::app::state::untranslated(reason));
        }
    }

    fn refuse_child_sheet_edit(&mut self, detail: Translatable) {
        self.document_state.error_notice = Some(ErrorNotice::sheet_edit(detail));
    }

    /// The existing name that `name` would collide with in another open
    /// sheet of the active project, if any. The engine only sees its own
    /// document, so project-wide uniqueness is checked here.
    fn name_used_by_another_open_sheet(&self, id: uuid::Uuid, name: &str) -> Option<String> {
        self.document_state
            .engines_in_active_document_project()
            .into_iter()
            .flat_map(|engine| engine.document().child_sheets.iter())
            .find(|c| c.uuid != id && c.name.trim().eq_ignore_ascii_case(name))
            .map(|c| c.name.clone())
    }

    /// The reference of another open sheet symbol in the active project that
    /// resolves to the same file as `filename` (typed against the active
    /// sheet's folder). Each reference is resolved against the folder of the
    /// sheet that carries it, so `a/x.snxsch` and `x.snxsch` held by sheets
    /// in different folders are told apart. The engine only sees its own
    /// document; the same-document clash is its to refuse.
    fn file_used_by_another_open_sheet(&self, id: uuid::Uuid, filename: &str) -> Option<String> {
        let active = self.document_state.active_path.as_ref()?;
        let target = file_identity(&ownership_reference(active.parent()?, filename)?);
        self.document_state
            .engine_entries_in_active_document_project()
            .into_iter()
            .filter(|(path, _)| *path != active)
            .find_map(|(path, engine)| {
                let dir = path.parent()?;
                engine
                    .document()
                    .child_sheets
                    .iter()
                    .filter(|c| c.uuid != id)
                    .find(|c| {
                        ownership_reference(dir, &c.filename)
                            .is_some_and(|p| file_identity(&p) == target)
                    })
                    .map(|c| c.filename.clone())
            })
    }
}

/// Case- and separator-insensitive key for a resolved sheet path, matching
/// the engine's own file-reference comparison.
fn file_identity(path: &std::path::Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase()
}

/// The buffered text for sheet `id`, clearing the buffer either way.
fn take_buffer(buf: &mut Option<(uuid::Uuid, String)>, id: uuid::Uuid) -> Option<String> {
    buf.take()
        .and_then(|(owner, text)| (owner == id).then_some(text))
}
