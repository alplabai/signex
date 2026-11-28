//! Relink and Replace (#1116, mockup row 204). Relink… points every image
//! that names one file, on every sheet of the project, at another file;
//! Replace… points one image at another file. Either is one undo step
//! (`write_sheets_as_group`): a closed sheet is changed in a tabless engine
//! and left unsaved, nothing is written to disk but the picked file's copy
//! into `assets/images/` when a changed sheet is saved. The Properties
//! buttons read the picked file and the project's closed sheets in a
//! `Task`; `relink_image` reads them as an edit's reads. Both end in
//! [`Signex::relink_images`].

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use iced::Task;
use serde_json::{Value, json};
use signex_engine::Command;
use signex_i18n::{ids, t};
use signex_types::image_asset::{ImageReadError, PickedImage};
use signex_types::schematic::{SchDrawing, SchImage, SchematicSheet};
use uuid::Uuid;

use super::ImagesMsg;
use crate::app::sheet_write::{SheetWrite, SheetWriteRefusal};
use crate::app::{Message, Signex, UndoGroupKind};

/// Which images a relink points at the new file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelinkScope {
    /// Every image naming this project-relative file, on every sheet of the
    /// project (Relink…).
    File(String),
    /// One image of the sheet the edit acts on (Replace…).
    Image(Uuid),
}

impl RelinkScope {
    fn takes(&self, image: &SchImage) -> bool {
        match self {
            Self::File(file) => image.file == *file,
            Self::Image(id) => image.uuid == *id,
        }
    }
}

/// The sheets of a project by path, closed ones as read from disk.
pub type ProjectSheets = HashMap<PathBuf, SchematicSheet>;

/// What a Properties Relink… or Replace… read off the UI thread.
#[derive(Debug, Clone)]
pub struct RelinkRead {
    pub picked: Result<Arc<PickedImage>, ImageReadError>,
    /// The project's sheets; `None` for a loose sheet or a Replace.
    pub project: Option<Arc<ProjectSheets>>,
}

fn refusal_text(refusal: SheetWriteRefusal) -> String {
    match refusal {
        SheetWriteRefusal::Busy(path) => format!("{} is being saved or moved", path.display()),
        SheetWriteRefusal::ReadOnly(path) => format!("{} opens read-only", path.display()),
        SheetWriteRefusal::NotLoaded(path) => format!("{} could not be loaded", path.display()),
        SheetWriteRefusal::NotChanged(path) => format!("{} refused the change", path.display()),
    }
}

/// The commands that point the images `scope` takes on `sheet` at `picked`.
fn relink_commands(
    sheet: &SchematicSheet,
    scope: &RelinkScope,
    picked: &PickedImage,
) -> Vec<Command> {
    let px = [picked.facts.width_px, picked.facts.height_px];
    sheet
        .drawings
        .iter()
        .filter_map(|d| match d {
            SchDrawing::Image(image) if scope.takes(image) => Some(Command::UpdateSchDrawing {
                drawing: SchDrawing::Image(image.relinked(&picked.file, &picked.facts.hash, px)),
            }),
            _ => None,
        })
        .collect()
}

impl Signex {
    /// Point the images `scope` names at `picked`, as one undo step across
    /// the sheets that hold them. `target` is the sheet the edit acts on;
    /// `project` its project's sheets (closed ones as read), `None` for a
    /// loose sheet. Answers with how many images and which sheets changed.
    pub(crate) fn relink_images(
        &mut self,
        target: &Path,
        scope: &RelinkScope,
        picked: &PickedImage,
        project: Option<&ProjectSheets>,
    ) -> Result<Value, String> {
        let mut candidates: Vec<(PathBuf, SchematicSheet)> = Vec::new();
        let live = |path: &Path| {
            self.document_state
                .engines
                .get(path)
                .map(|engine| engine.document().clone())
        };
        match (scope, project) {
            (RelinkScope::File(_), Some(sheets)) => {
                for (path, read) in sheets {
                    let sheet = live(path).unwrap_or_else(|| read.clone());
                    candidates.push((path.clone(), sheet));
                }
                if !sheets.contains_key(target)
                    && let Some(sheet) = live(target)
                {
                    candidates.push((target.to_path_buf(), sheet));
                }
            }
            _ => {
                let sheet = live(target).ok_or("no schematic is open")?;
                candidates.push((target.to_path_buf(), sheet));
            }
        }
        let mut edits: BTreeMap<PathBuf, ()> = BTreeMap::new();
        let mut loaded: HashMap<PathBuf, SchematicSheet> = HashMap::new();
        let mut images = 0;
        for (path, sheet) in candidates {
            let count = relink_commands(&sheet, scope, picked).len();
            if count == 0 {
                continue;
            }
            images += count;
            if !self.document_state.engines.contains_key(&path) {
                loaded.insert(path.clone(), sheet);
            }
            edits.insert(path, ());
        }
        if edits.is_empty() {
            return Err(match scope {
                RelinkScope::File(file) => format!("no image names {file}"),
                RelinkScope::Image(id) => format!("no image {id} on this sheet"),
            });
        }
        let sheets: Vec<PathBuf> = edits.keys().cloned().collect();
        let write = SheetWrite {
            label: "relink_image",
            outcome: "relinked",
            kind: UndoGroupKind::ImageRelink,
            loaded: &loaded,
        };
        let scope_for = scope.clone();
        let picked_for = picked.clone();
        self.write_sheets_as_group(&write, edits, move |sheet, _| {
            relink_commands(sheet, &scope_for, &picked_for)
        })
        .map_err(refusal_text)?;
        for sheet in &sheets {
            self.note_image_copy(sheet, picked);
        }
        self.ui_state.image_cache.retry_missing();
        Ok(json!({
            "relinked": images,
            "sheets": sheets,
            "file": picked.file,
            "hash": picked.facts.hash,
            "format": picked.facts.format.as_str(),
            "width_px": picked.facts.width_px,
            "height_px": picked.facts.height_px,
            "copied_on_save": picked.needs_copy,
        }))
    }

    /// Relink… or Replace… was pressed in Properties: ask for the file.
    pub(super) fn relink_dialog(&mut self, scope: RelinkScope) -> Task<Message> {
        if self.image_target().is_none() {
            return Task::none();
        }
        let i18n = &self.ui_state.i18n;
        let dialog = self
            .ui_state
            .io
            .picker
            .dialog()
            .title_in(i18n, ids::IMAGES_RELINK_TITLE)
            .filter_in(i18n, ids::IMAGES_FILTER, super::place::IMAGE_EXTENSIONS)
            .filter_in(i18n, ids::FILE_FILTER_ALL_FILES, &["*"]);
        Task::perform(dialog.pick_file(), move |path| {
            ImagesMsg::RelinkPicked {
                scope: scope.clone(),
                path,
            }
            .into()
        })
    }

    /// The relink file was picked: read and check it, and for Relink… the
    /// project's closed sheets, off the UI thread.
    pub(super) fn relink_picked(
        &mut self,
        scope: RelinkScope,
        path: Option<PathBuf>,
    ) -> Task<Message> {
        let Some(path) = path else {
            return Task::none();
        };
        let Some(target) = self.image_target() else {
            return Task::none();
        };
        let root = self.image_root(&target);
        let pending = self.pending_image_names(&root);
        let project = match scope {
            RelinkScope::File(_) => self.project_read_of(&target),
            RelinkScope::Image(_) => None,
        };
        let revision = self
            .document_state
            .engines
            .get(&target)
            .map(signex_engine::Engine::revision);
        Task::perform(
            async move {
                let job = move || {
                    let picked =
                        super::read::read_picked_image(&path, &root, &pending).map(Arc::new);
                    let project = project.map(|read| Arc::new(read.assemble().sheets));
                    RelinkRead { picked, project }
                };
                tokio::task::spawn_blocking(job)
                    .await
                    .unwrap_or_else(|error| RelinkRead {
                        picked: Err(ImageReadError::Unreadable(error.to_string())),
                        project: None,
                    })
            },
            move |read| {
                ImagesMsg::RelinkRead {
                    scope: scope.clone(),
                    target: target.clone(),
                    revision,
                    read: Box::new(read),
                }
                .into()
            },
        )
    }

    /// The relink's reads landed: relink, unless the sheet changed or is
    /// gone in between; say the outcome in Messages.
    pub(super) fn relink_read(
        &mut self,
        scope: &RelinkScope,
        target: &Path,
        revision: Option<u64>,
        read: RelinkRead,
    ) {
        let now = self
            .document_state
            .engines
            .get(target)
            .map(signex_engine::Engine::revision);
        if now.is_none() || now != revision {
            crate::diagnostics::log_info(
                t!(self.ui_state.i18n, ids::IMAGES_CANCELLED).into_string(),
            );
            return;
        }
        let i18n = std::sync::Arc::clone(&self.ui_state.i18n);
        let picked = match read.picked {
            Ok(picked) => picked,
            Err(error) => {
                let reason = super::read_error_text(&i18n, &error);
                crate::diagnostics::log_warning(
                    t!(i18n, ids::IMAGES_RELINK_FAILED, "reason" => reason).into_string(),
                );
                return;
            }
        };
        match self.relink_images(target, scope, &picked, read.project.as_deref()) {
            Ok(answer) => {
                let count = answer["relinked"].as_u64().unwrap_or(0);
                crate::diagnostics::log_info(
                    t!(i18n, ids::IMAGES_RELINKED, "count" => count, "file" => picked.file.as_str())
                        .into_string(),
                );
            }
            Err(reason) => crate::diagnostics::log_warning(
                t!(i18n, ids::IMAGES_RELINK_FAILED, "reason" => reason).into_string(),
            ),
        }
    }
}
