//! Pinned datasheet PDFs: picking one and saving the row that pins it
//! (#1332).
//!
//! A pick reads, validates and hashes the file on a worker and only stages
//! the bytes on the editor: nothing is written before the user saves. The
//! Save button then writes the staged PDFs to the library on a worker
//! (`assets/datasheets/<sha256>.pdf`, atomic), and only once they are on
//! disk saves the row, whose commit takes the PDFs along in a git-backed
//! library. The folder-rename gate covers that write like any document
//! save: it is refused while the project moves and is counted as in flight
//! until it finishes.

use std::path::Path;

use signex_library::PdfAsset;

use super::*;
use crate::library::messages::{DatasheetMsg, PickedFile, PickedPdf};

/// Validate and hash a picked file's bytes. CPU work: call from a worker.
pub(super) fn picked_file(bytes: Vec<u8>, filename: String) -> Result<PickedFile, String> {
    let asset = PdfAsset::from_bytes(bytes).map_err(|e| format!("{filename}: {e}"))?;
    Ok(PickedFile { asset, filename })
}

/// Paths, relative to the library root, of `assets` as git names them.
pub(super) fn asset_rel_paths(assets: &[PdfAsset]) -> Vec<String> {
    assets
        .iter()
        .filter_map(|a| signex_library::datasheet_asset_rel_path(&a.hash))
        .collect()
}

/// Write `assets` beside `library_file` (atomic, content-addressed) and
/// return their root-relative paths. Blocking IO: call from a worker.
pub(super) fn store_assets(
    library_file: &Path,
    assets: &[PdfAsset],
) -> Result<Vec<String>, String> {
    for asset in assets {
        let stored = signex_library::store_datasheet(library_file, &asset.bytes)
            .map_err(|e| format!("The datasheet PDF could not be stored: {e}"))?;
        if stored != asset.hash {
            return Err("The datasheet PDF changed while it was being stored".into());
        }
    }
    Ok(asset_rel_paths(assets))
}

impl Signex {
    /// Open the file dialog for document `index`; the result carries the
    /// document as it is now, so it finds it again if the list changes
    /// while the dialog is open.
    pub(super) fn begin_datasheet_pick(
        &mut self,
        address: &EditorAddress,
        index: usize,
    ) -> Task<Message> {
        let Some(picked_for) = self
            .library
            .editors
            .get(address)
            .and_then(|state| state.row.documents().into_iter().nth(index))
        else {
            return Task::none();
        };
        let address = address.clone();
        Task::perform(pick_pdf(), move |file| {
            Message::Library(LibraryMessage::EditorEvent {
                library_path: address.library_path.clone(),
                table: address.table.clone(),
                row_id: address.row_id,
                msg: EditorMsg::Datasheet(DatasheetMsg::UploadResult(Box::new(PickedPdf {
                    index,
                    picked_for: picked_for.clone(),
                    file,
                }))),
            })
        })
    }

    /// Start saving the row: at once when it pins no staged PDF, else
    /// after the PDFs are written on a worker.
    pub(super) fn begin_save_row(
        &mut self,
        address: &EditorAddress,
    ) -> anyhow::Result<Task<Message>> {
        let Some(state) = self.library.editors.get(address) else {
            return Ok(Task::none());
        };
        let staged = state.staged_assets_in_use();
        if staged.is_empty() {
            self.try_save_row(address)?;
            return Ok(Task::none());
        }
        let library_path = state.library_path.clone();
        self.refuse_while_project_moves(&library_path, "save")?;
        let in_flight = self
            .document_state
            .document_saves
            .begin_work(library_path.clone());
        let address = address.clone();
        let file = library_path.clone();
        Ok(Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let _in_flight = in_flight;
                    store_assets(&file, &staged)
                })
                .await
                .map_err(|join_error| format!("the datasheet store task failed: {join_error}"))
                .and_then(|result| result)
            },
            move |result| {
                Message::Library(LibraryMessage::EditorEvent {
                    library_path: address.library_path.clone(),
                    table: address.table.clone(),
                    row_id: address.row_id,
                    msg: EditorMsg::Datasheet(DatasheetMsg::AssetsStored(result)),
                })
            },
        ))
    }

    /// Write the staged PDFs on this thread, for the saves that must have
    /// everything on disk before they go on (Save All, quit, close).
    pub(super) fn store_staged_assets_blocking(
        &mut self,
        address: &EditorAddress,
    ) -> anyhow::Result<Vec<String>> {
        let Some(state) = self.library.editors.get(address) else {
            return Ok(Vec::new());
        };
        let staged = state.staged_assets_in_use();
        if staged.is_empty() {
            return Ok(Vec::new());
        }
        let library_path = state.library_path.clone();
        self.refuse_while_project_moves(&library_path, "save")?;
        store_assets(&library_path, &staged).map_err(anyhow::Error::msg)
    }

    /// The worker finished writing the staged PDFs: release the in-flight
    /// mark, then save the row, or report why a PDF was not stored (the
    /// row stays dirty and unsaved).
    pub(super) fn finish_staged_asset_store(
        &mut self,
        address: &EditorAddress,
        result: Result<Vec<String>, String>,
    ) {
        self.document_state
            .document_saves
            .finish(&address.library_path);
        let saved = result
            .map_err(anyhow::Error::msg)
            .and_then(|written| self.commit_row(address, &written));
        if let Err(e) = saved {
            self.report_save_row_failure(&e);
        }
    }
}

/// The file dialog, then the read and the validation of the chosen file.
/// `None` when the dialog is cancelled.
async fn pick_pdf() -> Option<Result<PickedFile, String>> {
    let handle = rfd::AsyncFileDialog::new()
        .set_title("Pin datasheet PDF")
        .add_filter("PDF", &["pdf"])
        .pick_file()
        .await?;
    let filename = handle.file_name();
    let bytes = handle.read().await;
    let checked = tokio::task::spawn_blocking(move || picked_file(bytes, filename))
        .await
        .unwrap_or_else(|join_error| Err(format!("the PDF check failed: {join_error}")));
    Some(checked)
}

#[cfg(test)]
mod tests;
