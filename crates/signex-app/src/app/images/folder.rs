//! Show in folder (#1116, mockup row 204): the file manager opens on a
//! placed image's file. The launch blocks, so it runs in a `Task`; a
//! headless run launches nothing and `show_image_in_folder` answers with
//! the paths.

use std::path::PathBuf;

use iced::Task;
use serde_json::{Value, json};
use signex_i18n::{ids, t};
use uuid::Uuid;

use super::ImagesMsg;
use crate::app::{Message, Signex};

impl Signex {
    /// The file of image `id` on the sheet an edit acts on, absolute, and
    /// the folder it is in.
    pub(crate) fn image_file_path(&self, id: Uuid) -> Result<(PathBuf, PathBuf), String> {
        let image = self.image_on_edit_sheet(id)?;
        let sheet = self
            .document_state
            .edit_path()
            .ok_or("no schematic is open")?
            .to_path_buf();
        let relative = signex_types::image_asset::checked_asset_path(&image.file)
            .map_err(|refusal| refusal.to_string())?;
        let path = self.image_root(&sheet).join(relative);
        let folder = path
            .parent()
            .map(std::path::Path::to_path_buf)
            .unwrap_or_default();
        Ok((path, folder))
    }

    /// What `show_image_in_folder` answers: the paths, and whether the file
    /// manager is asked to show them.
    pub(crate) fn image_folder_answer(&self, id: Uuid) -> Result<Value, String> {
        let (path, folder) = self.image_file_path(id)?;
        let shown = self.ui_state.io.opener == crate::io_ports::Opener::Native;
        Ok(json!({
            "id": id.to_string(),
            "path": path,
            "folder": folder,
            "shown": shown,
        }))
    }

    /// Show image `id`'s file in the file manager, off the UI thread.
    pub(crate) fn show_image_in_folder(&mut self, id: Uuid) -> Task<Message> {
        let (path, _) = match self.image_file_path(id) {
            Ok(paths) => paths,
            Err(reason) => {
                self.folder_shown(Err(reason));
                return Task::none();
            }
        };
        let opener = self.ui_state.io.opener;
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    opener.reveal(&path).map_err(|error| error.to_string())
                })
                .await
                .unwrap_or_else(|error| Err(error.to_string()))
            },
            |result| ImagesMsg::FolderShown(result).into(),
        )
    }

    /// The launch finished: a failure is said in Messages.
    pub(super) fn folder_shown(&mut self, result: Result<(), String>) {
        if let Err(reason) = result {
            crate::diagnostics::log_warning(
                t!(self.ui_state.i18n, ids::IMAGES_FOLDER_FAILED, "reason" => reason).into_string(),
            );
        }
    }
}
