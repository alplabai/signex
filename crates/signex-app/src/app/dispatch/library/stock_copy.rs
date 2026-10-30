//! Opening a stock symbol or footprint edits a copy (#813): the handlers of
//! the Copy to Library dialog and of the copy it starts.
//!
//! `update` only decides and records; the copy itself (reading the stock
//! file, creating a library, writing files, committing) is a
//! [`StockCopyJob`] on a blocking worker.

use std::path::{Path, PathBuf};

use super::*;
use crate::app::state::ErrorNotice;
use crate::library::mount::{MountIntent, MountRequest, spawn_pending_mounts};
use crate::library::refresh::{RefreshFollowUp, refresh_task};
use crate::library::stock_copy::job::{StockCopyJob, StockCopyOutcome, StockSource};
use crate::library::stock_copy::{CopyTarget, StockCopyMsg, StockCopyState, StockPrimitiveKind};

impl Signex {
    /// A stock primitive was asked for. With a usable remembered library
    /// the copy (or the copy already there) opens at once; otherwise the
    /// dialog asks which library.
    pub(crate) fn request_stock_copy(
        &mut self,
        path: PathBuf,
        kind: StockPrimitiveKind,
    ) -> Task<Message> {
        if self.library.stock_copies_in_flight.contains(&path) {
            return Task::none();
        }
        if let Some(open) = &self.library.stock_copy {
            // One question at a time: the second request is refused, not
            // dropped silently.
            if open.source != path {
                crate::diagnostics::log_warning(format!(
                    "Finish the Copy to Library dialog for {} before opening {}.",
                    open.label,
                    self.library.stock_primitive_label(&path, kind)
                ));
            }
            return Task::none();
        }
        if let Some(target) = self.library.usable_stock_copy_target() {
            let target = target.to_path_buf();
            return self.start_stock_copy(
                path.clone(),
                StockSource::File(path),
                kind,
                CopyTarget::Existing(target),
                false,
            );
        }
        let label = self.library.stock_primitive_label(&path, kind);
        self.library.stock_copy = Some(StockCopyState {
            source: path,
            kind,
            label,
            previous_unavailable: self.library.stock_copy_target.clone(),
            error: None,
        });
        Task::none()
    }

    pub(super) fn handle_stock_copy(&mut self, msg: StockCopyMsg) -> Task<Message> {
        match msg {
            StockCopyMsg::TargetPicked(root) => self.handle_stock_copy_target_picked(root),
            StockCopyMsg::NewLibraryRequested => new_library_dialog(),
            StockCopyMsg::NewLibraryPathChosen(Some(path)) => {
                let path =
                    crate::app::handlers::document_files::with_primitive_extension(path, "snxlib");
                self.start_dialog_copy(CopyTarget::New(path))
            }
            StockCopyMsg::NewLibraryPathChosen(None) => Task::none(),
            StockCopyMsg::Cancelled => {
                self.library.stock_copy = None;
                Task::none()
            }
            StockCopyMsg::CopyAgainRequested(copy_path) => self.copy_again(copy_path),
            StockCopyMsg::CopyFinished(outcome) => self.finish_stock_copy(outcome),
        }
    }

    fn handle_stock_copy_target_picked(&mut self, root: PathBuf) -> Task<Message> {
        if !self.library.can_receive_stock_copy(&root) {
            if let Some(state) = self.library.stock_copy.as_mut() {
                state.error = Some(format!(
                    "{} is not open and writable; pick another library.",
                    root.display()
                ));
            }
            return Task::none();
        }
        self.start_dialog_copy(CopyTarget::Existing(root))
    }

    /// Close the dialog and copy the file it asked about into `target`.
    fn start_dialog_copy(&mut self, target: CopyTarget) -> Task<Message> {
        let Some(state) = self.library.stock_copy.take() else {
            return Task::none();
        };
        self.start_stock_copy(
            state.source.clone(),
            StockSource::File(state.source),
            state.kind,
            target,
            false,
        )
    }

    /// Copy Again: a fresh copy of the stock primitive the copy at
    /// `copy_path` was made from, into the library that holds it.
    fn copy_again(&mut self, copy_path: PathBuf) -> Task<Message> {
        let Some(kind) = StockPrimitiveKind::of_path(&copy_path) else {
            return Task::none();
        };
        let copied_from = match kind {
            StockPrimitiveKind::Symbol => self
                .document_state
                .symbol_editors
                .get(&copy_path)
                .and_then(|editor| editor.file.copied_from),
            StockPrimitiveKind::Footprint => self
                .document_state
                .footprint_editors
                .get(&copy_path)
                .and_then(|editor| editor.file.copied_from),
        };
        let stock_dir = self
            .library
            .stock_library_root
            .as_deref()
            .and_then(Path::parent)
            .map(Path::to_path_buf);
        let library = self
            .library
            .containing_library(&copy_path)
            .map(|lib| lib.root.clone());
        let (Some(uuid), Some(stock_dir), Some(library)) = (copied_from, stock_dir, library) else {
            crate::diagnostics::log_warning(
                "Copy Again needs the stock library and the library holding this copy to be open.",
            );
            return Task::none();
        };
        if self.library.stock_copies_in_flight.contains(&copy_path) {
            return Task::none();
        }
        self.start_stock_copy(
            copy_path,
            StockSource::ByUuid { stock_dir, uuid },
            kind,
            CopyTarget::Existing(library),
            true,
        )
    }

    fn start_stock_copy(
        &mut self,
        key: PathBuf,
        source: StockSource,
        kind: StockPrimitiveKind,
        target: CopyTarget,
        copy_again: bool,
    ) -> Task<Message> {
        let source_library_dir = match &source {
            StockSource::File(path) => self
                .library
                .library_dir_of(path)
                .or_else(|| path.parent().and_then(Path::parent).map(Path::to_path_buf))
                .unwrap_or_default(),
            StockSource::ByUuid { stock_dir, .. } => stock_dir.clone(),
        };
        let library = match &target {
            CopyTarget::Existing(path) | CopyTarget::New(path) => path.clone(),
        };
        let job = StockCopyJob {
            key: key.clone(),
            source,
            kind,
            source_library_dir,
            target,
            copy_again,
            new_library_defaults: self.library.new_library_defaults.clone(),
            now: chrono::Utc::now(),
        };
        self.library.stock_copies_in_flight.insert(key.clone());
        let failed = StockCopyOutcome {
            source: key,
            library,
            created_library: false,
            reused: false,
            result: Err("the copy worker stopped before it finished".to_string()),
            registered_global: None,
        };
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || job.run())
                    .await
                    .unwrap_or(failed)
            },
            |outcome| {
                Message::Library(LibraryMessage::StockCopy(StockCopyMsg::CopyFinished(
                    outcome,
                )))
            },
        )
    }

    fn finish_stock_copy(&mut self, outcome: StockCopyOutcome) -> Task<Message> {
        self.library.stock_copies_in_flight.remove(&outcome.source);
        let mut tasks = Vec::new();
        // A library this copy created is recorded as global so it is
        // mounted again at the next start, whether or not the copy worked.
        // Merge rather than replace: the user may have added a global
        // library from the Components panel while the worker ran (#1758).
        match outcome.registered_global {
            Some(Ok(())) => crate::panels::components_panel::global_prefs::merge_path(
                &mut self.library.global_libraries,
                outcome.library.clone(),
            ),
            Some(Err(reason)) => {
                self.report_global_libraries_save_failure("register the new library", &reason)
            }
            None => {}
        }
        if outcome.created_library
            && self
                .library
                .request_mount(&outcome.library, MountIntent::Silent)
                == MountRequest::Spawn
        {
            tasks.push(spawn_pending_mounts(vec![outcome.library.clone()]));
        }
        let copy_path = match outcome.result {
            Ok(path) => path,
            Err(reason) => {
                let noun = StockPrimitiveKind::of_path(&outcome.source)
                    .map_or("primitive", StockPrimitiveKind::noun);
                crate::diagnostics::log_error(
                    "Failed to copy a stock primitive",
                    &anyhow::anyhow!(reason.clone()),
                );
                self.document_state.error_notice = Some(ErrorNotice::open(format!(
                    "The stock {noun} could not be copied into {}: {reason}",
                    outcome.library.display()
                )));
                return Task::batch(tasks);
            }
        };
        if outcome.reused {
            crate::diagnostics::log_info(format!(
                "Opened your existing copy {}. Use Copy Again in its editor for a fresh one.",
                copy_path.display()
            ));
        }
        if self.library.stock_copy_target.as_deref() != Some(outcome.library.as_path()) {
            self.library.stock_copy_target = Some(outcome.library.clone());
            let remembered = outcome.library.clone();
            tasks.push(crate::fonts::background_pref_write(
                "stock_copy_library",
                move || crate::fonts::write_stock_copy_target_pref(&remembered),
            ));
        }
        if !outcome.created_library && !outcome.reused {
            tasks.push(refresh_task(
                &mut self.library,
                &outcome.library,
                RefreshFollowUp::Nothing,
            ));
        }
        tasks.push(self.handle_open_primitive(copy_path));
        Task::batch(tasks)
    }
}

/// The save dialog that names the library to create.
fn new_library_dialog() -> Task<Message> {
    Task::perform(
        async {
            rfd::AsyncFileDialog::new()
                .set_title("New Library")
                .add_filter("Signex Library", &["snxlib"])
                .set_file_name("MyLibrary.snxlib")
                .save_file()
                .await
                .map(|file| file.path().to_path_buf())
        },
        |picked| {
            Message::Library(LibraryMessage::StockCopy(
                StockCopyMsg::NewLibraryPathChosen(picked),
            ))
        },
    )
}
