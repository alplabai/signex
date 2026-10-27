//! Autosave handlers (#104): the tick snapshots dirty schematics (and, in
//! `autosave_primitives.rs`, symbol and footprint editors), the
//! write runs off the UI thread, a normal save removes the sidecar.
//! The decisions live in `crate::autosave`; see its module docs. The
//! recovery prompt is in `autosave_recovery.rs`.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use iced::Task;

use crate::app::{Message, Signex};
use crate::autosave::{
    AutosaveDone, AutosaveMsg, AutosaveRemoved, DocStamp, InFlight, QUIT_WAIT_LIMIT, recovery,
    sidecar,
};

impl Signex {
    pub(crate) fn dispatch_autosave_message(&mut self, msg: AutosaveMsg) -> Task<Message> {
        match msg {
            AutosaveMsg::Ticked => self.autosave_tick(),
            AutosaveMsg::Written(done) => self.autosave_written(done),
            AutosaveMsg::Discarded(removed) => {
                self.document_state.autosave.finish_remove(&removed.path);
                if let Err(reason) = removed.result {
                    self.report_autosave_failure(&reason);
                }
                Task::none()
            }
            AutosaveMsg::RecoveryChecked(checked) => self.recovery_checked(checked),
            AutosaveMsg::Recovery(choice) => self.recovery_answered(choice),
            AutosaveMsg::RestoreLoaded(loaded) => self.restore_loaded(loaded),
            AutosaveMsg::StartupSwept(result) => {
                match result {
                    Ok(report) => self.report_sweep_failures(&report),
                    Err(reason) => self.report_autosave_failure(&reason),
                }
                Task::none()
            }
        }
    }

    fn is_dirty_schematic(&self, path: &Path) -> bool {
        self.document_state.engines.contains_key(path)
            && self.document_state.dirty_paths.contains(path)
    }

    /// Does `path` hold edits a recovery copy should protect: a dirty
    /// schematic, symbol editor or footprint editor?
    pub(super) fn holds_recoverable_edits(&self, path: &Path) -> bool {
        self.is_dirty_schematic(path) || self.is_dirty_primitive_editor(path)
    }

    /// Remove the sidecar of every document that is closed or no longer
    /// dirty. Cleanup never depends on the preference: a copy written
    /// while autosave was on must go even after it is turned off.
    pub(crate) fn autosave_sweep(&mut self) -> Task<Message> {
        // A document without an engine or editor is gone. One whose tab was
        // closed while dirty keeps its engine parked and is still autosaved,
        // so its earlier-run copy stays protected until the engine goes too.
        let open: std::collections::HashSet<PathBuf> = self
            .document_state
            .engines
            .keys()
            .chain(self.document_state.symbol_editors.keys())
            .chain(self.document_state.footprint_editors.keys())
            .cloned()
            .collect();
        self.document_state
            .autosave
            .retain_offers(|path| open.contains(path));
        self.document_state
            .autosave
            .retain_deferred(|path| open.contains(path));
        let Some(dir) = self.document_state.autosave.dir().map(Path::to_path_buf) else {
            return Task::none();
        };
        let stale = self
            .document_state
            .autosave
            .stale(|p| self.holds_recoverable_edits(p));
        let tasks: Vec<Task<Message>> = stale
            .into_iter()
            .map(|path| self.start_remove(&dir, path))
            .collect();
        Task::batch(tasks)
    }

    /// Remove every sidecar, for the quit that discards all unsaved work.
    pub(crate) fn autosave_discard_all(&mut self) -> Task<Message> {
        let Some(dir) = self.document_state.autosave.dir().map(Path::to_path_buf) else {
            return Task::none();
        };
        // A write still running would land after the removal below and
        // leave a sidecar of the discarded edits; the task removes its own
        // result once it sees this flag.
        self.document_state.autosave.supersede_all_in_flight();
        let paths = self.document_state.autosave.take_all_written();
        let tasks: Vec<Task<Message>> = paths
            .into_iter()
            .map(|path| self.start_remove(&dir, path))
            .collect();
        Task::batch(tasks)
    }

    pub(crate) fn start_remove(&mut self, dir: &Path, path: PathBuf) -> Task<Message> {
        self.document_state.autosave.forget(&path);
        self.document_state.autosave.begin_remove(&path);
        let guard = self.document_state.autosave.track_job();
        remove_task(dir.to_path_buf(), path, guard)
    }

    /// Quit path: wait for every sidecar write and removal still running,
    /// then run `next`, so the process never exits mid-write and a late
    /// write cannot leave a sidecar of edits the user discarded.
    pub(crate) fn wait_for_autosave_then(&mut self, next: Task<Message>) -> Task<Message> {
        let barrier = self.document_state.autosave.barrier();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    if !barrier.wait_timeout(QUIT_WAIT_LIMIT) {
                        crate::diagnostics::log_error(
                            "Autosave cleanup did not finish before exit",
                            &anyhow::anyhow!(
                                "autosave work was still running after {} s, so the app exits \
                                 without waiting further; a recovery copy may be left behind",
                                QUIT_WAIT_LIMIT.as_secs()
                            ),
                        );
                    }
                })
                .await
                .map_err(|e| format!("waiting for the autosave writes failed: {e}"))
            },
            |result| match result {
                Ok(()) => Message::Noop,
                Err(reason) => Message::File(crate::app::FileMsg::DocumentSaveWaitFailed(reason)),
            },
        )
        .chain(next)
    }

    fn autosave_tick(&mut self) -> Task<Message> {
        let sweep = self.autosave_sweep();
        // Re-checked here, not only in the subscription: a tick queued
        // just before the toggle went off must not write.
        if !self.ui_state.autosave_enabled {
            return sweep;
        }
        let Some(dir) = self.document_state.autosave.dir().map(Path::to_path_buf) else {
            return sweep;
        };
        let mut tasks = vec![sweep];
        let dirty: Vec<PathBuf> = self
            .document_state
            .engines
            .keys()
            .filter(|p| self.is_dirty_schematic(p))
            .cloned()
            .collect();
        for path in dirty {
            if let Some(task) = self.autosave_snapshot_task(&dir, &path) {
                tasks.push(task);
            }
        }
        for path in self.dirty_primitive_editor_paths() {
            if let Some(task) = self.autosave_primitive_snapshot_task(&dir, &path) {
                tasks.push(task);
            }
        }
        Task::batch(tasks)
    }

    fn autosave_snapshot_task(&mut self, dir: &Path, path: &Path) -> Option<Task<Message>> {
        let engine = self.document_state.engines.get(path)?;
        let stamp = DocStamp {
            revision: engine.revision(),
            net_color_revision: self.net_color_revision_of(path),
        };
        if !self.document_state.autosave.needs_write(path, stamp) {
            return None;
        }
        let bytes = match engine.serialize_with_net_colors(&self.ui_state.wire_color_overrides) {
            Ok(text) => text.into_bytes(),
            Err(error) => {
                let reason = format!(
                    "could not encode {} for autosave: {error:#}",
                    path.display()
                );
                self.report_autosave_failure(&reason);
                return None;
            }
        };
        let superseded = self.document_state.autosave.begin(path);
        let guard = self.document_state.autosave.track_job();
        Some(write_task(
            dir.to_path_buf(),
            path.to_path_buf(),
            stamp,
            bytes,
            superseded,
            guard,
        ))
    }

    fn autosave_written(&mut self, done: AutosaveDone) -> Task<Message> {
        let AutosaveDone {
            path,
            stamp,
            result,
        } = done;
        let ok = result.is_ok();
        let superseded = self.document_state.autosave.finish(&path, stamp, ok);
        match result {
            Ok(()) => self.document_state.autosave.clear_error(),
            Err(reason) => self.report_autosave_failure(&reason),
        }
        // Saved or closed while the write ran: the copy just written is
        // stale or pointless.
        let nothing_to_recover = superseded || !self.holds_recoverable_edits(&path);
        let Some(dir) = self.document_state.autosave.dir().map(Path::to_path_buf) else {
            return Task::none();
        };
        if ok && nothing_to_recover {
            return self.start_remove(&dir, path);
        }
        Task::none()
    }

    /// A normal save of `path` landed: drop its recovery copy. Runs
    /// whether or not the preference is still on, so turning autosave off
    /// never strands a sidecar behind a saved document.
    pub(crate) fn autosave_after_save(&mut self, path: &Path) -> Task<Message> {
        let on_disk = self.document_state.autosave.saved(path);
        let Some(dir) = self.document_state.autosave.dir().map(Path::to_path_buf) else {
            return Task::none();
        };
        if on_disk {
            return self.start_remove(&dir, path.to_path_buf());
        }
        Task::none()
    }

    /// The file at `path` was rewritten without going through a document
    /// save (Annotate and Reset Duplicate Designators on a sheet with no
    /// open tab). Only a copy this run wrote is removed. A copy an earlier
    /// run left may hold unsaved edits to a sheet nobody has opened yet, so
    /// it stays for the reconcile at the next open to compare bytes and
    /// offer it.
    pub(crate) fn autosave_after_disk_write(&mut self, path: &Path) -> Task<Message> {
        let ours = self.document_state.autosave.saved(path);
        let Some(dir) = self.document_state.autosave.dir().map(Path::to_path_buf) else {
            return Task::none();
        };
        if ours {
            return self.start_remove(&dir, path.to_path_buf());
        }
        Task::none()
    }

    fn report_sweep_failures(&mut self, report: &recovery::SweepReport) {
        let Some(first) = &report.first_failure else {
            return;
        };
        let reason = format!(
            "{first} ({} of the stranded temp files in the recovery folder could not be \
             cleared)",
            report.failed
        );
        self.report_autosave_failure(&reason);
    }

    pub(crate) fn report_autosave_failure(&mut self, reason: &str) {
        if self.document_state.autosave.should_report(reason) {
            crate::diagnostics::log_error("Autosave", &anyhow::anyhow!(reason.to_string()));
        }
    }
}

pub(super) fn write_task(
    dir: PathBuf,
    path: PathBuf,
    stamp: DocStamp,
    bytes: Vec<u8>,
    superseded: Arc<AtomicBool>,
    guard: InFlight,
) -> Task<Message> {
    let target = path.clone();
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || {
                let _guard = guard;
                sidecar::write(&dir, &target, &bytes)?;
                // Superseded while writing (saved, closed, or a discarding
                // quit): the copy just written is stale, and no completion
                // message may be left to remove it, so remove it here.
                if superseded.load(Ordering::SeqCst)
                    && let Err(reason) = sidecar::remove(&dir, &target)
                {
                    // Reported here: after a quit nothing handles the
                    // completion message that would carry it.
                    crate::diagnostics::log_error(
                        "Autosave",
                        &anyhow::anyhow!("{reason} (a superseded recovery copy was left behind)"),
                    );
                }
                Ok(())
            })
            .await
            .map_err(|e| format!("the autosave task failed: {e}"))
            .and_then(|result| result)
        },
        move |result| {
            Message::Autosave(AutosaveMsg::Written(AutosaveDone {
                path: path.clone(),
                stamp,
                result,
            }))
        },
    )
}

fn remove_task(dir: PathBuf, path: PathBuf, guard: InFlight) -> Task<Message> {
    let target = path.clone();
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || {
                let _guard = guard;
                sidecar::remove(&dir, &path)
            })
            .await
            .map_err(|e| format!("the autosave cleanup task failed: {e}"))
            .and_then(|result| result)
        },
        move |result| {
            Message::Autosave(AutosaveMsg::Discarded(AutosaveRemoved {
                path: target.clone(),
                result,
            }))
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::autosave::recovery;

    /// An earlier run's copy of a sheet that is not open must survive
    /// Annotate rewriting that sheet, and be offered when it opens.
    #[test]
    fn earlier_run_copy_of_an_unopened_sheet_survives_a_disk_write() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("recovery");
        let sheet = tmp.path().join("other.snxsch");
        std::fs::write(&sheet, b"annotated").unwrap();
        sidecar::write(&dir, &sheet, b"unsaved edits").unwrap();

        let (mut app, _boot) = Signex::new();
        app.document_state.autosave.set_dir(Some(dir.clone()));
        let task = app.autosave_after_disk_write(&sheet);
        assert!(
            iced_runtime::task::into_stream(task).is_none(),
            "no removal may be started for a copy this run did not write"
        );

        assert!(
            sidecar::sidecar_path(&dir, &sheet).exists(),
            "an earlier run's copy must not be deleted unseen"
        );
        let offered = recovery::reconcile(&dir, &sheet, std::time::SystemTime::now()).unwrap();
        assert!(offered.is_some(), "the differing copy is offered at open");
    }

    /// The quit wait returns only once every tracked job is done: a held
    /// job keeps the `Task::done` marker that follows it from arriving.
    #[test]
    fn the_quit_wait_holds_back_what_follows_until_a_tracked_job_finishes() {
        use futures::StreamExt;
        let (mut app, _boot) = Signex::new();
        let guard = app.document_state.autosave.track_job();
        let marker = Message::File(crate::app::FileMsg::DocumentSaveWaitFailed(
            "marker".to_string(),
        ));
        let task = app.wait_for_autosave_then(Task::done(marker));
        let Some(mut stream) = iced_runtime::task::into_stream(task) else {
            panic!("the wait is a real task");
        };

        let (sender, receiver) = std::sync::mpsc::channel();
        let runner = std::thread::spawn(move || {
            let runtime = tokio::runtime::Runtime::new().unwrap();
            runtime.block_on(async move {
                while let Some(action) = stream.next().await {
                    if let iced_runtime::Action::Output(message) = action
                        && sender.send(message).is_err()
                    {
                        break;
                    }
                }
            });
        });

        // Nothing may arrive while the job is held. The wait is far longer
        // than the stream needs to emit its first message when it does not
        // wait at all, so a broken barrier fails here every time.
        assert!(
            receiver
                .recv_timeout(std::time::Duration::from_millis(300))
                .is_err(),
            "the quit continued while an autosave job was still running"
        );
        drop(guard);
        let mut arrived = Vec::new();
        while let Ok(message) = receiver.recv_timeout(std::time::Duration::from_secs(10)) {
            arrived.push(message);
        }
        runner.join().unwrap();
        assert!(
            arrived.iter().any(|m| matches!(
                m,
                Message::File(crate::app::FileMsg::DocumentSaveWaitFailed(text)) if text == "marker"
            )),
            "the marker arrives once the job is released: {arrived:?}"
        );
    }
}
