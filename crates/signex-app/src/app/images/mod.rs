//! Images placed on a sheet as project assets (#1116, mockup row 204):
//! Place ▸ Graphic… opens the Place Image dialog on the project's
//! `assets/images/`, reads and checks the files off the UI thread and puts
//! the chosen image on the cursor; one click places it as one undo step. The
//! sheet stores the project-relative path and the content hash, never the
//! picture. A file picked from outside the project is copied into
//! `assets/images/` when the sheet is saved, and nothing is written before
//! that. The canvas reads the pictures in a `Task` and draws a placeholder
//! for a missing one; Properties ▸ Image, the resize handles, Relink, the
//! Projects tree's assets and the status bar count work on the same state.
//! Rules: docs/SCHEMATIC_IMAGES.md.

pub(crate) mod assets;
mod cache;
mod copy;
pub(crate) mod dialog;
pub(crate) mod edit;
pub(crate) mod export;
mod folder;
mod panel;
mod place;
pub(crate) mod read;
pub(crate) mod relink;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_assets;
#[cfg(test)]
mod tests_edit;
#[cfg(test)]
mod tests_relink;

use std::path::PathBuf;
use std::sync::Arc;

use iced::Task;
use signex_i18n::{I18n, ids, t};
use signex_types::image_asset::{ImageFormat, ImageReadError, ImageRefusal, PendingImageCopy};
use signex_types::image_asset_report::AssetReport;

pub(crate) use assets::{pending_image_keys, with_missing_images};
pub(crate) use cache::{ImageCache, ImageEntry, ImageKey};
pub use dialog::PlaceDialogMsg;
pub use panel::ImagePanelMsg;
pub use read::CopyFailure;
pub use relink::{RelinkRead, RelinkScope};

use crate::app::agent::WaitingCommand;
use crate::app::{Message, Signex};

impl From<ImagesMsg> for Message {
    fn from(msg: ImagesMsg) -> Self {
        Self::Edit(crate::app::EditMsg::Images(msg))
    }
}

/// What happened to a placed image or its file.
#[derive(Debug, Clone)]
pub enum ImagesMsg {
    /// Place ▸ Graphic… was chosen.
    PlaceRequested,
    /// The file dialog answered; `None` when it was cancelled.
    FilePicked(Option<PathBuf>),
    /// The picked file was read and checked off the UI thread for the sheet
    /// `target`, asked for at placement epoch `epoch`.
    FileRead {
        path: PathBuf,
        target: PathBuf,
        epoch: u64,
        read: Result<Arc<signex_types::image_asset::PickedImage>, ImageReadError>,
    },
    /// The picture of a placed image on `sheet` was read for the canvas.
    PictureRead {
        key: ImageKey,
        sheet: PathBuf,
        read: Result<(ImageFormat, Arc<[u8]>), ImageReadError>,
    },
    /// The copies a save made into the project landed, each with its result.
    CopiesWritten(Vec<(PendingImageCopy, Result<(), CopyFailure>)>),
    /// Something happened in the Place Image dialog.
    Dialog(PlaceDialogMsg),
    /// A scan of the assets of `root` landed, for the key it was started
    /// for; `waiting` is the `list_image_assets` it answers, and its sheet.
    AssetsScanned {
        root: PathBuf,
        key: u64,
        report: Result<Arc<AssetReport>, String>,
        waiting: Option<Box<(WaitingCommand, PathBuf)>>,
    },
    /// The file dialog of Relink… or Replace… answered.
    RelinkPicked {
        scope: RelinkScope,
        path: Option<PathBuf>,
    },
    /// The picked file and the project's sheets were read for a relink of
    /// images on `target`, at its engine `revision`.
    RelinkRead {
        scope: RelinkScope,
        target: PathBuf,
        revision: Option<u64>,
        read: Box<RelinkRead>,
    },
    /// A Show in folder launch finished.
    FolderShown(Result<(), String>),
    /// A handle drag dropped the image `id` with this box (top-left, size).
    HandleDropped {
        id: uuid::Uuid,
        position: signex_types::schematic::Point,
        size: (f64, f64),
    },
    /// A row of the Projects tree's `assets/images/` was picked: put that
    /// project file on the cursor over the active sheet.
    AssetPicked { file: String },
}

impl Signex {
    pub(crate) fn dispatch_images(&mut self, msg: ImagesMsg) -> Task<Message> {
        let task = match msg {
            ImagesMsg::PlaceRequested => self.open_place_dialog(),
            ImagesMsg::FilePicked(path) => self.read_picked_file(path),
            ImagesMsg::FileRead {
                path,
                target,
                epoch,
                read,
            } => {
                self.picked_file_read(&path, &target, epoch, read);
                Task::none()
            }
            ImagesMsg::PictureRead { key, sheet, read } => {
                self.picture_read(key, &sheet, read);
                Task::none()
            }
            ImagesMsg::CopiesWritten(results) => {
                self.image_copies_written(results);
                Task::none()
            }
            ImagesMsg::Dialog(msg) => self.dispatch_place_dialog(msg),
            ImagesMsg::AssetsScanned {
                root,
                key,
                report,
                waiting,
            } => self.assets_scanned(root, key, report, waiting),
            ImagesMsg::RelinkPicked { scope, path } => self.relink_picked(scope, path),
            ImagesMsg::RelinkRead {
                scope,
                target,
                revision,
                read,
            } => {
                self.relink_read(&scope, &target, revision, *read);
                Task::none()
            }
            ImagesMsg::FolderShown(result) => {
                self.folder_shown(result);
                Task::none()
            }
            ImagesMsg::HandleDropped { id, position, size } => {
                self.image_handle_dropped(id, position, size);
                Task::none()
            }
            ImagesMsg::AssetPicked { file } => self.asset_picked(&file),
        };
        Task::batch([task, self.finish_update()])
    }
}

/// Why a file was not used, in the person's language.
pub(crate) fn read_error_text(i18n: &I18n, error: &ImageReadError) -> String {
    match error {
        ImageReadError::Refused(refusal) => refusal_text(i18n, refusal),
        ImageReadError::Unreadable(why) => {
            t!(i18n, ids::IMAGES_REFUSAL_UNREADABLE, "error" => why.as_str()).into_string()
        }
        ImageReadError::Changed => t!(i18n, ids::IMAGES_REFUSAL_CHANGED).into_string(),
    }
}

/// A broken rule of the image checks, in the person's language.
pub(crate) fn refusal_text(i18n: &I18n, refusal: &ImageRefusal) -> String {
    let text = match refusal {
        ImageRefusal::EmptyPath => t!(i18n, ids::IMAGES_REFUSAL_EMPTY_PATH),
        ImageRefusal::AbsolutePath(_) => t!(i18n, ids::IMAGES_REFUSAL_ABSOLUTE),
        ImageRefusal::LeavesProject(_) => t!(i18n, ids::IMAGES_REFUSAL_LEAVES_PROJECT),
        ImageRefusal::TooLarge { bytes, limit } => {
            t!(i18n, ids::IMAGES_REFUSAL_TOO_LARGE, "bytes" => *bytes, "limit" => *limit)
        }
        ImageRefusal::UnknownFormat => t!(i18n, ids::IMAGES_REFUSAL_UNKNOWN_FORMAT),
        ImageRefusal::NoSize => t!(i18n, ids::IMAGES_REFUSAL_NO_SIZE),
        ImageRefusal::TooManyPixels { width, height } => {
            t!(i18n, ids::IMAGES_REFUSAL_TOO_MANY_PIXELS, "width" => *width, "height" => *height)
        }
        ImageRefusal::ExternalReference(reference) => {
            t!(i18n, ids::IMAGES_REFUSAL_EXTERNAL, "reference" => reference.as_str())
        }
    };
    text.into_string()
}
