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
    ApplyInput, DriftJob, DriftReport, MountedLibrary, SheetEntries, instance_stamps, part_stamps,
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
            self.library.drift.missing.clear();
            self.library.drift.names.clear();
            self.library.drift.unreadable_libs.clear();
            self.update_selection_info();
            return None;
        }
        // Drop what belongs to an unmounted library now: until the async
        // pass returns, "Update from library" must not apply its symbol.
        self.prune_unmounted_drift(&libs);
        // Clone only; hashing and library reads happen in the worker.
        let sheets: Vec<SheetEntries> = self
            .document_state
            .engines
            .iter()
            .map(|(path, engine)| (path.clone(), referenced_bodies(engine.document())))
            .collect();
        let parts = self
            .document_state
            .engines
            .iter()
            .map(|(path, engine)| (path.clone(), part_stamps(engine.document())))
            .collect();
        Some(DriftJob {
            generation: self.library.drift.generation,
            sheets,
            parts,
            libs,
        })
    }

    /// Remove `names` and `drifted` entries whose library is not in `libs`.
    fn prune_unmounted_drift(&mut self, libs: &[MountedLibrary]) {
        let mounted = |lib_id: &str| {
            crate::library::drift::library_of(lib_id)
                .is_some_and(|library| libs.iter().any(|l| l.id == library))
        };
        let drift = &mut self.library.drift;
        drift
            .unreadable_libs
            .retain(|library, _| libs.iter().any(|l| l.id == *library));
        drift.names.retain(|lib_id, _| mounted(lib_id));
        for rows in drift.drifted.values_mut().chain(drift.missing.values_mut()) {
            rows.retain(|lib_id| mounted(lib_id));
        }
        for rows in drift.stale_bodies.values_mut() {
            rows.retain(|key| mounted(signex_types::schematic::lib_id_of_body_ref(key)));
        }
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
        // Say it once per (library, reason): a different failure is new.
        for (library, (name, reason)) in &report.unreadable_libs {
            if self.library.drift.unreadable_libs.get(library) != Some(reason) {
                crate::diagnostics::log_warning(crate::library::drift::unreadable_library_warning(
                    name, reason,
                ));
            }
        }
        self.library.drift.unreadable_libs = report
            .unreadable_libs
            .iter()
            .map(|(library, (_, reason))| (*library, reason.clone()))
            .collect();
        let mut drifted = std::collections::HashMap::new();
        for (sheet, entries) in report.sheets {
            // Closed meanwhile: nothing to show.
            if !self.document_state.engines.contains_key(&sheet) {
                continue;
            }
            let was = self.library.drift.drifted.get(&sheet);
            let mut entries = entries;
            if let Some(previous) = was {
                keep_unreadable_rows(&mut entries, previous, &report.unreadable);
            }
            let changed = was.is_none_or(|w| *w != entries);
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
        let mut stale = report.stale_bodies;
        stale.retain(|sheet, _| self.document_state.engines.contains_key(sheet));
        self.library.drift.stale_bodies = stale;
        self.record_missing_symbols(report.missing, &report.unreadable);
        let mut names = report.names;
        keep_unreadable_names(&mut names, &self.library.drift.names, &report.unreadable);
        self.library.drift.names = names;
        self.update_selection_info();
        Task::none()
    }

    /// Store which placed symbols lost their library symbol (#1845) and say
    /// so once per change, naming the sheet; the Properties panel shows the
    /// state on each placed part.
    fn record_missing_symbols(
        &mut self,
        mut missing: std::collections::HashMap<
            std::path::PathBuf,
            std::collections::BTreeSet<String>,
        >,
        unreadable: &std::collections::HashSet<String>,
    ) {
        // An unreadable lookup says nothing about a removal: keep the
        // previous state, as `keep_unreadable_rows` does for drift.
        for (sheet, previous) in &self.library.drift.missing {
            let kept: Vec<&String> = previous
                .iter()
                .filter(|id| unreadable.contains(*id))
                .collect();
            if !kept.is_empty() {
                missing
                    .entry(sheet.clone())
                    .or_default()
                    .extend(kept.into_iter().cloned());
            }
        }
        missing.retain(|sheet, _| self.document_state.engines.contains_key(sheet));
        for (sheet, ids) in &missing {
            if self.library.drift.missing.get(sheet) == Some(ids) {
                continue;
            }
            let names: Vec<String> = ids
                .iter()
                .map(|id| {
                    self.library
                        .drift
                        .display_name(id, &self.library.open_libraries)
                })
                .collect();
            crate::diagnostics::log_warning(format!(
                "{}: library symbol missing for {}; the sheet keeps its embedded copy.",
                sheet.file_name().map_or_else(
                    || sheet.display().to_string(),
                    |n| n.to_string_lossy().into_owned()
                ),
                names.join(", ")
            ));
        }
        self.library.drift.missing = missing;
    }

    /// Properties ▸ "Update from library" for the placed symbol `uuid`: the
    /// instances of its `lib_id` go to the shared off-thread apply read.
    ///
    /// `only_this` is "Only this one": just that part (all its units) moves to
    /// the library's current version; by default every instance of the library
    /// symbol on the sheet does.
    pub(crate) fn update_symbol_from_library(
        &mut self,
        uuid: uuid::Uuid,
        only_this: bool,
    ) -> Task<Message> {
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
        if self.library.drift.is_missing(&sheet, &lib_id) {
            crate::diagnostics::log_warning(format!(
                "{} is missing from its library; this sheet keeps its embedded copy, so there is nothing to update from.",
                self.library
                    .drift
                    .display_name(&lib_id, &self.library.open_libraries)
            ));
            return Task::none();
        }
        if !self.library.drift.is_drifted(&sheet, &lib_id) {
            crate::diagnostics::log_warning(format!(
                "{} has no pending library update on this sheet.",
                self.library
                    .drift
                    .display_name(&lib_id, &self.library.open_libraries)
            ));
            return Task::none();
        }
        let mut stamps = instance_stamps(engine.document(), &lib_id, &Default::default());
        let only = only_this.then(|| signex_engine::placed_unit_ids(engine.document(), uuid));
        if let Some(units) = &only {
            stamps.retain(|s| units.contains(&s.symbol));
        }
        let input = ApplyInput {
            only,
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

/// The embedded bodies some placed symbol draws with. A version nothing
/// references (its last instance was deleted or moved) is not judged: it
/// would read "out of date" for good, with nothing for Update to move.
pub(crate) fn referenced_bodies(
    doc: &signex_types::schematic::SchematicSheet,
) -> Vec<(String, String, signex_types::schematic::LibSymbol)> {
    // The library symbol is what an instance names, not what the key spells.
    let mut used: std::collections::HashMap<&str, &str> = std::collections::HashMap::new();
    for s in &doc.symbols {
        used.entry(s.body_key()).or_insert(&s.lib_id);
    }
    doc.lib_symbols
        .iter()
        .filter_map(|(key, body)| {
            let lib_id = used.get(key.as_str())?;
            Some((key.clone(), (*lib_id).to_owned(), body.clone()))
        })
        .collect()
}

/// An unreadable library says nothing about its rows: carry the previous
/// ones over instead of reading the failure as "no drift". A kept row is only
/// a flag: "Update from library" re-reads the library when it is used, and the
/// engine's connectivity gate still vets the result.
fn keep_unreadable_rows(
    entries: &mut std::collections::BTreeSet<String>,
    previous: &std::collections::BTreeSet<String>,
    unreadable: &std::collections::HashSet<String>,
) {
    for lib_id in previous.iter().filter(|id| unreadable.contains(*id)) {
        entries.insert(lib_id.clone());
    }
}

/// Same rule for the readable `Library/Symbol` names.
fn keep_unreadable_names(
    names: &mut std::collections::HashMap<String, String>,
    previous: &std::collections::HashMap<String, String>,
    unreadable: &std::collections::HashSet<String>,
) {
    for (lib_id, name) in previous.iter().filter(|(id, _)| unreadable.contains(*id)) {
        names.entry(lib_id.clone()).or_insert_with(|| name.clone());
    }
}

#[cfg(test)]
mod tests;
