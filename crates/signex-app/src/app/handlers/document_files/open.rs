//! Document/project open + create handlers. Split from `handlers/document_files.rs`.

use std::path::PathBuf;

use anyhow::{Context, Result};

use super::super::super::*;
use super::new_project::{RuleSeed, new_project_parts, write_new_project_files};
use super::spelling::OpenGate;

impl Signex {
    /// An async read+parse of a document came back failed: log it and
    /// raise the Open Failed notice (a log line alone is invisible to
    /// someone who double-clicked a file).
    pub(crate) fn report_open_failure(&mut self, message: String) {
        crate::diagnostics::log_error(
            "Failed to open document path",
            &anyhow::anyhow!(message.clone()),
        );
        self.document_state.error_notice = Some(crate::app::state::ErrorNotice::open(message));
    }

    pub(crate) fn handle_document_file_opened(
        &mut self,
        path: Option<PathBuf>,
    ) -> iced::Task<Message> {
        let Some(path) = path else {
            return iced::Task::none();
        };

        self.interaction_state.editing_text = None;
        self.interaction_state.context_menu = None;

        match self.open_document_path(path) {
            Ok(task) => task,
            Err(error) => {
                crate::diagnostics::log_error("Failed to open document path", &error);
                self.document_state.error_notice =
                    Some(crate::app::state::ErrorNotice::open(format!("{error:#}")));
                iced::Task::none()
            }
        }
    }

    pub(crate) fn handle_new_project_file(&mut self, path: Option<PathBuf>) -> iced::Task<Message> {
        let Some(path) = path else {
            return iced::Task::none();
        };

        self.interaction_state.editing_text = None;
        self.interaction_state.context_menu = None;

        match new_project_parts(&path) {
            // The directory, the refusal check and the file writes run on a
            // blocking worker; `handle_new_project_written` opens the project.
            Ok(_) => {
                // A second New Project for the same path while the first
                // worker is pending would race it; ignore it.
                if !self.document_state.pending_opens.insert(path.clone()) {
                    return iced::Task::none();
                }
                let seed = RuleSeed {
                    dir: crate::rule_profiles::profiles_dir(),
                    selected: self.ui_state.default_rule_profile.clone(),
                    pending: self.pending_profile_writes(),
                };
                iced::Task::perform(
                    super::super::dock::project_navigation::add_io::on_blocking_pool({
                        let path = path.clone();
                        move || write_new_project_files(&path, &seed).map_err(|e| format!("{e:#}"))
                    }),
                    move |result| Message::File(FileMsg::NewProjectWritten { path, result }),
                )
            }
            Err(error) => {
                self.report_new_project_failure(&format!("{error:#}"));
                iced::Task::none()
            }
        }
    }

    fn report_new_project_failure(&mut self, error: &str) {
        crate::diagnostics::log_error(
            "Failed to create new project",
            &anyhow::anyhow!(error.to_owned()),
        );
        self.document_state.error_notice = Some(crate::app::state::ErrorNotice::create(format!(
            "Failed to create new project: {error}"
        )));
    }

    /// Completion of the off-thread New Project writes: on success load the
    /// project and open its companion schematic; a failure raises the notice.
    pub(crate) fn handle_new_project_written(
        &mut self,
        project_path: PathBuf,
        result: Result<PathBuf, String>,
    ) -> iced::Task<Message> {
        self.document_state
            .pending_opens
            .remove_untracked(&project_path);
        let written = match result {
            Ok(written) => written,
            Err(error) => {
                self.report_new_project_failure(&error);
                return iced::Task::none();
            }
        };
        match self.open_new_project(&written) {
            Ok(task) => task,
            Err(error) => {
                self.report_new_project_failure(&format!("{error:#}"));
                iced::Task::none()
            }
        }
    }

    /// Load a freshly written project and open its companion schematic as
    /// a tab (`<stem>.snxsch` beside the `.snxprj`).
    fn open_new_project(&mut self, project_path: &std::path::Path) -> Result<iced::Task<Message>> {
        let (dir, stem) = new_project_parts(project_path)?;
        let sch_path = dir.join(format!("{stem}.snxsch"));

        // The project itself opens synchronously (a directory-driven
        // `.snxprj` parse, no heavy IO) — only the schematic tab's
        // read+parse below is async, via `open_document_path`'s
        // `snxsch` branch.
        let project_task = self.open_document_path(project_path.to_path_buf())?;
        let schematic_task = {
            // Best-effort: surface the schematic tab so the new project
            // lands the user on a drawable canvas. A failure is logged
            // to the Messages panel and leaves them on the Welcome view.
            match self.open_document_path(sch_path) {
                Ok(task) => task,
                Err(error) => {
                    crate::diagnostics::log_error("Open new project schematic", &error);
                    iced::Task::none()
                }
            }
        };
        self.refresh_panel_ctx();
        // Both Tasks go back to the runtime: the project's library
        // mounts must never be dropped (see `open_document_path`).
        Ok(iced::Task::batch([project_task, schematic_task]))
    }

    /// Help ▸ Open Sample Project: unpack the bundled sample (once) into
    /// the per-user config directory on a blocking worker, so `update`
    /// does no IO. `handle_sample_project_unpacked` opens it.
    pub(crate) fn handle_open_sample_project(&mut self) -> iced::Task<Message> {
        let Some(root) = crate::sample_project::sample_root() else {
            self.report_sample_failure(&anyhow::anyhow!(
                "no per-user config directory to unpack the sample into"
            ));
            return iced::Task::none();
        };
        iced::Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    crate::sample_project::ensure_extracted(&root)
                        .map(|_| root)
                        .map_err(|error| format!("{error:#}"))
                })
                .await
                .unwrap_or_else(|error| Err(format!("sample unpack did not complete: {error}")))
            },
            |result| Message::File(FileMsg::SampleProjectUnpacked(result)),
        )
    }

    /// Open the unpacked sample project and its sheet like any other
    /// project. A failure reaches the log and the Open Failed notice.
    pub(crate) fn handle_sample_project_unpacked(
        &mut self,
        result: Result<PathBuf, String>,
    ) -> iced::Task<Message> {
        let root = match result {
            Ok(root) => root,
            Err(message) => {
                self.report_sample_failure(&anyhow::anyhow!(message));
                return iced::Task::none();
            }
        };
        let project_task = match self.open_document_path(crate::sample_project::project_path(&root))
        {
            Ok(task) => task,
            Err(error) => {
                self.report_sample_failure(&error);
                return iced::Task::none();
            }
        };
        // The project task carries cold library mounts; never drop it.
        let sheet_task = match self.open_document_path(crate::sample_project::sheet_path(&root)) {
            Ok(task) => task,
            Err(error) => {
                crate::diagnostics::log_error("Open sample sheet", &error);
                iced::Task::none()
            }
        };
        self.refresh_panel_ctx();
        iced::Task::batch([project_task, sheet_task])
    }

    fn report_sample_failure(&mut self, error: &anyhow::Error) {
        crate::diagnostics::log_error("Open sample project", error);
        self.document_state.error_notice =
            Some(crate::app::state::ErrorNotice::open(format!("{error:#}")));
    }

    pub(crate) fn handle_active_document_save_requested(&mut self) -> iced::Task<Message> {
        match self.save_active_document() {
            Ok(task) => task,
            Err(error) => {
                self.report_save_failure("Failed to save active document", &error);
                iced::Task::none()
            }
        }
    }

    pub(crate) fn handle_active_document_save_as_requested(
        &mut self,
        from_path: PathBuf,
        to_path: PathBuf,
    ) -> iced::Task<Message> {
        match self.save_active_document_as(&from_path, to_path) {
            Ok(task) => task,
            Err(error) => {
                self.report_save_failure("Failed to save active document as", &error);
                iced::Task::none()
            }
        }
    }

    pub(crate) fn open_document_path(&mut self, path: PathBuf) -> Result<iced::Task<Message>> {
        self.refuse_while_project_moves(&path, "open")?;
        let ext = path
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("");
        let task = match ext {
            // Returns the project's cold `.snxlib` mount preparations
            // (#99 part 2c) — must be propagated, not dropped. Dropping
            // it leaves every library recorded as pending and never
            // spawned, so the project opens with no libraries mounted and
            // nothing logs a failure.
            "standard_pro" | "snxprj" => self.open_project_file(path)?,
            "standard_sch" | "snxsch" => self.open_schematic_file(path)?,
            "standard_pcb" | "snxpcb" => self.open_pcb_file(path)?,
            "snxsym" | "snxfpt" => match self.gate_open(path) {
                OpenGate::Proceed { path, probe } => {
                    iced::Task::batch([probe, self.handle_open_primitive(path)])
                }
                OpenGate::Deferred(probe) => probe,
            },
            // `.snxlib/` is a directory package — open it as a Library
            // Browser tab in the main canvas area. The handler mounts
            // the library if not already mounted and pushes the tab
            // (or activates it if a tab for the same library is
            // already open).
            "snxlib" => self.handle_open_library_browser(path),
            _ => anyhow::bail!("unsupported file type: .{ext}"),
        };

        Ok(task)
    }

    fn open_project_file(&mut self, path: PathBuf) -> Result<iced::Task<Message>> {
        let (_, mount_task) = self.load_or_activate_project(&path)?;
        // Success path only: `?` above returns before a project that
        // failed to load can be recorded.
        let recent_task = self.record_recent_project(&path);
        self.refresh_panel_ctx();
        Ok(iced::Task::batch([mount_task, recent_task]))
    }

    /// Append a `LoadedProject` for `project_path` to the workspace if
    /// it isn't already loaded, then make it active. De-dupes by path
    /// so re-opening the same project just switches activity. Used by
    /// both `open_project_file` (direct .standard_pro open) and the
    /// companion-project path inside `open_schematic_file` /
    /// `open_pcb_file`.
    ///
    /// Returns the resolved `ProjectId` **plus** a `Task` that prepares
    /// the project's cold `.snxlib` mounts off the UI thread (#99 part
    /// 2c). The `ProjectId` still comes back synchronously because every
    /// caller uses it immediately — `.snxprj` parsing itself stays
    /// synchronous on purpose: `parse_project` measures 0.018 ms and does
    /// not scale with library count, so making it async would buy nothing
    /// and would break that immediate use. The library mounts are the
    /// part that actually costs frames.
    fn load_or_activate_project(
        &mut self,
        project_path: &std::path::Path,
    ) -> Result<(crate::app::state::ProjectId, iced::Task<Message>)> {
        if let Some(existing) = self
            .document_state
            .projects
            .iter()
            .find(|p| p.path == project_path)
        {
            let id = existing.id;
            self.document_state.active_project = Some(id);
            return Ok((id, iced::Task::none()));
        }
        // A write of this file is still running (its project may have been
        // closed since): parsing now would load the old bytes as clean.
        if self.document_state.document_saves.is_saving(project_path) {
            anyhow::bail!(
                "A save to that project file is still running, so it was not opened; \
                 try again once it finishes: {}",
                super::save::file_label(project_path)
            );
        }
        let data = signex_types::project::parse_project(project_path)
            .with_context(|| format!("parse project {}", project_path.display()))?;
        let id = self.document_state.mint_project_id();
        // Auto-mount every library referenced by `Project::libraries`
        // so the project tree picks them up before the panel rebuild
        // fires. Errors are logged inside `auto_mount_project_libraries`
        // and never bubble: a missing library shouldn't block the
        // project from loading.
        let outcome =
            crate::library::commands::auto_mount_project_libraries(&mut self.library, &data);
        if outcome.refreshed > 0 || !outcome.pending.is_empty() {
            tracing::info!(
                target: "signex::library",
                project = %project_path.display(),
                refreshed = outcome.refreshed,
                pending = outcome.pending.len(),
                "auto-mounted project libraries"
            );
        }
        // Fan the cold mounts out — one `spawn_blocking` per `.snxlib`,
        // all in flight at once. The work is CPU-parse-bound (~40 MB/s on
        // hardware that does GB/s), so six libraries are embarrassingly
        // parallel. `request_mount` already recorded each path as pending
        // inside `auto_mount_project_libraries`, so no path spawns twice.
        let mount_tasks = crate::library::mount::spawn_pending_mounts(outcome.pending);
        // Probe the project's volume once, off-thread (#694); `update` only
        // stores the answer on the project.
        let case_probe = crate::app::state::project_case::spawn_case_probe(
            id,
            project_path.parent().unwrap_or(project_path).to_path_buf(),
        );
        let mount_task = iced::Task::batch([mount_tasks, case_probe]);
        self.document_state
            .projects
            .push(super::super::super::state::LoadedProject {
                id,
                path: project_path.to_path_buf(),
                data,
                pending_libraries: std::collections::HashMap::new(),
                case_fold: crate::app::state::scope::CaseFold::host_default(),
                case_probed: false,
                case_decided: false,
            });
        self.document_state.active_project = Some(id);
        // A project that recorded a rule profile learns, off the update loop,
        // whether the local copy of that profile has changed since (#1608).
        let drift = self.check_profile_drift_task_for(id);
        Ok((id, iced::Task::batch([mount_task, drift])))
    }

    /// Whether a document already owns `path`: it has a tab, or its engine
    /// holds unsaved edits (a parked tab-less engine). A clean tab-less
    /// engine owns nothing: reopening reads the file, which is the truth.
    /// Exact path equality, pending the `CaseFold` key (#1034).
    pub(crate) fn path_is_owned_by_open_document(&self, path: &std::path::Path) -> bool {
        self.document_state.tabs.iter().any(|tab| tab.path == path)
            || (self.document_state.engines.contains_key(path)
                && self.document_state.dirty_paths.contains(path))
    }

    /// The owner of a superseded schematic open result may be a dirty
    /// engine parked without a tab (a sheet-reference rewrite loaded it
    /// while the open was in flight). The user asked for that tab, so attach
    /// it to the parked engine, exactly like a re-open does; nothing is read
    /// from disk and the engine is kept as is.
    pub(crate) fn attach_parked_tab_for_superseded_open(
        &mut self,
        path: &std::path::Path,
        title: String,
    ) {
        if self.document_state.tabs.iter().any(|tab| tab.path == path) {
            return;
        }
        self.attach_parked_schematic_tab(path.to_path_buf(), title);
    }

    /// An open result landed for a path a document now owns (a Save As got
    /// there first, #1089): applying it would push a second tab and replace
    /// the owner's engine with disk content, losing its edits. The caller
    /// drops the result; this logs why, including that an `Err` was dropped.
    pub(crate) fn is_open_result_superseded<T, E>(
        &self,
        path: &std::path::Path,
        result: &Result<T, E>,
    ) -> bool {
        if !self.path_is_owned_by_open_document(path) {
            return false;
        }
        let what = match result {
            Ok(_) => "result",
            Err(_) => "failure (its error was dropped too)",
        };
        crate::diagnostics::log_warning(format!(
            "[open] A document already owns that path, so the open {what} was dropped: {}",
            super::save::file_label(path)
        ));
        true
    }

    /// #478 review — dedup guard shared by `open_schematic_file` /
    /// `open_pcb_file`. Mirrors the activate-existing convention in
    /// `handle_open_primitive`: if `path` already has a tab, activate
    /// it. If it doesn't, but an async open for it is already in
    /// flight (spawned by a previous call, not yet completed), do
    /// nothing and let that completion create the tab — spawning a
    /// second `Task::perform` for the same path would let both
    /// completions push a tab aliasing one `document_state.engines`
    /// entry, and closing one would orphan the other. Returns `true`
    /// when the caller must stop and not spawn a new open `Task`.
    fn activate_or_skip_duplicate_open(&mut self, path: &std::path::Path) -> bool {
        if self.activate_tab_for_path(path) {
            return true;
        }
        // A save is still writing `path`: a read now would return the old
        // bytes as a clean tab. The write lands as the re-keyed tab.
        if self.document_state.document_saves.is_saving(path) {
            crate::diagnostics::log_warning(format!(
                "[open] A save to that file (or a case variant of its name) is still running, \
                 so it was not opened; try again once it finishes: {}",
                super::save::file_label(path)
            ));
            return true;
        }
        self.document_state.pending_opens.contains(path)
    }

    /// Activate the tab that owns `path`, if any. Returns whether one exists.
    pub(crate) fn activate_tab_for_path(&mut self, path: &std::path::Path) -> bool {
        let Some(idx) = self.document_state.tabs.iter().position(|t| t.path == path) else {
            return false;
        };
        if idx != self.document_state.active_tab {
            self.park_active_schematic_session();
            self.document_state.active_tab = idx;
            self.sync_active_tab();
        }
        true
    }

    fn open_schematic_file(&mut self, path: PathBuf) -> Result<iced::Task<Message>> {
        let (path, probe) = match self.gate_open(path) {
            OpenGate::Proceed { path, probe } => (path, probe),
            OpenGate::Deferred(probe) => return Ok(probe),
        };
        let task = self.open_schematic_resolved(path)?;
        Ok(iced::Task::batch([probe, task]))
    }

    fn open_schematic_resolved(&mut self, path: PathBuf) -> Result<iced::Task<Message>> {
        if self.activate_or_skip_duplicate_open(&path) {
            return Ok(iced::Task::none());
        }

        // Try to load the companion project so the schematic tab gets
        // a `project_id` via `project_for_path`. Best-effort: a missing
        // or unparseable `.snxprj` doesn't block opening the loose
        // schematic.
        let mut companion_mounts = iced::Task::none();
        if let Some(dir) = path.parent() {
            let stem = path
                .file_stem()
                .and_then(|segment| segment.to_str())
                .unwrap_or("");
            let companion = dir.join(format!("{stem}.snxprj"));
            if companion.exists() {
                match self.load_or_activate_project(&companion) {
                    // The companion project's cold `.snxlib` mounts
                    // prepare off-thread (#99 part 2c); its task is
                    // batched with this document's own read below so
                    // neither waits on the other.
                    Ok((_, mount_task)) => companion_mounts = mount_task,
                    Err(error) => {
                        crate::diagnostics::log_error("Failed to load companion project", &error);
                    }
                }
            }
        }
        let title = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .unwrap_or_else(|| "Schematic".to_string());
        // Parked-engine restore — same Altium-parity rule as the
        // project-tree open path. Reparsing from disk would discard
        // edits the user made before closing the tab.
        if self.document_state.engines.contains_key(&path)
            && self.document_state.dirty_paths.contains(&path)
        {
            self.attach_parked_schematic_tab(path, title);
            return Ok(iced::Task::none());
        }
        // Read + parse off the UI thread — `update()` must never block
        // on disk IO (MVU rule 3). Mirrors `refresh_history_panel`'s
        // `Task::perform` + `spawn_blocking` pattern: the completion
        // message (`FileMsg::SchematicOpenFinished`) carries the
        // original path/title so the dispatch handler opens the tab
        // exactly as this function used to do inline.
        //
        // Mark the path in flight right before spawning — cleared in
        // both arms of `FileMsg::SchematicOpenFinished` (#478 review).
        self.document_state.pending_opens.insert(path.clone());
        let read_path = path.clone();
        // Batched, not sequenced: the companion project's library mounts
        // and this schematic's own read are independent, so neither waits
        // on the other (#99 part 2c).
        Ok(iced::Task::batch([
            companion_mounts,
            iced::Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || read_and_parse_schematic(&read_path))
                        .await
                        .unwrap_or_else(|e| Err(format!("spawn_blocking: {e}")))
                },
                move |result| {
                    Message::File(FileMsg::SchematicOpenFinished {
                        path,
                        title,
                        result: result.map(Box::new),
                    })
                },
            ),
        ]))
    }

    fn open_pcb_file(&mut self, path: PathBuf) -> Result<iced::Task<Message>> {
        let (path, probe) = match self.gate_open(path) {
            OpenGate::Proceed { path, probe } => (path, probe),
            OpenGate::Deferred(probe) => return Ok(probe),
        };
        let task = self.open_pcb_resolved(path)?;
        Ok(iced::Task::batch([probe, task]))
    }

    fn open_pcb_resolved(&mut self, path: PathBuf) -> Result<iced::Task<Message>> {
        if self.activate_or_skip_duplicate_open(&path) {
            return Ok(iced::Task::none());
        }

        // Same companion-project resolution as `open_schematic_file` so
        // the PCB tab can resolve `project_id` for project-scoped
        // handlers.
        let mut companion_mounts = iced::Task::none();
        if let Some(dir) = path.parent() {
            let stem = path
                .file_stem()
                .and_then(|segment| segment.to_str())
                .unwrap_or("");
            let companion = dir.join(format!("{stem}.snxprj"));
            if companion.exists() {
                match self.load_or_activate_project(&companion) {
                    // The companion project's cold `.snxlib` mounts
                    // prepare off-thread (#99 part 2c); its task is
                    // batched with this document's own read below so
                    // neither waits on the other.
                    Ok((_, mount_task)) => companion_mounts = mount_task,
                    Err(error) => {
                        crate::diagnostics::log_error("Failed to load companion project", &error);
                    }
                }
            }
        }
        let title = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .unwrap_or_else(|| "PCB".to_string());
        // Read + parse off the UI thread — same reasoning as
        // `open_schematic_file` above. Mark the path in flight right
        // before spawning — cleared in both arms of
        // `FileMsg::PcbOpenFinished` (#478 review).
        self.document_state.pending_opens.insert(path.clone());
        let read_path = path.clone();
        // Batched, not sequenced — same reasoning as
        // `open_schematic_file` above (#99 part 2c).
        Ok(iced::Task::batch([
            companion_mounts,
            iced::Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || read_and_parse_pcb(&read_path))
                        .await
                        .unwrap_or_else(|e| Err(format!("spawn_blocking: {e}")))
                },
                move |result| {
                    Message::File(FileMsg::PcbOpenFinished {
                        path,
                        title,
                        result: result.map(Box::new),
                    })
                },
            ),
        ]))
    }
}

/// Read + parse a `.snxsch` off the UI thread (the `spawn_blocking` body
/// for `open_schematic_file`). Stringifies the full `anyhow` context
/// chain so the error survives the `Task::perform` boundary (`Message`
/// is `Clone`; `anyhow::Error` is not) — same shape as `HistoryLoaded`'s
/// `Result<_, String>`.
fn read_and_parse_schematic(
    path: &std::path::Path,
) -> Result<signex_types::schematic::SchematicSheet, String> {
    std::fs::read_to_string(path)
        .with_context(|| format!("read schematic {}", path.display()))
        .and_then(|text| {
            signex_types::format::SnxSchematic::parse(&text)
                .with_context(|| format!("parse schematic {}", path.display()))
                .map(|schematic| schematic.sheet)
        })
        .map_err(|err| format!("{err:#}"))
}

/// Read + parse a `.snxpcb` off the UI thread — see
/// `read_and_parse_schematic`.
fn read_and_parse_pcb(path: &std::path::Path) -> Result<signex_types::pcb::PcbBoard, String> {
    std::fs::read_to_string(path)
        .with_context(|| format!("read pcb {}", path.display()))
        .and_then(|text| {
            signex_types::format::SnxPcb::parse(&text)
                .with_context(|| format!("parse pcb {}", path.display()))
                .map(|pcb| pcb.board)
        })
        .map_err(|err| format!("{err:#}"))
}

#[cfg(test)]
mod tests {
    use crate::app::Signex;

    #[test]
    fn loading_a_project_starts_its_volume_probe_exactly_once() {
        let dir = tempfile::tempdir().expect("tempdir");
        let project = dir.path().join("P.snxprj");
        std::fs::write(&project, b"").expect("write project");
        let (mut app, _task) = Signex::new();

        let (_id, first) = app.load_or_activate_project(&project).expect("load");
        let (_id, again) = app.load_or_activate_project(&project).expect("reactivate");

        assert_eq!(first.units(), 1, "the probe is the only task: no libraries");
        assert_eq!(
            again.units(),
            0,
            "an already loaded project is not probed again"
        );
    }

    /// Write a project whose rules record a profile (or none).
    fn project_recording_profile(
        dir: &std::path::Path,
        profile: Option<&str>,
    ) -> std::path::PathBuf {
        let path = dir.join("P.snxprj");
        std::fs::write(&path, b"").expect("marker");
        let mut data = signex_types::project::parse_project(&path).expect("parse");
        if let Some(name) = profile {
            let rules = signex_types::project::ProjectRules {
                profile: Some(signex_types::project::RuleProfileRef {
                    name: name.into(),
                    source: "user".into(),
                    sha256: "recorded".into(),
                    ..Default::default()
                }),
                ..Default::default()
            };
            data.rules = Some(rules);
        }
        signex_types::project::write_project(&path, &data).expect("write");
        path
    }

    #[test]
    fn opening_a_project_that_recorded_a_profile_starts_the_drift_check_once() {
        let dir = tempfile::tempdir().expect("tempdir");
        let project = project_recording_profile(dir.path(), Some("Company"));
        let (mut app, _task) = Signex::new();

        let (_id, first) = app.load_or_activate_project(&project).expect("load");
        assert_eq!(
            first.units(),
            2,
            "the volume probe plus the off-thread drift check"
        );
        let (_id, again) = app.load_or_activate_project(&project).expect("reactivate");
        assert_eq!(again.units(), 0, "re-activating checks nothing again");
    }

    #[test]
    fn opening_a_project_without_a_recorded_profile_checks_nothing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let project = project_recording_profile(dir.path(), None);
        let (mut app, _task) = Signex::new();
        let (_id, task) = app.load_or_activate_project(&project).expect("load");
        assert_eq!(task.units(), 1, "only the volume probe");
    }

    #[test]
    fn a_project_already_told_about_drift_is_not_checked_again() {
        let dir = tempfile::tempdir().expect("tempdir");
        let project = project_recording_profile(dir.path(), Some("Company"));
        let (mut app, _task) = Signex::new();
        let (id, _) = app.load_or_activate_project(&project).expect("load");
        app.document_state.profile_drift_noted.insert(id);
        assert_eq!(app.check_profile_drift_task().units(), 0);
    }
}
