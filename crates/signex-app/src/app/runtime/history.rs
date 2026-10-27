use super::super::*;

impl Signex {
    fn history_source_key(&self) -> Option<std::path::PathBuf> {
        let d = &self.document_state;
        d.tabs.get(d.active_tab).map(|t| t.path.clone())
    }

    /// Idle-cursor-move variant of [`Self::refresh_history_panel`]: only
    /// re-target when the active tab changed (a new target starts a history
    /// load); otherwise just refresh the dirty bit (#111).
    pub(in crate::app) fn refresh_history_panel_cheap(&mut self) -> Task<Message> {
        if self.history_source_key() != self.document_state.history.source_key {
            return self.refresh_history_panel();
        }
        let d = &mut self.document_state;
        if let Some(p) = &d.history.active_path {
            let dirty = history_dirty(&d.tabs, d.active_tab, &d.dirty_paths, p);
            if dirty != d.history.dirty {
                d.history.dirty = dirty;
                d.panel_ctx.history = d.history.clone();
            }
        }
        Task::none()
    }

    /// Recompute the History panel's target from the active tab, bump the
    /// generation counter on change, and return a `Task::perform` that
    /// finds the file's repository and loads its git history off the UI
    /// thread. Called from [`Self::finish_update`] so every dispatch path
    /// that ends with a `finish_update()` call refreshes the panel
    /// automatically. Returns `Task::none()` when the target hasn't
    /// changed since the last refresh. Reads only Model state: the `.git`
    /// search and the Library Browser's directory check run in the task
    /// (#1107).
    pub(super) fn refresh_history_panel(&mut self) -> Task<Message> {
        self.document_state.history.source_key = self.history_source_key();
        let target = history_target(self);

        // No change → nothing to do. Comparing on the target path keeps
        // the panel from re-fetching when an unrelated refresh fires
        // (selection change, theme change, etc.).
        if self.document_state.history.active_path == target {
            // Mirror into the panel ctx in case generation/loading
            // bookkeeping was clobbered by a prior path-less branch.
            // Also refresh the dirty bit — the user may have just
            // saved/edited the active file without switching tabs.
            if let Some(p) = self.document_state.history.active_path.clone() {
                let d = &self.document_state;
                self.document_state.history.dirty =
                    history_dirty(&d.tabs, d.active_tab, &d.dirty_paths, &p);
            }
            self.document_state.panel_ctx.history = self.document_state.history.clone();
            return Task::none();
        }

        self.document_state.history.generation =
            self.document_state.history.generation.wrapping_add(1);
        self.document_state.history.active_path = target.clone();
        self.document_state.history.pending_restore = None;
        self.document_state.history.entries = Vec::new();

        let Some(target) = target else {
            self.document_state.history.loading = false;
            self.document_state.history.dirty = false;
            self.document_state.history.mode =
                crate::panels::history::HistoryRenderMode::NoActiveFile;
            self.document_state.panel_ctx.history = self.document_state.history.clone();
            return Task::none();
        };

        let d = &self.document_state;
        self.document_state.history.dirty =
            history_dirty(&d.tabs, d.active_tab, &d.dirty_paths, &target);
        self.document_state.history.loading = true;
        self.document_state.history.mode = crate::panels::history::HistoryRenderMode::Loading;
        self.document_state.panel_ctx.history = self.document_state.history.clone();

        let generation = self.document_state.history.generation;
        let response_path = target.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || load_history(&target))
                    .await
                    .unwrap_or_else(|e| Err(format!("spawn_blocking: {e}")))
            },
            move |result| {
                Message::Ui(UiMsg::HistoryLoaded {
                    generation,
                    path: response_path.clone(),
                    result,
                })
            },
        )
    }

    /// Apply an async per-file history load, discarding results from a
    /// previous tab (stale `generation`).
    pub(crate) fn apply_history_loaded(
        &mut self,
        generation: u32,
        path: std::path::PathBuf,
        result: Result<crate::panels::history::HistoryLoad, String>,
    ) -> Task<Message> {
        // Drop stale results from a previous tab — the
        // generation token compares cheaply and is the
        // authoritative staleness check (the path field
        // is informational only).
        if generation != self.document_state.history.generation {
            return Task::none();
        }
        self.document_state.history.loading = false;
        match result {
            Ok(crate::panels::history::HistoryLoad::Entries(entries)) => {
                self.document_state.history.mode = crate::panels::history::HistoryRenderMode::Ready;
                self.document_state.history.entries = entries;
            }
            Ok(crate::panels::history::HistoryLoad::NoRepo) => {
                self.document_state.history.mode =
                    crate::panels::history::HistoryRenderMode::NoRepo;
                self.document_state.history.entries = Vec::new();
            }
            // A failed revwalk used to be `unwrap_or_default()`d
            // into an empty `Ready`, which the panel renders as
            // the "No history yet." card — a corrupt object or an
            // unreadable `.git/` presented as "this file has no
            // commits" (#599).
            Err(error) => {
                // `error` before `path`: the Messages panel
                // compacts a record to 160 characters
                // (`diagnostics::compact_message`), and the
                // diagnosis is worth more there than the path,
                // which the panel itself is already showing.
                tracing::error!(
                    target: "signex::history",
                    error = %error,
                    path = %path.display(),
                    "git history could not be read; Restore is unavailable"
                );
                self.document_state.history.mode =
                    crate::panels::history::HistoryRenderMode::Error(error);
                self.document_state.history.entries = Vec::new();
                // This arm does not run `finish_update`, so
                // republish here or the failure sits in the ring
                // buffer until some unrelated later message
                // happens to flush it.
                self.sync_diagnostics_panel_ctx();
            }
        }
        self.document_state.panel_ctx.history = self.document_state.history.clone();
        Task::none()
    }
}

/// Whether the file the History panel tracks has unsaved edits. A Library
/// Browser tab is keyed on its `.snxlib` directory, but the library is
/// tracked (and marked dirty) through its `library.toml`, so both keys are
/// consulted for that tab kind. Pure: no filesystem access.
fn history_dirty(
    tabs: &[crate::app::TabInfo],
    active_tab: usize,
    dirty_paths: &std::collections::HashSet<std::path::PathBuf>,
    path: &std::path::Path,
) -> bool {
    if dirty_paths.contains(path) {
        return true;
    }
    matches!(
        tabs.get(active_tab).map(|t| &t.kind),
        Some(crate::app::TabKind::LibraryBrowser(_))
    ) && dirty_paths.contains(&path.join("library.toml"))
}

/// The file the History panel tracks for the active tab, from Model state
/// alone. `None` when there is no active tab or it has no addressable
/// file (a ComponentEditor tab: the row-shaped editor doesn't write an
/// addressable file in v1; a Project tab: see the arm).
///
/// A Library Browser tab is keyed on its `.snxlib` directory (or the file
/// itself, depending on entry point); whether it is a directory is decided
/// by [`load_history`], off the UI thread.
fn history_target(app: &super::super::Signex) -> Option<std::path::PathBuf> {
    let active = app.document_state.tabs.get(app.document_state.active_tab)?;
    match &active.kind {
        crate::app::TabKind::LibraryBrowser(p) => Some(p.clone()),
        crate::app::TabKind::ComponentEditor(_) => None,
        // A restored `.snxprj` would replace the whole loaded project
        // (libraries, sheets, variants) under every open document; not
        // offered until the Project tab can reload it safely (#1084).
        crate::app::TabKind::Project(_) => None,
        _ => Some(active.path.clone()),
    }
}

/// Blocking: find the repository `target` belongs to and read its history.
/// Run from `spawn_blocking`.
fn load_history(target: &std::path::Path) -> Result<crate::panels::history::HistoryLoad, String> {
    use crate::panels::history::HistoryLoad;
    // A library directory is represented by its `library.toml` (mirrors
    // `LocalGitAdapter::history`'s pathspec handling).
    let full_path = if target.is_dir() {
        target.join("library.toml")
    } else {
        target.to_path_buf()
    };
    let Some((project_dir, rel_path)) = locate_in_repo(&full_path) else {
        return Ok(HistoryLoad::NoRepo);
    };
    signex_library::project_file_history(&project_dir, &rel_path)
        .map(|entries| {
            HistoryLoad::Entries(
                entries
                    .into_iter()
                    .map(|e| signex_widgets::HistoryEntry {
                        sha: e.sha,
                        author_name: e.author_name,
                        author_email: e.author_email,
                        time: e.time,
                        subject: e.subject,
                    })
                    .collect(),
            )
        })
        .map_err(|e| e.to_string())
}

/// The git working tree `full_path` participates in, as
/// `(project_dir, rel_path)` for `signex_library::project_file_history`.
///
/// Discovery walks parent directories looking for a `.git/`. We stop at
/// the first ancestor that has one. For library-rooted files (`.snxsym` /
/// `.snxfpt` etc.) the `.git/` typically sits at the `.snxlib` directory;
/// for project files it sits at the project root. The walk is capped at
/// 12 levels so a misrooted path can't climb forever. Blocking.
fn locate_in_repo(full_path: &std::path::Path) -> Option<(std::path::PathBuf, std::path::PathBuf)> {
    let mut current = full_path.parent();
    for _ in 0..12 {
        let dir = current?;
        if crate::app::state::project_fs::dir_has_git(dir) {
            let rel = full_path.strip_prefix(dir).ok()?;
            return Some((dir.to_path_buf(), rel.to_path_buf()));
        }
        current = dir.parent();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::panels::history::{HistoryLoad, HistoryRenderMode};

    fn tmp() -> tempfile::TempDir {
        tempfile::Builder::new()
            .prefix("snx-1107")
            .tempdir()
            .unwrap()
    }

    fn app_with_tab(path: &std::path::Path) -> crate::app::Signex {
        let (mut app, _boot) = crate::app::Signex::new();
        app.document_state.tabs.push(crate::app::TabInfo {
            title: "s".into(),
            path: path.to_path_buf(),
            cached_document: None,
            dirty: false,
            project_id: None,
            kind: crate::app::TabKind::Schematic,
        });
        app.document_state.active_tab = app.document_state.tabs.len() - 1;
        app
    }

    /// The target comes from the tab alone: whether the file is in a
    /// repository is the task's answer, not something `update` looks up.
    /// (A file outside any repository used to be resolved to NoRepo on the
    /// UI thread, by walking its parents.)
    #[test]
    fn the_target_is_resolved_from_model_state_alone() {
        let dir = tmp();
        let sheet = dir.path().join("untracked.snxsch");
        let mut app = app_with_tab(&sheet);

        let _task = app.refresh_history_panel();

        let history = &app.document_state.history;
        assert_eq!(history.active_path.as_deref(), Some(sheet.as_path()));
        assert_eq!(history.mode, HistoryRenderMode::Loading);
        assert!(history.loading);
    }

    #[test]
    fn a_file_outside_any_repository_loads_as_no_repo() {
        let dir = tmp();
        let sheet = dir.path().join("loose.snxsch");
        std::fs::write(&sheet, "").unwrap();
        assert_eq!(load_history(&sheet), Ok(HistoryLoad::NoRepo));
    }

    #[test]
    fn a_file_in_a_repository_is_found_relative_to_its_root() {
        let dir = tmp();
        std::fs::create_dir(dir.path().join(".git")).unwrap();
        let sheet = dir.path().join("sheets").join("main.snxsch");
        assert_eq!(
            locate_in_repo(&sheet),
            Some((
                dir.path().to_path_buf(),
                std::path::PathBuf::from("sheets/main.snxsch")
            ))
        );
    }

    #[test]
    fn a_library_directory_is_tracked_by_its_library_toml() {
        let dir = tmp();
        let library = dir.path().join("parts.snxlib");
        let repo = git2::Repository::init(&library).unwrap();
        // `load_history` maps the directory to its `library.toml`; the
        // repository found for that file is the library itself.
        let full_path = library.join("library.toml");
        assert_eq!(
            locate_in_repo(&full_path),
            Some((library.clone(), std::path::PathBuf::from("library.toml")))
        );
        // A real, empty repository: the directory loads as an empty history.
        assert_eq!(load_history(&library), Ok(HistoryLoad::Entries(Vec::new())));

        // One commit touching library.toml yields exactly one entry.
        std::fs::write(
            &full_path,
            "name = \"parts\"
",
        )
        .unwrap();
        let mut index = repo.index().unwrap();
        index
            .add_path(std::path::Path::new("library.toml"))
            .unwrap();
        index.write().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "add library.toml", &tree, &[])
            .unwrap();
        match load_history(&library) {
            Ok(HistoryLoad::Entries(entries)) => assert_eq!(entries.len(), 1),
            other => panic!("expected one entry, got {other:?}"),
        }
    }

    /// A Library Browser tab is keyed on the `.snxlib` directory; its dirty
    /// bit lives on `library.toml`.
    #[test]
    fn a_library_browser_tab_reads_dirty_from_library_toml() {
        let dir = tmp();
        let library = dir.path().join("parts.snxlib");
        let (mut app, _boot) = crate::app::Signex::new();
        app.document_state.tabs.push(crate::app::TabInfo {
            title: "parts".into(),
            path: library.clone(),
            cached_document: None,
            dirty: false,
            project_id: None,
            kind: crate::app::TabKind::LibraryBrowser(library.clone()),
        });
        app.document_state.active_tab = app.document_state.tabs.len() - 1;
        app.document_state
            .dirty_paths
            .insert(library.join("library.toml"));

        let _task = app.refresh_history_panel();

        assert!(app.document_state.history.dirty);
    }
}
