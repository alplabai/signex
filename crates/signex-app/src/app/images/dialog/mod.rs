//! The Place Image dialog (#1116, mockup row 204): the project's images
//! under `assets/images/` as thumbnails with a filter, a large preview of
//! the chosen one with its facts (file, format, size, uses), and the size it
//! is placed at: a Width field (the height follows) filled with the first
//! size for the sheet's free area, or the file's own size at 96 dpi. Add
//! image file… takes a file from anywhere; one from outside the project is
//! marked new and copied into `assets/images/` when the sheet is saved.
//! Place puts the image on the cursor; one click places it as one undo step.
//! The folder is read in a `Task`; the view only reads this state.

#[cfg(test)]
mod tests;
mod view;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use iced::Task;
use signex_i18n::{ids, t};
use signex_types::image_asset::{ImageReadError, PickedImage};
use signex_types::schematic::{first_size_mm, free_area_mm};

use super::{ImageEntry, ImagesMsg};
use crate::app::{Message, Signex};

pub(crate) use view::view;

/// One file the dialog lists.
#[derive(Debug, Clone)]
pub struct DialogFile {
    /// Project-relative path, `/` separated.
    pub file: String,
    pub read: Result<Arc<PickedImage>, ImageReadError>,
    /// The picture of the thumbnail and the preview.
    pub thumb: Option<ImageEntry>,
    /// Picked from outside the project: the save copies it in.
    pub is_new: bool,
}

impl DialogFile {
    fn of(file: String, read: Result<Arc<PickedImage>, ImageReadError>) -> Self {
        let thumb = read
            .as_ref()
            .ok()
            .map(|picked| ImageEntry::of(picked.facts.format, &picked.bytes));
        let is_new = read.as_ref().is_ok_and(|picked| picked.needs_copy);
        Self {
            file,
            read,
            thumb,
            is_new,
        }
    }

    /// The name the thumbnail shows.
    pub fn name(&self) -> &str {
        self.file.rsplit('/').next().unwrap_or(&self.file)
    }
}

/// The dialog's state while it is open.
#[derive(Debug, Clone)]
pub struct PlaceDialog {
    /// The sheet the image lands on.
    pub target: PathBuf,
    /// The folder the project's images are listed from is being read.
    pub loading: bool,
    pub files: Vec<DialogFile>,
    pub filter: String,
    /// Index into `files`.
    pub chosen: Option<usize>,
    /// The Width field's text, in mm.
    pub width: String,
    /// Place at the file's own size instead of `width`.
    pub at_file_size: bool,
}

impl PlaceDialog {
    fn new(target: PathBuf) -> Self {
        Self {
            target,
            loading: true,
            files: Vec::new(),
            filter: String::new(),
            chosen: None,
            width: String::new(),
            at_file_size: false,
        }
    }

    /// The files the filter lets through, with their index.
    pub fn shown(&self) -> impl Iterator<Item = (usize, &DialogFile)> {
        let needle = self.filter.trim().to_lowercase();
        self.files
            .iter()
            .enumerate()
            .filter(move |(_, f)| needle.is_empty() || f.file.to_lowercase().contains(&needle))
    }

    /// The chosen file, when it was read and passed the checks.
    pub fn chosen_pick(&self) -> Option<&Arc<PickedImage>> {
        self.files.get(self.chosen?)?.read.as_ref().ok()
    }

    /// The width Place uses: the file's own at 96 dpi, or the typed one.
    /// `None` while the field does not read as a length above zero.
    pub fn placed_width(&self) -> Option<f64> {
        let picked = self.chosen_pick()?;
        if self.at_file_size {
            return Some(picked.facts.natural_size_mm().0);
        }
        let width = signex_types::numeric::parse_finite_f64(self.width.trim()).ok()?;
        (width > 0.0 && width <= signex_types::schematic::IMAGE_MAX_EDGE_MM).then_some(width)
    }
}

/// What happened in the Place Image dialog.
#[derive(Debug, Clone)]
pub enum PlaceDialogMsg {
    /// The project's images folder was read for the dialog on `target`.
    Listed {
        target: PathBuf,
        files: Vec<(String, Result<Arc<PickedImage>, ImageReadError>)>,
    },
    FilterChanged(String),
    Chosen(usize),
    WidthTyped(String),
    /// The size choice: the file's own size (`true`) or the Width field.
    FileSizePicked(bool),
    AddFilePressed,
    PlacePressed,
    Closed,
}

/// Read every file of `root`'s images folder for the dialog. Blocking.
fn read_project_images(root: &Path) -> Vec<(String, Result<Arc<PickedImage>, ImageReadError>)> {
    super::read::list_asset_folder(root)
        .into_iter()
        .map(|file| {
            let read = super::read::read_picked_image(&root.join(&file), root, &[]).map(Arc::new);
            (file, read)
        })
        .collect()
}

impl Signex {
    /// Place ▸ Graphic…: open the dialog on the active sheet and read its
    /// project's images.
    pub(super) fn open_place_dialog(&mut self) -> Task<Message> {
        let Some(target) = self.image_target() else {
            return Task::none();
        };
        let root = self.image_root(&target);
        self.ui_state.image_cache.dialog = Some(Box::new(PlaceDialog::new(target.clone())));
        Task::perform(
            async move {
                let read_root = root.clone();
                tokio::task::spawn_blocking(move || read_project_images(&read_root))
                    .await
                    .unwrap_or_default()
            },
            move |files| ImagesMsg::Dialog(PlaceDialogMsg::Listed { target, files }).into(),
        )
    }

    /// A file picked with Add image file… was read: list it and choose it.
    /// `false` when no dialog waits for it.
    pub(super) fn dialog_took_pick(
        &mut self,
        target: &Path,
        read: &Result<Arc<PickedImage>, ImageReadError>,
        picked_name: String,
    ) -> bool {
        let Some(dialog) = self.ui_state.image_cache.dialog.as_deref_mut() else {
            return false;
        };
        if dialog.target != target {
            return false;
        }
        let file = match read {
            Ok(picked) => picked.file.clone(),
            Err(_) => picked_name,
        };
        let entry = DialogFile::of(file.clone(), read.clone());
        let index = match dialog.files.iter().position(|f| f.file == file) {
            Some(index) => {
                dialog.files[index] = entry;
                index
            }
            None => {
                dialog.files.push(entry);
                dialog.files.len() - 1
            }
        };
        self.choose_in_dialog(index);
        true
    }

    /// Choose file `index` and fill the Width field with its first size.
    fn choose_in_dialog(&mut self, index: usize) {
        let free = self
            .ui_state
            .image_cache
            .dialog
            .as_deref()
            .and_then(|dialog| {
                self.document_state
                    .engines
                    .get(&dialog.target)
                    .map(|engine| free_area_mm(engine.document()))
            });
        let Some(dialog) = self.ui_state.image_cache.dialog.as_deref_mut() else {
            return;
        };
        dialog.chosen = Some(index);
        if let Some(picked) = dialog.chosen_pick() {
            let natural = picked.facts.natural_size_mm();
            let (width, _) = first_size_mm(natural, free.unwrap_or(natural));
            dialog.width = format!("{width:.2}");
        }
    }

    pub(super) fn dispatch_place_dialog(&mut self, msg: PlaceDialogMsg) -> Task<Message> {
        if self.ui_state.image_cache.dialog.is_none() {
            return Task::none();
        }
        match msg {
            PlaceDialogMsg::Listed { target, files } => {
                let Some(dialog) = self.ui_state.image_cache.dialog.as_deref_mut() else {
                    return Task::none();
                };
                if dialog.target != target {
                    return Task::none();
                }
                dialog.loading = false;
                let added = std::mem::take(&mut dialog.files);
                dialog.files = files
                    .into_iter()
                    .map(|(file, read)| DialogFile::of(file, read))
                    .collect();
                // A file Add image file… brought in while the folder was read.
                for extra in added {
                    if !dialog.files.iter().any(|f| f.file == extra.file) {
                        dialog.files.push(extra);
                    }
                }
                if dialog.chosen.is_none() && !dialog.files.is_empty() {
                    self.choose_in_dialog(0);
                }
            }
            PlaceDialogMsg::FilterChanged(text) => {
                if let Some(dialog) = self.ui_state.image_cache.dialog.as_deref_mut() {
                    dialog.filter = text;
                }
            }
            PlaceDialogMsg::Chosen(index) => self.choose_in_dialog(index),
            PlaceDialogMsg::WidthTyped(text) => {
                if let Some(dialog) = self.ui_state.image_cache.dialog.as_deref_mut() {
                    dialog.width = text;
                    dialog.at_file_size = false;
                }
            }
            PlaceDialogMsg::FileSizePicked(on) => {
                if let Some(dialog) = self.ui_state.image_cache.dialog.as_deref_mut() {
                    dialog.at_file_size = on;
                }
            }
            PlaceDialogMsg::AddFilePressed => return self.place_image_dialog(),
            PlaceDialogMsg::PlacePressed => self.place_from_dialog(),
            PlaceDialogMsg::Closed => self.ui_state.image_cache.dialog = None,
        }
        Task::none()
    }

    /// Place: put the chosen image on the cursor at the chosen width and
    /// close the dialog. Refused, with the dialog left open, while the
    /// width does not read or the chosen file was refused.
    fn place_from_dialog(&mut self) {
        let Some(dialog) = self.ui_state.image_cache.dialog.as_deref() else {
            return;
        };
        let (Some(picked), Some(width)) = (dialog.chosen_pick().cloned(), dialog.placed_width())
        else {
            return;
        };
        let target = dialog.target.clone();
        if self.document_state.active_path.as_deref() != Some(target.as_path()) {
            self.ui_state.image_cache.dialog = None;
            crate::diagnostics::log_info(
                t!(self.ui_state.i18n, ids::IMAGES_CANCELLED).into_string(),
            );
            return;
        }
        match self.image_fragment(&picked, Some(width), &target) {
            Ok(copied) => {
                self.ui_state.image_cache.dialog = None;
                self.note_image_copy(&target, &picked);
                self.arm_fragment(copied, &target);
            }
            Err(reason) => crate::diagnostics::log_warning(
                t!(self.ui_state.i18n, ids::IMAGES_REFUSED, "file" => picked.picked_name.as_str(), "reason" => reason)
                    .into_string(),
            ),
        }
    }
}
