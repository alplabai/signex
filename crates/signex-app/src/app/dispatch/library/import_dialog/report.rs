//! The loss report's actions (mockup row 211, "Done: loss report", #3121):
//! Save report… copies the library's `import-report.md` to a file the user
//! picks, Open report opens it in the system viewer, Open in Library
//! Browser opens the library. The Library Browser's library menu opens the
//! same view on a library's stored `import-report.json`. Every read, copy
//! and launch runs on a worker.

use std::path::{Path, PathBuf};

use super::*;
use crate::library::import_dialog::result::ReportNote;
use crate::library_import::ImportReport;
use crate::library_import::report::{REPORT_MD, read_stored_report, save_stored_report};

impl Signex {
    /// Save report…: ask where; nothing is written until a file is picked.
    pub(super) fn pick_import_report_target(&mut self) -> Task<Message> {
        let Some(result) = self
            .library
            .import_dialog
            .as_deref()
            .and_then(|d| d.finished())
        else {
            return Task::none();
        };
        let name = format!("{}-{REPORT_MD}", result.library_name());
        let picker = self
            .ui_state
            .io
            .picker
            .dialog()
            .title_in(
                &self.ui_state.i18n,
                ids::LIBRARY_IMPORT_DIALOG_SAVE_REPORT_TITLE,
            )
            .set_file_name(name);
        Task::perform(picker.save_file(), |target| {
            dialog_message(ImportDialogMsg::ReportSavePicked(target))
        })
    }

    /// Copy the readable report to `target` on a worker, atomically.
    pub(super) fn save_import_report(&mut self, target: Option<PathBuf>) -> Task<Message> {
        let Some(target) = target else {
            return Task::none();
        };
        let Some(dir) = self
            .library
            .import_dialog
            .as_deref()
            .and_then(|d| d.finished())
            .map(|r| r.library_dir.clone())
        else {
            return Task::none();
        };
        Task::perform(
            async move {
                let written = target.clone();
                let saved = tokio::task::spawn_blocking(move || save_stored_report(&dir, &written))
                    .await
                    .unwrap_or_else(|e| Err(e.to_string()));
                match saved {
                    Ok(()) => Ok(target),
                    Err(why) => Err((target, why)),
                }
            },
            |saved| dialog_message(ImportDialogMsg::ReportSaved(saved)),
        )
    }

    pub(super) fn import_report_saved(
        &mut self,
        saved: Result<PathBuf, (PathBuf, String)>,
    ) -> Task<Message> {
        let note = match saved {
            Ok(path) => ReportNote::Saved(path),
            Err((path, why)) => {
                log::warn!(
                    "Library import: report not saved to {}: {why}",
                    path.display()
                );
                ReportNote::Failed(
                    Translatable::new(ids::LIBRARY_IMPORT_DIALOG_REPORT_SAVE_FAILED)
                        .arg("path", path.display().to_string())
                        .arg("error", why),
                )
            }
        };
        self.edit_import_dialog(|d| d.report_note = Some(note))
    }

    /// Open report: the readable report in the system viewer.
    pub(super) fn open_import_report(&mut self) -> Task<Message> {
        let Some(md) = self
            .library
            .import_dialog
            .as_deref()
            .and_then(|d| d.finished())
            .map(|r| r.report_md())
        else {
            return Task::none();
        };
        let opener = self.ui_state.io.opener;
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || opener.view(&md).map_err(|e| e.to_string()))
                    .await
                    .unwrap_or_else(|e| Err(e.to_string()))
            },
            |opened| dialog_message(ImportDialogMsg::ReportOpened(opened)),
        )
    }

    pub(super) fn import_report_opened(&mut self, opened: Result<(), String>) -> Task<Message> {
        let Err(why) = opened else {
            return Task::none();
        };
        self.edit_import_dialog(|d| {
            let path = d.finished().map(|r| r.report_md()).unwrap_or_default();
            log::warn!("Library import: could not open {}: {why}", path.display());
            d.report_note = Some(ReportNote::Failed(
                Translatable::new(ids::LIBRARY_IMPORT_DIALOG_REPORT_OPEN_FAILED)
                    .arg("path", path.display().to_string())
                    .arg("error", why),
            ));
        })
    }

    /// Open in Library Browser: the dialog closes and the library's
    /// browser tab opens.
    pub(super) fn open_imported_library(&mut self) -> Task<Message> {
        let file = self
            .library
            .import_dialog
            .as_deref()
            .and_then(|d| d.finished())
            .and_then(|r| r.library_file.clone());
        let Some(file) = file.filter(|f| self.library.library_at(f).is_some()) else {
            return Task::none();
        };
        self.library.import_dialog = None;
        Task::done(Message::Library(LibraryMessage::OpenLibraryBrowser(file)))
    }

    /// View import report in the Library Browser's library menu: read the
    /// library's stored report on a worker. An open dialog stays as it is,
    /// as File ▸ Import ▸ Library… leaves it.
    pub(super) fn read_stored_import_report(&mut self, file: PathBuf) -> Task<Message> {
        self.library.import_menu = None;
        if self.library.import_dialog.is_some() {
            return Task::none();
        }
        let dir = file.parent().map(Path::to_path_buf).unwrap_or_default();
        Task::perform(
            async move {
                let read = tokio::task::spawn_blocking(move || read_stored_report(&dir))
                    .await
                    .unwrap_or_else(|e| Err(e.to_string()));
                (file, read)
            },
            |(library_file, read)| {
                dialog_message(ImportDialogMsg::StoredReportRead {
                    library_file,
                    report: read.map(Box::new),
                })
            },
        )
    }

    /// The stored report was read: the dialog opens on it, or on why it
    /// could not be read.
    pub(super) fn stored_import_report_read(
        &mut self,
        library_file: PathBuf,
        report: Result<Box<ImportReport>, String>,
    ) -> Task<Message> {
        if self.library.import_dialog.is_some() {
            return Task::none();
        }
        let result = ImportResult::stored(library_file, report.map(|r| *r));
        self.library.import_dialog = Some(Box::new(ImportDialogState::for_stored_report(result)));
        Task::none()
    }
}
