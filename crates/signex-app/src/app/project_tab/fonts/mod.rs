//! Fonts section of the Project tab (#1178, mockup row 205): the fonts the
//! project carries in `assets/fonts/`, one row per family with its styles,
//! embedding permission and license. Add font file... reads, checks and
//! copies the file off the UI thread, then lists it as one step of the
//! project's General history; Remove and the license are steps too. The
//! `.snxprj` is dirty until Save; the copied file is on disk at once
//! (`store`). Catalog commands: `commands`. Rules: `docs/PROJECT_TAB.md`,
//! "Fonts".

mod commands;
mod store;
mod tab_state;
mod view;

#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use iced::Task;
use serde_json::{Value, json};
use signex_engine::ProjectFontCommand;
use signex_i18n::{Translatable, ids, t};
use signex_types::project::{FontEmbedding, MAX_FONT_LICENSE_CHARS, ProjectFontProblem};

pub(crate) use commands::{project_add_font, project_remove_font, project_set_font_license};
pub use store::StoredFont;
pub use tab_state::FontsTabState;
pub(super) use view::view;

use super::ProjectTabMsg;
use crate::app::agent::WaitingCommand;
use crate::app::command::CommandOutcome;
use crate::app::state::ProjectId;
use crate::app::{Message, ProjectMsg, Signex};
use crate::ignore::IgnoreResult;

/// Messages of the Fonts section, under `ProjectTabMsg::Fonts`.
#[derive(Debug, Clone)]
pub enum ProjectFontsMsg {
    /// Add font file...: the file picker.
    AddRequested,
    /// The picker answered; `None` is a cancel.
    FilePicked(Option<PathBuf>),
    /// The file was read, checked and copied off the UI thread, or why not.
    /// `waiting` is the command run with operands answered now; `None` for
    /// the picker's file.
    FileStored {
        source: PathBuf,
        stored: Arc<Result<StoredFont, Translatable>>,
        waiting: Option<Box<WaitingCommand>>,
    },
    Removed {
        family: String,
    },
    LicenseEdited {
        family: String,
        text: String,
    },
    /// Enter in a license box: commit what was typed.
    LicenseSubmitted {
        family: String,
    },
}

/// `msg` for the Fonts section of `project_id`.
pub(super) fn message(project_id: ProjectId, msg: ProjectFontsMsg) -> Message {
    Message::Project(ProjectMsg::Tab(ProjectTabMsg::Fonts { project_id, msg }))
}

/// The draft key of a family: names compare without case.
fn draft_key(family: &str) -> String {
    family.trim().to_lowercase()
}

/// The words of a refused font edit.
pub(super) fn problem_words(problem: &ProjectFontProblem) -> Translatable {
    match problem {
        ProjectFontProblem::Unreadable => Translatable::new(ids::PROJECT_FONTS_LIST_UNREADABLE),
        ProjectFontProblem::Restricted { family } => {
            Translatable::new(ids::PROJECT_FONTS_RESTRICTED).arg("family", family.clone())
        }
        ProjectFontProblem::NoSuchFamily { family } => {
            Translatable::new(ids::PROJECT_FONTS_NO_SUCH_FAMILY).arg("family", family.clone())
        }
        ProjectFontProblem::NotAnAssetPath { path } => {
            Translatable::new(ids::PROJECT_FONTS_BAD_PATH).arg("path", path.clone())
        }
        ProjectFontProblem::LicenseTooLong => {
            Translatable::new(ids::PROJECT_FONTS_LICENSE_TOO_LONG)
                .arg("max", MAX_FONT_LICENSE_CHARS)
        }
        ProjectFontProblem::NoFamily => Translatable::new(ids::PROJECT_FONTS_NO_FAMILY),
    }
}

/// The catalog word of an embedding permission.
pub(super) fn embedding_word(embedding: FontEmbedding) -> signex_i18n::MessageId {
    match embedding {
        FontEmbedding::Installable => ids::PROJECT_FONTS_EMBEDDING_INSTALLABLE,
        FontEmbedding::Editable => ids::PROJECT_FONTS_EMBEDDING_EDITABLE,
        FontEmbedding::PreviewAndPrint => ids::PROJECT_FONTS_EMBEDDING_PREVIEW_AND_PRINT,
        FontEmbedding::Restricted => ids::PROJECT_FONTS_EMBEDDING_RESTRICTED,
    }
}

impl Signex {
    pub(super) fn dispatch_project_fonts(
        &mut self,
        project_id: ProjectId,
        msg: ProjectFontsMsg,
    ) -> Task<Message> {
        // A late message for a closed project must not recreate its state.
        if self.document_state.project_by_id(project_id).is_none() {
            crate::diagnostics::log_warning(
                "Project tab: the project was closed; the fonts edit was dropped",
            );
            return Task::none();
        }
        match msg {
            ProjectFontsMsg::AddRequested => return self.pick_project_font(project_id),
            ProjectFontsMsg::FilePicked(None) => {}
            ProjectFontsMsg::FilePicked(Some(source)) => {
                match self.begin_font_store(project_id, source.clone(), None) {
                    Ok(task) => return task,
                    Err(reason) => self.note_font_refusal(project_id, &source, reason),
                }
            }
            ProjectFontsMsg::FileStored {
                source,
                stored,
                waiting,
            } => return self.finish_font_store(project_id, &source, &stored, waiting),
            ProjectFontsMsg::Removed { family } => {
                self.edit_fonts_from_tab(project_id, &ProjectFontCommand::Remove { family })
                    .ignore("the refusal is kept in the tab and shown under the table");
            }
            ProjectFontsMsg::LicenseEdited { family, text } => {
                let tab = self.fonts_tab(project_id);
                tab.license_drafts.insert(draft_key(&family), text);
                tab.problem = None;
            }
            ProjectFontsMsg::LicenseSubmitted { family } => {
                let key = draft_key(&family);
                let Some(text) = self.fonts_tab(project_id).license_drafts.get(&key) else {
                    return Task::none();
                };
                let command = ProjectFontCommand::SetLicense {
                    family,
                    license: text.clone(),
                };
                if self.edit_fonts_from_tab(project_id, &command).is_ok() {
                    self.fonts_tab(project_id).license_drafts.remove(&key);
                }
            }
        }
        Task::none()
    }

    fn fonts_tab(&mut self, project_id: ProjectId) -> &mut FontsTabState {
        &mut self
            .document_view
            .project_tabs
            .entry(project_id)
            .or_default()
            .fonts
    }

    /// The file picker of Add font file..., for fonts only.
    fn pick_project_font(&mut self, project_id: ProjectId) -> Task<Message> {
        let i18n = &self.ui_state.i18n;
        let dialog = self
            .ui_state
            .io
            .picker
            .dialog()
            .title_in(i18n, ids::PROJECT_FONTS_DIALOG_TITLE)
            .filter_in(
                i18n,
                ids::PROJECT_FONTS_FILTER,
                &signex_types::project::PROJECT_FONT_EXTENSIONS,
            );
        Task::perform(dialog.pick_file(), move |path| {
            message(project_id, ProjectFontsMsg::FilePicked(path))
        })
    }

    /// Start the off-thread read, check and copy of `source` into project
    /// `project_id`'s font folder. Refused at once for a project that may
    /// not change: read-only, a font list it cannot read, a folder on the
    /// move.
    pub(super) fn begin_font_store(
        &mut self,
        project_id: ProjectId,
        source: PathBuf,
        waiting: Option<Box<WaitingCommand>>,
    ) -> Result<Task<Message>, Translatable> {
        let project = self
            .document_state
            .project_by_id(project_id)
            .ok_or_else(|| Translatable::new(ids::PROJECT_FONTS_PROJECT_GONE))?;
        if project.data.unknown.newer_minor_source.is_some() {
            return Err(Translatable::new(ids::PROJECT_FONTS_READ_ONLY));
        }
        if let Err(problem) = project.data.project_fonts() {
            return Err(problem_words(&problem));
        }
        let project_path = project.path.clone();
        let dir = project.dir().to_path_buf();
        if self
            .refuse_while_project_moves(&project_path, "add a font to")
            .is_err()
        {
            return Err(Translatable::new(ids::PROJECT_FONTS_PROJECT_MOVING));
        }
        self.fonts_tab(project_id).adding = true;
        let reading = source.clone();
        Ok(Task::perform(
            async move {
                tokio::task::spawn_blocking(move || store::store_font_file(&reading, &dir))
                    .await
                    .unwrap_or_else(|error| {
                        Err(Translatable::new(ids::PROJECT_FONTS_IO)
                            .arg("file", String::new())
                            .arg("why", error.to_string()))
                    })
            },
            move |stored| {
                message(
                    project_id,
                    ProjectFontsMsg::FileStored {
                        source,
                        stored: Arc::new(stored),
                        waiting,
                    },
                )
            },
        ))
    }

    /// The copy landed: list the file, or keep why not, and answer the
    /// command that waited for it.
    fn finish_font_store(
        &mut self,
        project_id: ProjectId,
        source: &Path,
        stored: &Result<StoredFont, Translatable>,
        waiting: Option<Box<WaitingCommand>>,
    ) -> Task<Message> {
        let Some(waiting) = waiting else {
            self.list_stored_font(project_id, source, stored)
                .ignore("the outcome is in Messages and the tab");
            return Task::none();
        };
        let target = self
            .document_state
            .project_by_id(project_id)
            .map_or_else(|| source.to_path_buf(), |project| project.path.clone());
        self.finish_waiting_command(*waiting, &target, |app| {
            match app.list_stored_font(project_id, source, stored) {
                Ok(answer) => CommandOutcome::Answered(answer),
                Err(reason) => CommandOutcome::Disabled { reason },
            }
        })
    }

    /// List a copied file under its families as one General step and say so
    /// in Messages; on a refusal, say why there and keep it for the tab.
    /// Answers with the font list.
    fn list_stored_font(
        &mut self,
        project_id: ProjectId,
        source: &Path,
        stored: &Result<StoredFont, Translatable>,
    ) -> Result<Value, Translatable> {
        self.fonts_tab(project_id).adding = false;
        let font = match stored {
            Ok(font) => font,
            Err(reason) => {
                self.note_font_refusal(project_id, source, reason.clone());
                return Err(reason.clone());
            }
        };
        let command = ProjectFontCommand::AddFile {
            path: font.path.clone(),
            faces: font.info.faces.clone(),
            embedding: font.info.embedding,
        };
        let changed = match self.apply_project_font(project_id, &command) {
            Ok(changed) => changed,
            Err(problem) => {
                let reason = problem_words(&problem);
                self.note_font_refusal(project_id, source, reason.clone());
                return Err(reason);
            }
        };
        self.fonts_tab(project_id).problem = None;
        self.log_added_font(font);
        let mut answer = self.project_fonts_json(project_id);
        if let Value::Object(map) = &mut answer {
            map.insert("changed".into(), json!(changed));
            map.insert("added".into(), added_json(font));
        }
        Ok(answer)
    }

    /// The Messages lines of a listed font: what it was added as, and the
    /// license it names.
    fn log_added_font(&self, font: &StoredFont) {
        let i18n = &self.ui_state.i18n;
        let families = family_names(font).join(", ");
        let embedding = t!(i18n, embedding_word(font.info.embedding)).into_string();
        let line = t!(
            i18n,
            ids::PROJECT_FONTS_ADDED,
            "file" => font.file_name.clone(),
            "families" => families,
            "embedding" => embedding,
            "path" => font.path.clone()
        );
        crate::diagnostics::log_info(line.into_string());
        if let Some(license) = &font.info.license_hint {
            let line = t!(
                i18n,
                ids::PROJECT_FONTS_LICENSE_HINT,
                "file" => font.file_name.clone(),
                "license" => license.clone()
            );
            crate::diagnostics::log_info(line.into_string());
        }
    }

    /// A refused add: the Messages line, and the reason kept for the tab.
    fn note_font_refusal(&mut self, project_id: ProjectId, source: &Path, reason: Translatable) {
        let file = source
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| source.display().to_string());
        let line = t!(
            &self.ui_state.i18n,
            ids::PROJECT_FONTS_REFUSED,
            "file" => file,
            "why" => reason.render(&*self.ui_state.i18n)
        );
        crate::diagnostics::log_warning(line.into_string());
        let tab = self.fonts_tab(project_id);
        tab.adding = false;
        tab.problem = Some(reason);
    }

    /// Apply `command` from the tab and keep its refusal for the section.
    fn edit_fonts_from_tab(
        &mut self,
        project_id: ProjectId,
        command: &ProjectFontCommand,
    ) -> Result<bool, ProjectFontProblem> {
        let outcome = self.apply_project_font(project_id, command);
        self.fonts_tab(project_id).problem = outcome.as_ref().err().map(problem_words);
        outcome
    }

    /// Apply one Fonts edit to the project's data as one step of its General
    /// history and mark the `.snxprj` dirty: the tab's and the catalog
    /// commands' one commit path. `Ok(false)` when nothing changed or the
    /// project is no longer loaded.
    pub(crate) fn apply_project_font(
        &mut self,
        project_id: ProjectId,
        command: &ProjectFontCommand,
    ) -> Result<bool, ProjectFontProblem> {
        let document = &mut self.document_state;
        document.note_project_clean_state(project_id);
        let Some(project) = document.projects.iter_mut().find(|p| p.id == project_id) else {
            crate::diagnostics::log_warning(
                "Project tab: the project was closed; the edit was dropped",
            );
            return Ok(false);
        };
        let history = document.general_history.entry(project_id).or_default();
        if !history.apply_font(&mut project.data, command)? {
            return Ok(false);
        }
        document.sync_project_history_dirty(project_id, true);
        self.after_project_general_change(false);
        Ok(true)
    }

    /// The fonts of project `id` as stored, and whether the list can be
    /// read.
    pub(crate) fn project_fonts_json(&self, id: ProjectId) -> Value {
        let Some(project) = self.document_state.project_by_id(id) else {
            return json!({});
        };
        let data = &project.data;
        json!({
            "fonts": data.project_fonts_raw().cloned().unwrap_or_else(|| json!([])),
            "fonts_unreadable": data.project_fonts().is_err(),
        })
    }
}

/// The families a stored file was listed under, each once, in file order.
fn family_names(font: &StoredFont) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for face in &font.info.faces {
        if !names
            .iter()
            .any(|seen| seen.eq_ignore_ascii_case(&face.family))
        {
            names.push(face.family.clone());
        }
    }
    names
}

/// The `added` part of an add's answer.
fn added_json(font: &StoredFont) -> Value {
    let faces: Vec<Value> = font
        .info
        .faces
        .iter()
        .map(|face| json!({ "family": face.family, "style": face.style }))
        .collect();
    json!({
        "file": font.file_name,
        "path": font.path,
        "faces": faces,
        "embedding": font.info.embedding.token(),
        "license_hint": font.info.license_hint,
        "copied": font.copied,
    })
}
