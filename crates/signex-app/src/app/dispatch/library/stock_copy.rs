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
use crate::library::stock_copy::job::{StockCopyJob, StockCopyOutcome};
use crate::library::stock_copy::{CopyTarget, StockCopyMsg, StockCopyState, StockPrimitiveKind};

impl Signex {
    /// A stock primitive was asked for. With a usable remembered library
    /// the copy starts at once; otherwise the dialog asks which library.
    pub(crate) fn request_stock_copy(
        &mut self,
        path: PathBuf,
        kind: StockPrimitiveKind,
    ) -> Task<Message> {
        if self.library.stock_copies_in_flight.contains(&path) {
            return Task::none();
        }
        if let Some(target) = self.library.usable_stock_copy_target() {
            let target = target.to_path_buf();
            return self.start_stock_copy(path, kind, CopyTarget::Existing(target));
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
        self.start_stock_copy(state.source, state.kind, target)
    }

    fn start_stock_copy(
        &mut self,
        source: PathBuf,
        kind: StockPrimitiveKind,
        target: CopyTarget,
    ) -> Task<Message> {
        let taken_names = match &target {
            CopyTarget::Existing(root) => self.library.primitive_names(root, kind),
            CopyTarget::New(_) => Default::default(),
        };
        let source_library_dir = self
            .library
            .library_dir_of(&source)
            .or_else(|| {
                source
                    .parent()
                    .and_then(Path::parent)
                    .map(Path::to_path_buf)
            })
            .unwrap_or_default();
        let library = match &target {
            CopyTarget::Existing(path) | CopyTarget::New(path) => path.clone(),
        };
        let job = StockCopyJob {
            source: source.clone(),
            kind,
            source_library_dir,
            target,
            taken_names,
            new_library_defaults: self.library.new_library_defaults.clone(),
            now: chrono::Utc::now(),
        };
        self.library.stock_copies_in_flight.insert(source.clone());
        let failed = StockCopyOutcome {
            source,
            library,
            created_library: false,
            result: Err("the copy worker stopped before it finished".to_string()),
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
                return Task::none();
            }
        };
        let mut tasks = Vec::new();
        if self.library.stock_copy_target.as_deref() != Some(outcome.library.as_path()) {
            self.library.stock_copy_target = Some(outcome.library.clone());
            let remembered = outcome.library.clone();
            tasks.push(crate::fonts::background_pref_write(
                "stock_copy_library",
                move || crate::fonts::write_stock_copy_target_pref(&remembered),
            ));
        }
        if outcome.created_library {
            if self
                .library
                .request_mount(&outcome.library, MountIntent::Silent)
                == MountRequest::Spawn
            {
                tasks.push(spawn_pending_mounts(vec![outcome.library.clone()]));
            }
        } else {
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
