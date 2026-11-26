//! Images placed on a sheet as project assets (#1116, mockup row 204):
//! Place ▸ Graphic… asks for a PNG, JPEG, GIF or SVG file, reads and checks
//! it off the UI thread and puts the image on the cursor; one click places
//! it as one undo step. The sheet stores the project-relative path and the
//! content hash, never the picture. A file picked from outside the project
//! is copied into `assets/images/` when the sheet is saved, and nothing is
//! written before that. The canvas reads the pictures in a `Task` and draws
//! a placeholder for a missing one. Rules: docs/SCHEMATIC_IMAGES.md.

mod cache;
mod copy;
mod place;
pub(crate) mod read;
#[cfg(test)]
mod tests;

use std::path::PathBuf;
use std::sync::Arc;

use iced::Task;
use signex_i18n::{I18n, ids, t};
use signex_types::image_asset::{ImageFormat, ImageReadError, ImageRefusal, PendingImageCopy};

pub(crate) use cache::{ImageCache, ImageEntry, ImageKey};
pub use read::CopyFailure;

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
}

impl Signex {
    pub(crate) fn dispatch_images(&mut self, msg: ImagesMsg) -> Task<Message> {
        let task = match msg {
            ImagesMsg::PlaceRequested => self.place_image_dialog(),
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
