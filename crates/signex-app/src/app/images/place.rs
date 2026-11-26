//! Place ▸ Graphic… (#1116): the file dialog, the read off the UI thread,
//! and the image on the cursor. The operand form shares
//! [`Signex::image_fragment`] and [`Signex::note_image_copy`]
//! (`command/image_operands.rs`).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use iced::Task;
use signex_engine::CopiedSelection;
use signex_i18n::{ids, t};
use signex_types::image_asset::{IMAGE_ASSET_DIR, ImageReadError, PendingImageCopy, PickedImage};
use signex_types::schematic::{Point, SchDrawing, SchImage, first_size_mm, free_area_mm};

use super::ImagesMsg;
use crate::app::{Message, Signex};

/// The file kinds the dialog offers.
const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "svg"];

impl Signex {
    /// The sheet a picked image lands on: the active schematic, when it
    /// takes an edit. Says why not in the Messages panel.
    fn image_target(&mut self) -> Option<PathBuf> {
        if !self.has_active_schematic() {
            crate::diagnostics::log_warning(
                t!(self.ui_state.i18n, ids::IMAGES_NOT_ON_SHEET).into_string(),
            );
            return None;
        }
        if let Some(reason) = self.refuse_if_active_read_only() {
            crate::diagnostics::log_warning(reason);
            return None;
        }
        self.document_state.active_path.clone()
    }

    /// Ask for the image file to place.
    pub(super) fn place_image_dialog(&mut self) -> Task<Message> {
        if self.image_target().is_none() {
            return Task::none();
        }
        let i18n = &self.ui_state.i18n;
        let dialog = self
            .ui_state
            .io
            .picker
            .dialog()
            .title_in(i18n, ids::IMAGES_DIALOG_TITLE)
            .filter_in(i18n, ids::IMAGES_FILTER, IMAGE_EXTENSIONS)
            .filter_in(i18n, ids::FILE_FILTER_ALL_FILES, &["*"]);
        Task::perform(dialog.pick_file(), |path| {
            ImagesMsg::FilePicked(path).into()
        })
    }

    /// The names, with their hashes, that picks not saved yet took under
    /// the images folder of `root`.
    pub(crate) fn pending_image_names(&self, root: &Path) -> Vec<(String, String)> {
        self.document_state
            .image_copies
            .iter()
            .filter(|copy| copy.root == root)
            .filter_map(|copy| {
                let name = copy.file.strip_prefix(IMAGE_ASSET_DIR)?.strip_prefix('/')?;
                Some((name.to_string(), copy.hash.clone()))
            })
            .collect()
    }

    /// Read and check the picked file off the UI thread.
    pub(super) fn read_picked_file(&mut self, path: Option<PathBuf>) -> Task<Message> {
        let Some(path) = path else {
            return Task::none();
        };
        let Some(target) = self.image_target() else {
            return Task::none();
        };
        let epoch = self.interaction_state.placement_epoch;
        let root = self.image_root(&target);
        let pending = self.pending_image_names(&root);
        Task::perform(
            async move {
                let read_path = path.clone();
                let read = tokio::task::spawn_blocking(move || {
                    super::read::read_picked_image(&read_path, &root, &pending).map(Arc::new)
                })
                .await
                .unwrap_or_else(|error| Err(ImageReadError::Unreadable(error.to_string())));
                (path, read)
            },
            move |(path, read)| {
                ImagesMsg::FileRead {
                    path,
                    target,
                    epoch,
                    read,
                }
                .into()
            },
        )
    }

    /// The picked file was read: put the image on the cursor, or say why not.
    /// A read that lands after Esc, a tool change or a sheet switch is
    /// dropped.
    pub(super) fn picked_file_read(
        &mut self,
        path: &Path,
        target: &Path,
        epoch: u64,
        read: Result<Arc<PickedImage>, ImageReadError>,
    ) {
        let current = epoch == self.interaction_state.placement_epoch
            && self.document_state.active_path.as_deref() == Some(target);
        if !current {
            crate::diagnostics::log_info(
                t!(self.ui_state.i18n, ids::IMAGES_CANCELLED).into_string(),
            );
            return;
        }
        let placed = read
            .map_err(|error| super::read_error_text(&self.ui_state.i18n, &error))
            .and_then(|picked| {
                let copied = self.image_fragment(&picked, None, target)?;
                Ok((picked, copied))
            });
        match placed {
            Ok((picked, copied)) => {
                self.note_image_copy(target, &picked);
                self.arm_fragment(copied, target);
            }
            Err(reason) => {
                let file = path.file_name().map_or_else(
                    || path.display().to_string(),
                    |name| name.to_string_lossy().into_owned(),
                );
                crate::diagnostics::log_warning(
                    t!(self.ui_state.i18n, ids::IMAGES_REFUSED, "file" => file, "reason" => reason)
                        .into_string(),
                );
            }
        }
    }

    /// `picked` as cursor content for the sheet `target`: an image `width`
    /// mm wide, else its first size on that sheet (mockup row 204), the
    /// height following the file's aspect ratio. Anchored at its centre.
    pub(crate) fn image_fragment(
        &self,
        picked: &PickedImage,
        width: Option<f64>,
        target: &Path,
    ) -> Result<CopiedSelection, String> {
        let natural = picked.facts.natural_size_mm();
        let size = match width {
            Some(width) => (width, width * natural.1 / natural.0),
            None => {
                let free = self
                    .document_state
                    .engines
                    .get(target)
                    .map_or(natural, |engine| free_area_mm(engine.document()));
                first_size_mm(natural, free)
            }
        };
        let mut image = SchImage::new(
            uuid::Uuid::now_v7(),
            Point::new(0.0, 0.0),
            size,
            picked.file.clone(),
            picked.facts.hash.clone(),
        );
        image.source_px = Some([picked.facts.width_px, picked.facts.height_px]);
        CopiedSelection::from_drawings(
            vec![SchDrawing::Image(image)],
            self.fragment_grid_mm(),
            &mut signex_types::object_id::UuidV7Ids,
        )
        .map_err(|error| error.to_string())?
        .ok_or_else(|| picked.file.clone())
    }

    /// Remember that saving `sheet` copies `picked` into its project; a pick
    /// already in the project needs no copy.
    pub(crate) fn note_image_copy(&mut self, sheet: &Path, picked: &PickedImage) {
        if !picked.needs_copy {
            return;
        }
        let copies = &mut self.document_state.image_copies;
        let known = copies.iter().any(|copy| {
            copy.sheet == sheet && copy.file == picked.file && copy.hash == picked.facts.hash
        });
        if !known {
            copies.push(PendingImageCopy {
                sheet: sheet.to_path_buf(),
                root: picked.root.clone(),
                file: picked.file.clone(),
                hash: picked.facts.hash.clone(),
                bytes: Arc::clone(&picked.bytes),
            });
        }
    }
}
