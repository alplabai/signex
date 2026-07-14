use super::super::*;

impl Signex {
    /// Recompute the History panel's target path from the active tab,
    /// bump the generation counter on change, and return a
    /// `Task::perform` that loads the file's git history off the UI
    /// thread. Called from [`Self::finish_update`] so every dispatch
    /// path that ends with a `finish_update()` call refreshes the
    /// panel automatically. Returns `Task::none()` when the target
    /// hasn't changed since the last refresh.
    pub(super) fn refresh_history_panel(&mut self) -> Task<Message> {
        let target = resolve_history_target(self);

        // No change → nothing to do. Comparing on the resolved full
        // path keeps the panel from re-fetching when an unrelated
        // refresh fires (selection change, theme change, etc.).
        let new_active_path = target.as_ref().map(|t| t.full_path().to_path_buf());
        if self.document_state.history.active_path == new_active_path {
            // Mirror into the panel ctx in case generation/loading
            // bookkeeping was clobbered by a prior path-less branch.
            // Also refresh the dirty bit — the user may have just
            // saved/edited the active file without switching tabs.
            if let Some(p) = self.document_state.history.active_path.clone() {
                self.document_state.history.dirty = self.document_state.dirty_paths.contains(&p);
            }
            self.document_state.panel_ctx.history = self.document_state.history.clone();
            return Task::none();
        }

        self.document_state.history.generation =
            self.document_state.history.generation.wrapping_add(1);
        self.document_state.history.active_path = new_active_path.clone();
        self.document_state.history.entries = Vec::new();

        match target {
            None => {
                self.document_state.history.loading = false;
                self.document_state.history.dirty = false;
                self.document_state.history.mode =
                    crate::panels::history::HistoryRenderMode::NoActiveFile;
                self.document_state.panel_ctx.history = self.document_state.history.clone();
                Task::none()
            }
            Some(HistoryTarget::Untracked { full_path }) => {
                self.document_state.history.loading = false;
                self.document_state.history.dirty =
                    self.document_state.dirty_paths.contains(&full_path);
                self.document_state.history.mode =
                    crate::panels::history::HistoryRenderMode::NoRepo;
                self.document_state.panel_ctx.history = self.document_state.history.clone();
                Task::none()
            }
            Some(HistoryTarget::Tracked {
                project_dir,
                rel_path,
                full_path,
            }) => {
                let dirty = self.document_state.dirty_paths.contains(&full_path);
                self.document_state.history.dirty = dirty;
                self.document_state.history.loading = true;
                self.document_state.history.mode =
                    crate::panels::history::HistoryRenderMode::Loading;
                self.document_state.panel_ctx.history = self.document_state.history.clone();

                let generation = self.document_state.history.generation;
                let response_path = full_path.clone();

                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            signex_library::project_file_history(&project_dir, &rel_path)
                        })
                        .await
                        .unwrap_or_else(|e| {
                            Err(signex_library::adapter::LibraryError::Backend(format!(
                                "spawn_blocking: {e}"
                            )))
                        })
                    },
                    move |res| {
                        let mapped = match res {
                            Ok(entries) => Ok(entries
                                .into_iter()
                                .map(|e| signex_widgets::HistoryEntry {
                                    sha: e.sha,
                                    author_name: e.author_name,
                                    author_email: e.author_email,
                                    time: e.time,
                                    subject: e.subject,
                                })
                                .collect()),
                            Err(err) => Err(err.to_string()),
                        };
                        Message::HistoryLoaded {
                            generation,
                            path: response_path.clone(),
                            result: mapped,
                        }
                    },
                )
            }
        }
    }
}

/// What the active tab resolves to for the History panel. `Tracked`
/// means we found a `.git/` ancestor and have a relative pathspec
/// to walk; `Untracked` means we have an on-disk file but no
/// `.git/` was found (the user hasn't enabled version control on
/// this project yet); `None` means the active tab has no
/// addressable file (no tabs at all, or a ComponentEditor tab).
enum HistoryTarget {
    Tracked {
        project_dir: std::path::PathBuf,
        rel_path: std::path::PathBuf,
        full_path: std::path::PathBuf,
    },
    Untracked {
        full_path: std::path::PathBuf,
    },
}

impl HistoryTarget {
    fn full_path(&self) -> &std::path::Path {
        match self {
            HistoryTarget::Tracked { full_path, .. } | HistoryTarget::Untracked { full_path } => {
                full_path.as_path()
            }
        }
    }
}

/// Resolve the active tab into a `(project_dir, rel_path)` pair the
/// History panel can hand to `signex_library::project_file_history`.
///
/// Discovery walks parent directories looking for a `.git/`. We stop
/// at the first ancestor that has one — that's the git working tree
/// the file participates in. For library-rooted files (`.snxsym` /
/// `.snxfpt` etc.) the `.git/` typically sits at the `.snxlib`
/// directory; for project files it sits at the project root.
///
/// Returns `None` for tab kinds that don't correspond to an
/// on-disk file we want to track (e.g. ComponentEditor — the
/// row-shaped editor doesn't write an addressable file in v1).
fn resolve_history_target(app: &super::super::Signex) -> Option<HistoryTarget> {
    let active = app.document_state.tabs.get(app.document_state.active_tab)?;
    let full_path: std::path::PathBuf = match &active.kind {
        // Schematic / Pcb / SymbolEditor / FootprintEditor all carry
        // a real on-disk path on `TabInfo.path`. LibraryBrowser keys
        // on the directory; prefer the `library.toml` inside it as a
        // representative file (mirrors `LocalGitAdapter::history`'s
        // pathspec handling).
        crate::app::TabKind::LibraryBrowser(p) => {
            // The Library Browser tab key is the `.snxlib` directory
            // (or the file path itself, depending on entry point).
            // Fall back to the directory + `library.toml` only when
            // the path is a directory; otherwise treat the file path
            // as the target.
            if p.is_dir() {
                p.join("library.toml")
            } else {
                p.clone()
            }
        }
        crate::app::TabKind::ComponentEditor(_) => return None,
        _ => active.path.clone(),
    };

    // Walk parents until we find a `.git/`. Cap the walk at 12 levels
    // so a misrooted path can't burn cycles climbing forever.
    let mut current = full_path.parent();
    for _ in 0..12 {
        let Some(dir) = current else {
            break;
        };
        if dir.join(".git").exists() {
            let rel = match full_path.strip_prefix(dir) {
                Ok(rel) => rel.to_path_buf(),
                Err(_) => return Some(HistoryTarget::Untracked { full_path }),
            };
            return Some(HistoryTarget::Tracked {
                project_dir: dir.to_path_buf(),
                rel_path: rel,
                full_path,
            });
        }
        current = dir.parent();
    }
    Some(HistoryTarget::Untracked { full_path })
}
