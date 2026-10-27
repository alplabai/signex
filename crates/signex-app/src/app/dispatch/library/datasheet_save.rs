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
    refuse_oversize(bytes.len() as u64, &filename)?;
    let asset = PdfAsset::from_bytes(bytes).map_err(|e| format!("{filename}: {e}"))?;
    Ok(PickedFile { asset, filename })
}

/// A file over [`signex_library::MAX_DATASHEET_BYTES`] is refused: the bytes
/// are held in memory until the row is saved.
fn refuse_oversize(len: u64, filename: &str) -> Result<(), String> {
    if len > signex_library::MAX_DATASHEET_BYTES {
        return Err(format!(
            "{filename}: this file is larger than {} MB",
            signex_library::MAX_DATASHEET_BYTES / (1024 * 1024)
        ));
    }
    Ok(())
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

    /// Refuse before anything is written when the library cannot hold the
    /// staged PDFs: read-only, or an adapter that keeps no files beside
    /// its library file (a server library).
    fn refuse_unstorable(&self, library_path: &Path) -> anyhow::Result<()> {
        if let Some(error) = self.read_only_write_error(library_path) {
            return Err(error);
        }
        let adapter = self
            .library
            .library_at(library_path)
            .and_then(|lib| self.library.set.get(lib.library_id));
        if adapter.is_some_and(|a| a.root_dir().is_none()) {
            anyhow::bail!(
                "this library cannot store pinned datasheet PDFs: it keeps no files beside \
                 its library file. Use a URL datasheet instead"
            );
        }
        Ok(())
    }

    /// Start saving the row: at once when it pins no staged PDF, else
    /// after the PDFs are written on a worker. A second Save while that
    /// write runs does nothing.
    pub(super) fn begin_save_row(
        &mut self,
        address: &EditorAddress,
    ) -> anyhow::Result<Task<Message>> {
        let Some(state) = self.library.editors.get(address) else {
            return Ok(Task::none());
        };
        if state.saving_row.is_some() {
            return Ok(Task::none());
        }
        let staged = state.staged_assets_in_use();
        if staged.is_empty() {
            self.try_save_row(address)?;
            return Ok(Task::none());
        }
        let library_path = state.library_path.clone();
        self.refuse_unstorable(&library_path)?;
        self.refuse_while_project_moves(&library_path, "save")?;
        // A typed number still in its edit buffer is part of this save;
        // the copy saved is the row as it is now.
        let Some(state) = self.library.editors.get_mut(address) else {
            return Ok(Task::none());
        };
        crate::library::component_preview::updates::flush_edit_buffers(state)
            .map_err(anyhow::Error::msg)?;
        state.saving_row = Some(state.row.clone());
        let staged = state.staged_assets_in_use();
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
        if state.saving_row.is_some() {
            anyhow::bail!("this row is still being saved; try again in a moment");
        }
        let staged = state.staged_assets_in_use();
        if staged.is_empty() {
            return Ok(Vec::new());
        }
        let library_path = state.library_path.clone();
        self.refuse_unstorable(&library_path)?;
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
        // The save this store belonged to is gone when its editor was
        // closed (and maybe reopened) meanwhile: the reopened editor holds
        // a different live row that was never saved, so it is not
        // committed. The written PDFs stay beside the library, unreferenced.
        let still_saving = self
            .library
            .editors
            .get(address)
            .is_some_and(|state| state.saving_row.is_some());
        if !still_saving {
            if let Err(e) = result {
                self.report_save_row_failure(&anyhow::Error::msg(e));
            }
            return;
        }
        let saved = result
            .inspect_err(|_| {
                if let Some(state) = self.library.editors.get_mut(address) {
                    state.saving_row = None;
                }
            })
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
    // Refused from the file's size, before it is read into memory.
    if let Ok(meta) = tokio::fs::metadata(handle.path()).await
        && let Err(reason) = refuse_oversize(meta.len(), &filename)
    {
        return Some(Err(reason));
    }
    let bytes = handle.read().await;
    let checked = tokio::task::spawn_blocking(move || picked_file(bytes, filename))
        .await
        .unwrap_or_else(|join_error| Err(format!("the PDF check failed: {join_error}")));
    Some(checked)
}

#[cfg(test)]
mod tests;
