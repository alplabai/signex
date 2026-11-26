//! Enable Version Control flow for the project-navigation dock.

use super::*;
use crate::app::contracts::{CommitAuthor, TrackKind};

/// The generated-outputs folder; its row starts unticked (mockup row 26).
const OUTPUTS_FOLDER: &str = "outputs";

impl CommitAuthor {
    /// Whether a run may start: git has an author, or both fields are filled.
    pub(crate) fn is_ready(&self) -> bool {
        match self {
            Self::Probing => false,
            Self::Resolved => true,
            Self::Missing { .. } => self.typed().is_some(),
        }
    }

    /// The typed name and email, trimmed; `None` unless both are filled and
    /// git would accept them. Checked by the repo-config writer's own
    /// `parse_author`, so the dialog and the History panel agree.
    pub(crate) fn typed(&self) -> Option<(String, String)> {
        let Self::Missing { name, email } = self else {
            return None;
        };
        let (name, email) = (name.trim(), email.trim());
        signex_library::adapters::local_git::project_repo::parse_author(&format!(
            "{name} <{email}>"
        ))
        .filter(|(n, e)| n == name && e == email)
    }
}

/// Ask git, off the UI thread, whether it resolves a commit author; the
/// dialog asks for one only when it does not (mockup row 26). A probe that
/// fails asks too.
fn probe_commit_author(project_dir: std::path::PathBuf) -> Task<Message> {
    Task::perform(
        crate::app::path_op_jobs::run_blocking(|| {
            signex_library::git_identity::identity_for_default_config().is_ok()
        }),
        move |joined| {
            Message::EnableVersionControl(crate::app::EnableVersionControlMsg::AuthorProbed {
                project_dir: project_dir.clone(),
                has_identity: joined.unwrap_or(false),
            })
        },
    )
}

/// Read which entries the library folder holds, off the UI thread; see
/// `ItemsScanned`.
fn scan_library_items(project_dir: std::path::PathBuf) -> Task<Message> {
    let scan_dir = project_dir.clone();
    Task::perform(
        crate::app::path_op_jobs::run_blocking(move || collect_track_items_for_library(&scan_dir)),
        move |joined| {
            Message::EnableVersionControl(crate::app::EnableVersionControlMsg::ItemsScanned {
                project_dir: project_dir.clone(),
                result: joined,
            })
        },
    )
}

impl Signex {
    pub(crate) fn open_enable_version_control_dialog(
        &mut self,
        tree_path: Vec<usize>,
    ) -> Task<Message> {
        let Some(&project_idx) = tree_path.first() else {
            return Task::none();
        };
        let Some(project) = self.document_state.projects.get(project_idx) else {
            return Task::none();
        };
        let Some(project_dir) = project.path.parent().map(std::path::Path::to_path_buf) else {
            return Task::none();
        };
        if !self.document_state.project_fs.facts.lacks_git(&project_dir) {
            // Already version-controlled (or not probed yet) — nothing to enable.
            return Task::none();
        }
        let items = collect_track_items(project, &project_dir);
        let commit_message = self.first_commit_message(&project.data.name);
        self.ui_state.enable_version_control = Some(crate::app::EnableVersionControlState {
            scope: crate::app::VersionControlScope::Project,
            project_path: project.path.clone(),
            project_dir: project_dir.clone(),
            project_name: project.data.name.clone(),
            items,
            use_lfs: false,
            commit_message,
            commit_every_save: true,
            error: None,
            working: false,
            scanning: false,
            author: CommitAuthor::Probing,
        });
        probe_commit_author(project_dir)
    }

    /// Git answered whether it has a commit author. Without one the dialog
    /// shows the name and email fields. Dropped when the dialog was closed or
    /// reopened for another directory meanwhile.
    pub(crate) fn handle_enable_version_control_author_probed(
        &mut self,
        project_dir: &std::path::Path,
        has_identity: bool,
    ) {
        let Some(state) = self
            .ui_state
            .enable_version_control
            .as_mut()
            .filter(|s| s.author == CommitAuthor::Probing && s.project_dir == project_dir)
        else {
            return;
        };
        state.author = if has_identity {
            CommitAuthor::Resolved
        } else {
            CommitAuthor::Missing {
                name: String::new(),
                email: String::new(),
            }
        };
    }

    /// The first commit message the dialog offers for `name`, in the UI
    /// language. The isolation marks around the name are dropped: the text
    /// becomes a commit subject, not a label.
    fn first_commit_message(&self, name: &str) -> String {
        signex_i18n::t!(
            self.ui_state.i18n.as_ref(),
            signex_i18n::ids::DIALOG_VERSION_CONTROL_COMMIT_MESSAGE_DEFAULT,
            "name" => name
        )
        .into_string()
        .replace(['\u{2068}', '\u{2069}'], "")
    }

    /// An edit in the dialog: a row, Select all, the LFS box, the first
    /// commit message or the Commit every save switch. A run in flight or a
    /// scan still reading the rows has the choices; edits are ignored then.
    pub(crate) fn edit_enable_version_control(
        &mut self,
        edit: crate::app::EnableVersionControlMsg,
    ) {
        use crate::app::EnableVersionControlMsg as Msg;
        let Some(s) = self.ui_state.enable_version_control.as_mut() else {
            return;
        };
        if s.working || s.scanning {
            return;
        }
        match edit {
            Msg::ToggleLfs => s.use_lfs = !s.use_lfs,
            Msg::ToggleItem(idx) => {
                if let Some(item) = s.items.get_mut(idx) {
                    item.tracked = !item.tracked;
                }
            }
            Msg::AllTicked => s.items.iter_mut().for_each(|item| item.tracked = true),
            Msg::CommitMessageChanged(text) => s.commit_message = text,
            Msg::CommitEverySaveToggled => s.commit_every_save = !s.commit_every_save,
            Msg::AuthorNameChanged(text) => {
                if let CommitAuthor::Missing { name, .. } = &mut s.author {
                    *name = text;
                }
            }
            Msg::AuthorEmailChanged(text) => {
                if let CommitAuthor::Missing { email, .. } = &mut s.author {
                    *email = text;
                }
            }
            Msg::Confirm
            | Msg::Close
            | Msg::ItemsScanned { .. }
            | Msg::Finished { .. }
            | Msg::AuthorProbed { .. } => {}
        }
    }

    /// v0.11 library-node: open the same Enable Version Control modal
    /// scoped to a single `.snxlib` directory rather than the whole
    /// project tree. The library context-menu only surfaces this when
    /// the library's `root_dir` has no `.git/` already.
    pub(crate) fn open_library_enable_version_control_dialog(
        &mut self,
        row: crate::app::LibraryRowRef,
    ) -> Task<Message> {
        // Resolved by identity, not row position (#742): a stale row
        // is refused with a Messages-panel warning.
        let Some(project_idx) = self.resolve_library_row(&row, "Enable Version Control") else {
            return Task::none();
        };
        let library_file_path = row.root;
        let Some(root_dir) = library_file_path.parent().map(|p| p.to_path_buf()) else {
            return Task::none();
        };
        // A pending library is not on disk yet, so there is nothing to
        // put under version control. Read from the Model, no stat call.
        if self.document_state.projects[project_idx]
            .pending_libraries
            .values()
            .any(|spec| spec.lib_path == library_file_path)
        {
            crate::diagnostics::log_warning(format!(
                "Enable Version Control cancelled: library {} is not on disk yet.",
                library_file_path.display()
            ));
            return Task::none();
        }
        if !self.document_state.project_fs.facts.lacks_git(&root_dir) {
            // Already version-controlled (or not probed yet) — nothing to enable.
            return Task::none();
        }
        let library_name = library_file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("library")
            .to_string();
        let commit_message = self.first_commit_message(&library_name);
        let project_dir = root_dir.clone();
        // `project_path` is purely informational; point it at a
        // `library.toml` (whether it exists yet or not) inside the
        // library so the modal can mirror the project-scope pattern.
        self.ui_state.enable_version_control = Some(crate::app::EnableVersionControlState {
            scope: crate::app::VersionControlScope::Library,
            project_path: root_dir.join("library.toml"),
            project_dir: root_dir,
            project_name: library_name,
            // Which entries exist is read on a worker; see `ItemsScanned`.
            items: Vec::new(),
            use_lfs: false,
            commit_message,
            // A project setting; the library dialog does not show it.
            commit_every_save: true,
            error: None,
            working: false,
            scanning: true,
            author: CommitAuthor::Probing,
        });
        Task::batch([
            scan_library_items(project_dir.clone()),
            probe_commit_author(project_dir),
        ])
    }

    /// The row list of a library-scope dialog arrived. Dropped when the dialog
    /// was closed or reopened for another directory meanwhile.
    pub(crate) fn handle_enable_version_control_items_scanned(
        &mut self,
        project_dir: &std::path::Path,
        result: Result<Vec<crate::app::TrackItem>, String>,
    ) {
        let Some(state) = self
            .ui_state
            .enable_version_control
            .as_mut()
            .filter(|s| s.scanning && s.project_dir == project_dir)
        else {
            return;
        };
        state.scanning = false;
        match result {
            Ok(items) => state.items = items,
            Err(why) => {
                // The dialog stays usable with no rows (everything tracked).
                let detail = format!(
                    "Could not read the library folder {}: {why}",
                    project_dir.display()
                );
                crate::diagnostics::log_warning(detail.clone());
                state.error = Some(detail);
            }
        }
    }

    /// Start the confirmed run on the blocking pool (#1122): `git init`,
    /// the `.gitignore` / `.gitattributes` writes, staging and the initial
    /// commit can take long on a large project or a network volume. The
    /// dialog stays open, marked working, until
    /// [`Self::handle_enable_version_control_finished`].
    pub(crate) fn handle_enable_version_control_confirm(&mut self) -> Task<Message> {
        let Some(state) = self.ui_state.enable_version_control.as_mut() else {
            return Task::none();
        };
        if state.working || state.scanning {
            return Task::none();
        }
        // A blank subject is refused: the dialog withholds Enable meanwhile.
        let message = state.commit_message.trim().to_string();
        // So is a run before git answered, or with an author field blank.
        if message.is_empty() || !state.author.is_ready() {
            return Task::none();
        }
        let author = state.author.typed();
        state.working = true;
        state.error = None;
        // Build the `.gitignore` body from unticked rows. An empty
        // body (every row ticked) is passed through as `None` so the
        // library skips writing the file entirely — keeps the
        // working tree clean for the all-tracked case. The library
        // handles the actual write + rollback atomically alongside
        // `git init`, so disk state never goes half-applied even on
        // failure.
        let gitignore = build_gitignore_body(&state.items);
        let gitignore = (!gitignore.is_empty()).then_some(gitignore);
        let (scope, project_dir, use_lfs) = (state.scope, state.project_dir.clone(), state.use_lfs);
        let dir = project_dir.clone();
        // The project's "Keep agent journals in git" (#2253) decides whether the
        // block this writes lists them.
        let project_path = state.project_path.clone();
        let keep_journals = self
            .document_state
            .projects
            .iter()
            .find(|p| p.path == project_path)
            .is_none_or(|p| p.data.agent_journal.keep_in_git);
        // The only `.snxprj` the folder may hold (project scope).
        let project_stem = matches!(scope, crate::app::VersionControlScope::Project)
            .then(|| state.project_path.file_stem())
            .flatten()
            .map(|s| s.to_string_lossy().into_owned());
        run_off_thread(scope, project_dir, move || {
            enable_version_control_blocking(
                scope,
                &dir,
                project_stem.as_deref(),
                use_lfs,
                gitignore.as_deref(),
                keep_journals,
                FirstCommit {
                    message: &message,
                    author: author.as_ref().map(|(n, e)| (n.as_str(), e.as_str())),
                },
            )
        })
    }

    /// `.git` now exists under `dir`: record it at once so the menu and the
    /// open guards stop offering Enable before the re-probe lands, then
    /// re-probe to confirm.
    pub(super) fn record_git_present(&mut self, dir: &std::path::Path) {
        self.document_state
            .project_fs
            .facts
            .set_exists(crate::app::state::project_fs::git_marker(dir), true);
        self.document_state.project_fs.invalidate();
    }

    /// Dismiss the dialog (Close button / Esc). Refused while a run is in
    /// flight: the dialog is the only handle on that run, and closing it
    /// would let a second concurrent `git init` start on the same directory.
    pub(crate) fn close_enable_version_control_dialog(&mut self) {
        if self
            .ui_state
            .enable_version_control
            .as_ref()
            .is_some_and(|s| s.working)
        {
            return;
        }
        self.ui_state.enable_version_control = None;
    }

    /// Apply a finished run. The repository and the project's `enable_git`
    /// flag follow `project_dir`; the dialog is only touched when it is
    /// still open for that same directory.
    pub(crate) fn handle_enable_version_control_finished(
        &mut self,
        scope: crate::app::VersionControlScope,
        project_dir: std::path::PathBuf,
        outcome: crate::app::EnableVersionControlOutcome,
    ) {
        use crate::app::EnableVersionControlOutcome as Outcome;
        let dialog_is_this_run = self
            .ui_state
            .enable_version_control
            .as_ref()
            .is_some_and(|s| s.project_dir == project_dir);
        // The dialog cannot change or close while its run is in flight, so
        // its switch is the one the run started with; with no dialog for this
        // run the switch is on, as it opens.
        let commit_every_save = self
            .ui_state
            .enable_version_control
            .as_ref()
            .filter(|_| dialog_is_this_run)
            .is_none_or(|s| s.commit_every_save);
        let (adapter_error, commit_warning) = match outcome {
            Outcome::InitFailed(error) => {
                crate::diagnostics::log_warning(format!(
                    "[git] enable version control failed at {}: {error}",
                    project_dir.display()
                ));
                if dialog_is_this_run && let Some(s) = self.ui_state.enable_version_control.as_mut()
                {
                    s.working = false;
                    s.error = Some(error);
                }
                return;
            }
            Outcome::Created {
                adapter_error,
                commit_warning,
            } => (adapter_error, commit_warning),
        };
        if let Some(warning) = commit_warning {
            crate::diagnostics::log_warning(format!(
                "[git] commit .gitattributes failed: {warning}"
            ));
        }
        let mut project_closed = false;
        if let Some(reason) = &adapter_error {
            crate::diagnostics::log_warning(format!(
                "[git] {reason} at {} — enable_git left off; fix the cause, delete the \
                 project's .git folder, then enable version control again",
                project_dir.display()
            ));
            // The dialog closes below and `.git` now exists, so it cannot be
            // reopened — without this the user sees a normal close with
            // auto-commit still off.
            self.document_state.error_notice =
                Some(crate::app::state::ErrorNotice::version_control(
                    signex_i18n::Translatable::new(signex_i18n::ids::NOTICE_VCS_AUTO_COMMIT_OFF)
                        .arg("path", project_dir.display().to_string())
                        .arg("reason", reason.to_string()),
                ));
        } else if matches!(scope, crate::app::VersionControlScope::Project) && commit_every_save {
            // Turn `enable_git` on for the matching project as a step of its
            // Project tab history (#1693), so the .snxprj save captures the
            // flag and an undo of an older step cannot turn it off. Match by
            // `data.dir`; only when the adapter setup succeeded (v0.23) and
            // the dialog's "Commit every save" was left on.
            let project_id = self
                .document_state
                .projects
                .iter()
                .find(|p| std::path::Path::new(&p.data.dir) == project_dir)
                .map(|p| p.id);
            match project_id {
                Some(id) => {
                    let on = signex_engine::ProjectSectionCommand::SetVersionControl { on: true };
                    if let Err(problem) = self.apply_project_section(id, &on) {
                        crate::diagnostics::log_warning(format!(
                            "[git] commit on save was not turned on: {problem}"
                        ));
                    }
                }
                None => {
                    // The project was closed while the run was in flight: the
                    // repository exists but no loaded project can record it.
                    crate::diagnostics::log_warning(format!(
                        "[git] repository created at {} but its project is no longer \
                         open — enable_git was not recorded",
                        project_dir.display()
                    ));
                    self.document_state.error_notice =
                        Some(crate::app::state::ErrorNotice::version_control(
                            signex_i18n::Translatable::new(
                                signex_i18n::ids::NOTICE_VCS_PROJECT_CLOSED,
                            )
                            .arg("path", project_dir.display().to_string()),
                        ));
                    project_closed = true;
                }
            }
        }

        if dialog_is_this_run {
            self.ui_state.enable_version_control = None;
        }
        self.record_git_present(&project_dir);
        self.refresh_panel_ctx();
        if adapter_error.is_none() && !project_closed {
            let scope_label = match scope {
                crate::app::VersionControlScope::Project => "repository",
                crate::app::VersionControlScope::Library => "library repository",
            };
            crate::diagnostics::log_info(format!(
                "[git] initialised {scope_label} at {}",
                project_dir.display()
            ));
        }
    }
}

/// Run `job` on the blocking pool and report its outcome as `Finished` for
/// `project_dir`.
fn run_off_thread(
    scope: crate::app::VersionControlScope,
    project_dir: std::path::PathBuf,
    job: impl FnOnce() -> crate::app::EnableVersionControlOutcome + Send + 'static,
) -> Task<Message> {
    Task::perform(
        async move {
            tokio::task::spawn_blocking(job).await.unwrap_or_else(|e| {
                crate::app::EnableVersionControlOutcome::InitFailed(format!(
                    "the version-control worker stopped: {e}"
                ))
            })
        },
        move |outcome| {
            Message::EnableVersionControl(crate::app::EnableVersionControlMsg::Finished {
                scope,
                project_dir: project_dir.clone(),
                outcome,
            })
        },
    )
}

/// The first commit: its subject, and the author the dialog asked for when
/// git had none (`None` uses git's own).
#[derive(Debug, Clone, Copy)]
struct FirstCommit<'a> {
    message: &'a str,
    author: Option<(&'a str, &'a str)>,
}

/// Blocking: everything Enable Version Control does on disk. Run from
/// `spawn_blocking`.
fn enable_version_control_blocking(
    scope: crate::app::VersionControlScope,
    project_dir: &std::path::Path,
    project_stem: Option<&str>,
    use_lfs: bool,
    gitignore: Option<&str>,
    keep_journals: bool,
    commit: FirstCommit<'_>,
) -> crate::app::EnableVersionControlOutcome {
    use crate::app::EnableVersionControlOutcome as Outcome;
    // Refuse a folder that holds other projects or personal files (#1205)
    // before any `git init`; the library repeats the stem-less part.
    if let Some(reason) =
        signex_library::vcs_target_guard::system_version_control_refusal(project_dir, project_stem)
    {
        return Outcome::InitFailed(reason);
    }
    if let Err(error) = try_init_project_repo(
        project_dir,
        use_lfs,
        matches!(scope, crate::app::VersionControlScope::Project),
        gitignore,
        keep_journals,
        commit,
    ) {
        return Outcome::InitFailed(error.to_string());
    }
    // `enable_project_version_control` already merged the managed
    // `.gitignore` block (both scopes) and `.gitattributes` block (project
    // scope) before the initial commit, so nothing is left to write here.
    Outcome::Created {
        adapter_error: None,
        commit_warning: None,
    }
}

/// Thin wrapper around `signex_library::enable_project_version_control`
/// — kept here so the dispatch handler can stay synchronous and
/// surface the `LibraryError` as a user-facing string.
/// `project_scope` adds the managed `.gitattributes` block; both scopes
/// get the managed `.gitignore` block. `gitignore` is the picker's unticked
/// rows, merged in outside the block, or `None` for none. `commit` is the
/// first commit's subject and the author the dialog asked for, which the
/// library writes into the new repository's config (mockup row 26).
fn try_init_project_repo(
    project_dir: &std::path::Path,
    use_lfs: bool,
    project_scope: bool,
    gitignore: Option<&str>,
    keep_journals: bool,
    commit: FirstCommit<'_>,
) -> Result<(), signex_library::LibraryError> {
    signex_library::enable_project_version_control_as(
        project_dir,
        use_lfs,
        project_scope,
        gitignore,
        keep_journals,
        commit.message,
        commit.author,
    )
}

/// Build the per-row pick-list for the project-scope Enable Version
/// Control modal. Surfaces the `.snxprj`, every sheet, the pcb file,
/// and each `.snxlib` directory as separately tickable rows so the
/// user can opt expensive folders out of the initial commit.
pub(crate) fn collect_track_items(
    project: &crate::app::state::LoadedProject,
    project_dir: &std::path::Path,
) -> Vec<crate::app::TrackItem> {
    let mut items: Vec<crate::app::TrackItem> = Vec::new();
    // The .snxprj itself — always ticked, can't really be excluded
    // sensibly but we surface it so users see the full picture.
    if let Some(name) = project.path.file_name().and_then(|n| n.to_str()) {
        items.push(crate::app::TrackItem {
            absolute: project.path.clone(),
            relative: name.to_string(),
            kind: TrackKind::Project,
            is_directory: false,
            tracked: true,
        });
    }
    // Schematic sheets registered on the project.
    for sheet in &project.data.sheets {
        let abs = project_dir.join(&sheet.filename);
        items.push(crate::app::TrackItem {
            absolute: abs,
            relative: sheet.filename.clone(),
            kind: TrackKind::Schematic,
            is_directory: false,
            tracked: true,
        });
    }
    // Optional PCB file.
    if let Some(pcb) = project.data.pcb_file.as_ref() {
        items.push(crate::app::TrackItem {
            absolute: project_dir.join(pcb),
            relative: pcb.clone(),
            kind: TrackKind::Pcb,
            is_directory: false,
            tracked: true,
        });
    }
    // Each library directory the project pulls in. Project-local
    // entries materialise as `.snxlib` directories under
    // `project_dir`; the parent of `resolve_library_path()` is the
    // library's working tree.
    for entry in &project.data.libraries {
        let resolved = project.data.resolve_library_path(entry);
        let Some(lib_root) = resolved.parent() else {
            continue;
        };
        // Only surface project-local libraries — shared/global ones
        // live outside the project dir and don't belong in the
        // project-scope `.gitignore`.
        if !lib_root.starts_with(project_dir) {
            continue;
        }
        let relative = lib_root
            .strip_prefix(project_dir)
            .ok()
            .and_then(|p| p.to_str())
            .unwrap_or_default()
            .to_string();
        if relative.is_empty() {
            continue;
        }
        items.push(crate::app::TrackItem {
            absolute: lib_root.to_path_buf(),
            relative,
            kind: TrackKind::Library,
            is_directory: true,
            tracked: true,
        });
    }
    items.push(outputs_item(project_dir));
    items
}

/// The generated-outputs folder, unticked: its files churn with every export,
/// so they stay out of the first commit unless the user ticks it (mockup row
/// 26). Listed whether or not it exists yet, so later outputs stay out too.
fn outputs_item(project_dir: &std::path::Path) -> crate::app::TrackItem {
    crate::app::TrackItem {
        absolute: project_dir.join(OUTPUTS_FOLDER),
        relative: OUTPUTS_FOLDER.to_string(),
        kind: TrackKind::Folder,
        is_directory: true,
        tracked: false,
    }
}

/// Build the per-row pick-list for the library-scope Enable Version
/// Control modal. Each row is a top-level entry inside the library's
/// `root_dir` — the `library.toml` / `components.tsv` manifest pair
/// plus any of the canonical subdirectories (`classes/` / `symbols/`
/// / `footprints/` / `sims/` / `3dmodels/`) that already exist on
/// disk. Entries that don't exist are skipped so the picker only
/// shows real artefacts.
pub(crate) fn collect_track_items_for_library(
    root_dir: &std::path::Path,
) -> Vec<crate::app::TrackItem> {
    let mut items: Vec<crate::app::TrackItem> = Vec::new();
    // Manifest-shaped files at the library root. Only surface them
    // when present — bare-bones libraries may carry only the
    // `.snxlib` file without a sidecar config.
    let files: &[(&str, TrackKind)] = &[
        ("library.toml", TrackKind::Config),
        ("components.tsv", TrackKind::Components),
    ];
    for (name, kind) in files {
        let abs = root_dir.join(name);
        if !abs.exists() {
            continue;
        }
        items.push(crate::app::TrackItem {
            absolute: abs,
            relative: (*name).to_string(),
            kind: *kind,
            is_directory: false,
            tracked: true,
        });
    }
    // Canonical subdirectories — surfaced as `Folder` rows. Skip the
    // ones that aren't on disk yet so the picker stays accurate.
    for name in signex_library::adapters::local_git::LIBRARY_SUBFOLDERS {
        let abs = root_dir.join(name);
        if !abs.is_dir() {
            continue;
        }
        items.push(crate::app::TrackItem {
            absolute: abs,
            relative: name.to_string(),
            kind: TrackKind::Folder,
            is_directory: true,
            tracked: true,
        });
    }
    items
}

/// Render the user's tick-list into a `.gitignore` body. Returns an
/// empty string when every row is ticked (no exclusions needed) so
/// the caller can skip writing a no-op file. Directory rows get a
/// trailing slash so git matches the directory and its contents.
pub(crate) fn build_gitignore_body(items: &[crate::app::TrackItem]) -> String {
    let mut lines: Vec<String> = Vec::new();
    for item in items {
        if item.tracked {
            continue;
        }
        if item.relative.is_empty() {
            continue;
        }
        // Use forward slashes — git always wants forward slashes
        // even on Windows. `relative` is already forward-slashed for
        // the items we generate, but be defensive.
        let mut pat = item.relative.replace('\\', "/");
        if item.is_directory && !pat.ends_with('/') {
            pat.push('/');
        }
        lines.push(pat);
    }
    if lines.is_empty() {
        String::new()
    } else {
        let mut out = String::from("# Generated by Signex Enable Version Control\n");
        for line in lines {
            out.push_str(&line);
            out.push('\n');
        }
        out
    }
}

#[cfg(test)]
#[path = "version_control_tests.rs"]
mod version_control_tests;

#[cfg(test)]
#[path = "version_control_gitignore_tests.rs"]
mod version_control_gitignore_tests;

#[cfg(test)]
#[path = "version_control_scan_tests.rs"]
mod version_control_scan_tests;
