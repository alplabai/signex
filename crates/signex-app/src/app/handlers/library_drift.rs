//! Library symbol drift handlers (#912): the `update` half of
//! `library/drift.rs`.
//!
//! * A recheck is *requested* by setting `library.drift.check_pending`
//!   (sheet opened, library mounted, primitive saved, undo/redo);
//!   `finish_update` drains it into one worker task covering every open
//!   sheet.
//! * "Update from library" takes the same off-thread read as the Library
//!   Updates modal's Apply (`dispatch/library/updates.rs`): the worker reads
//!   the library's symbol and each instance's row fresh, and the completion
//!   applies them through `Command::ReplaceLibSymbol` - one undo step. The
//!   engine refuses it when a connected pin would vanish or move; the
//!   refusal reaches the Messages panel and the sheet is left untouched.
//! * "Keep" is doing nothing: the choice is not persisted, so the drift
//!   shows again the next time the sheet is opened.

use iced::Task;

use super::super::contracts::Message;
use super::super::state::Signex;
use crate::library::drift::{
    ApplyInput, ApplyStamp, DriftJob, DriftReport, MountedLibrary, SheetEntries,
};
use crate::library::messages::LibraryMessage;

impl Signex {
    /// Start the off-thread drift pass if one was requested.
    pub(crate) fn drain_pending_drift_check(&mut self) -> Task<Message> {
        let Some(job) = self.take_drift_job() else {
            return Task::none();
        };
        let generation = job.generation;
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || job.run())
                    .await
                    .map_err(|e| format!("library drift worker failed: {e}"))
            },
            move |result| {
                let report = result.unwrap_or_else(|problem| DriftReport {
                    problems: vec![problem],
                    generation,
                    ..Default::default()
                });
                Message::Library(LibraryMessage::DriftChecked(Box::new(report)))
            },
        )
    }

    /// The open libraries, as the off-thread workers read them.
    pub(crate) fn mounted_libraries(&self) -> Vec<MountedLibrary> {
        self.library
            .open_libraries
            .iter()
            .map(|l| MountedLibrary {
                id: l.library_id,
                path: l.root.clone(),
                display_name: l.display_name.clone(),
            })
            .collect()
    }

    /// Consume `check_pending` into a [`DriftJob`] (a new generation), or
    /// `None` when nothing was requested or no library is mounted (then the
    /// previous result is cleared and in-flight passes become stale).
    pub fn take_drift_job(&mut self) -> Option<DriftJob> {
        if !std::mem::take(&mut self.library.drift.check_pending) {
            return None;
        }
        self.library.drift.generation += 1;
        let libs = self.mounted_libraries();
        if libs.is_empty() {
            self.library.drift.drifted.clear();
            self.library.drift.names.clear();
            self.update_selection_info();
            return None;
        }
        // Clone only; hashing and library reads happen in the worker.
        let sheets: Vec<SheetEntries> = self
            .document_state
            .engines
            .iter()
            .map(|(path, engine)| {
                let entries = engine
                    .document()
                    .lib_symbols
                    .iter()
                    .map(|(id, sym)| (id.clone(), sym.clone()))
                    .collect();
                (path.clone(), entries)
            })
            .collect();
        Some(DriftJob {
            generation: self.library.drift.generation,
            sheets,
            libs,
        })
    }

    pub(crate) fn handle_library_drift_checked(&mut self, report: DriftReport) -> Task<Message> {
        // A newer pass was started since this one: its result is stale.
        if report.generation != self.library.drift.generation {
            return Task::none();
        }
        for problem in &report.problems {
            crate::diagnostics::log_warning(problem);
        }
        // A worker failure carries only `problems`; keep the previous result.
        if report.sheets.is_empty() && !report.problems.is_empty() {
            return Task::none();
        }
        let mut drifted = std::collections::HashMap::new();
        for (sheet, entries) in report.sheets {
            // Closed meanwhile: nothing to show.
            if !self.document_state.engines.contains_key(&sheet) {
                continue;
            }
            let was = self.library.drift.drifted.get(&sheet);
            let changed = was.is_none_or(|w| !w.keys().eq(entries.keys()));
            if changed && !entries.is_empty() {
                crate::diagnostics::log_info(format!(
                    "{}: {} library symbol(s) differ from the library; select a part and use \"Update from library\" in Properties.",
                    sheet.display(),
                    entries.len()
                ));
            }
            drifted.insert(sheet, entries);
        }
        self.library.drift.drifted = drifted;
        self.library.drift.names = report.names;
        self.update_selection_info();
        Task::none()
    }

    /// Properties ▸ "Update from library" for the placed symbol `uuid`: the
    /// instances of its `lib_id` go to the shared off-thread apply read.
    pub(crate) fn update_symbol_from_library(&mut self, uuid: uuid::Uuid) -> Task<Message> {
        let Some(sheet) = self.document_state.active_path.clone() else {
            return Task::none();
        };
        let Some(engine) = self.document_state.active_engine() else {
            return Task::none();
        };
        let Some(lib_id) = engine
            .document()
            .symbols
            .iter()
            .find(|s| s.uuid == uuid)
            .map(|s| s.lib_id.clone())
        else {
            return Task::none();
        };
        if !self.library.drift.is_drifted(&sheet, &lib_id) {
            crate::diagnostics::log_warning(format!(
                "{lib_id} has no pending library update on this sheet."
            ));
            return Task::none();
        }
        let stamps = engine
            .document()
            .symbols
            .iter()
            .filter(|s| s.lib_id == lib_id)
            .filter_map(|s| {
                Some(ApplyStamp {
                    symbol: s.uuid,
                    reference: s.reference.clone(),
                    row_id: s.row_id?,
                    library_id: s.library_id?,
                })
            })
            .collect();
        let input = ApplyInput {
            sheet,
            engine_instance: engine.instance_id(),
            any_unchecked: false,
            scan_failed: false,
            tracks_pending: false,
            wanted: vec![(lib_id, stamps)],
        };
        self.spawn_apply_read(input)
    }
}
