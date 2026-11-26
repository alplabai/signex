//! The copy into the project on save (#1116, mockup row 204): a picked file
//! from outside the project is written to `assets/images/` when the sheet
//! that places it is saved, and nothing is written before that. The copy
//! runs in a `Task` beside the sheet's own write; a copy that fails is kept
//! and runs again on the next save, and a file already there with other
//! content is never replaced.

use std::collections::HashSet;
use std::path::Path;

use iced::Task;
use signex_i18n::{ids, t};
use signex_types::image_asset::PendingImageCopy;
use signex_types::schematic::SchDrawing;

use super::ImagesMsg;
use super::read::{CopyFailure, write_image_copy};
use crate::app::{Message, Signex};

/// Write each copy, blocking: run it off the UI thread.
pub(crate) fn write_image_copies(
    copies: Vec<PendingImageCopy>,
) -> Vec<(PendingImageCopy, Result<(), CopyFailure>)> {
    copies
        .into_iter()
        .map(|copy| {
            let result = write_image_copy(&copy);
            (copy, result)
        })
        .collect()
}

impl Signex {
    /// The copies saving `sheet` makes: the picks waiting for this sheet
    /// that one of its images still shows, taken out of the waiting list. A
    /// pick no image shows (the placement was cancelled or undone) stays
    /// waiting, so a redo still finds it.
    pub(crate) fn take_image_copies(&mut self, sheet: &Path) -> Vec<PendingImageCopy> {
        let Some(engine) = self.document_state.engines.get(sheet) else {
            return Vec::new();
        };
        let shown: HashSet<(&str, &str)> = engine
            .document()
            .drawings
            .iter()
            .filter_map(|d| match d {
                SchDrawing::Image(image) => Some((image.file.as_str(), image.hash.as_str())),
                _ => None,
            })
            .collect();
        let (taken, kept): (Vec<_>, Vec<_>) = std::mem::take(&mut self.document_state.image_copies)
            .into_iter()
            .partition(|copy| {
                copy.sheet == sheet && shown.contains(&(copy.file.as_str(), copy.hash.as_str()))
            });
        self.document_state.image_copies = kept;
        taken
    }

    /// Start the copies a save of `sheet` makes, off the UI thread.
    pub(crate) fn begin_image_copies(&mut self, sheet: &Path) -> Task<Message> {
        let copies = self.take_image_copies(sheet);
        if copies.is_empty() {
            return Task::none();
        }
        let held = copies.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || write_image_copies(copies))
                    .await
                    .unwrap_or_else(|error| {
                        // The writer stopped: every copy waits for the next save.
                        held.into_iter()
                            .map(|copy| (copy, Err(CopyFailure::Io(error.to_string()))))
                            .collect()
                    })
            },
            |results| ImagesMsg::CopiesWritten(results).into(),
        )
    }

    /// The copies landed: say each in the Messages panel, keep a failed one
    /// waiting, and read again the pictures a copy may have brought.
    pub(super) fn image_copies_written(
        &mut self,
        results: Vec<(PendingImageCopy, Result<(), CopyFailure>)>,
    ) {
        let mut written = false;
        for (copy, result) in results {
            let i18n = &self.ui_state.i18n;
            let file = copy.file.as_str();
            match result {
                Ok(()) => {
                    written = true;
                    crate::diagnostics::log_info(
                        t!(i18n, ids::IMAGES_COPIED, "file" => file).into_string(),
                    );
                }
                Err(CopyFailure::Exists) => crate::diagnostics::log_warning(
                    t!(i18n, ids::IMAGES_COPY_EXISTS, "file" => file).into_string(),
                ),
                Err(CopyFailure::Refused(refusal)) => {
                    let error = super::refusal_text(i18n, &refusal);
                    crate::diagnostics::log_warning(
                        t!(i18n, ids::IMAGES_COPY_FAILED, "file" => file, "error" => error)
                            .into_string(),
                    );
                }
                Err(CopyFailure::Io(error)) => {
                    crate::diagnostics::log_warning(
                        t!(i18n, ids::IMAGES_COPY_FAILED, "file" => file, "error" => error)
                            .into_string(),
                    );
                    self.document_state.image_copies.push(copy);
                }
            }
        }
        if written {
            self.ui_state.image_cache.retry_missing();
        }
    }
}
