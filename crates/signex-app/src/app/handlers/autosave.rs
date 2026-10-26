//! Autosave handlers (#104): the tick snapshots dirty schematics, the
//! write runs off the UI thread, a normal save removes the sidecar.
//! The decisions live in `crate::autosave`; see its module docs.

use std::path::{Path, PathBuf};

use iced::Task;

use crate::app::{Message, Signex};
use crate::autosave::{AutosaveDone, AutosaveMsg, AutosaveRemoved, DocStamp, sidecar};

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
        }
    }

    fn is_dirty_schematic(&self, path: &Path) -> bool {
        self.document_state.engines.contains_key(path)
            && self.document_state.dirty_paths.contains(path)
    }

    /// Remove the sidecar of every document that is closed or no longer
    /// dirty. Cleanup never depends on the preference: a copy written
    /// while autosave was on must go even after it is turned off.
    pub(crate) fn autosave_sweep(&mut self) -> Task<Message> {
        let Some(dir) = self.document_state.autosave.dir().map(Path::to_path_buf) else {
            return Task::none();
        };
        let stale = self
            .document_state
            .autosave
            .stale(|p| self.is_dirty_schematic(p));
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
        // leave a sidecar of the discarded edits; its completion removes it.
        self.document_state.autosave.supersede_all_in_flight();
        let paths = self.document_state.autosave.take_all_written();
        let tasks: Vec<Task<Message>> = paths
            .into_iter()
            .map(|path| self.start_remove(&dir, path))
            .collect();
        Task::batch(tasks)
    }

    fn start_remove(&mut self, dir: &Path, path: PathBuf) -> Task<Message> {
        self.document_state.autosave.forget(&path);
        self.document_state.autosave.begin_remove(&path);
        remove_task(dir.to_path_buf(), path)
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
        self.document_state.autosave.begin(path);
        Some(write_task(
            dir.to_path_buf(),
            path.to_path_buf(),
            stamp,
            bytes,
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
        let nothing_to_recover = superseded || !self.is_dirty_schematic(&path);
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

    fn report_autosave_failure(&mut self, reason: &str) {
        if self.document_state.autosave.should_report(reason) {
            crate::diagnostics::log_error("Autosave", &anyhow::anyhow!(reason.to_string()));
        }
    }
}

fn write_task(dir: PathBuf, path: PathBuf, stamp: DocStamp, bytes: Vec<u8>) -> Task<Message> {
    let target = path.clone();
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || sidecar::write(&dir, &target, &bytes))
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

fn remove_task(dir: PathBuf, path: PathBuf) -> Task<Message> {
    let target = path.clone();
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || sidecar::remove(&dir, &path))
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
