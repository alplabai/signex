//! Enable Version Control flow for the project-navigation dock —
//!
//! Extracted verbatim from the project-navigation dock handlers
//! (`handlers/dock/project_navigation`); pure code motion, zero
//! behaviour change.

use super::*;

impl Signex {
    pub(crate) fn open_enable_version_control_dialog(&mut self, tree_path: Vec<usize>) {
        let Some(&project_idx) = tree_path.first() else {
            return;
        };
        let Some(project) = self.document_state.projects.get(project_idx) else {
            return;
        };
        let Some(project_dir) = project.path.parent() else {
            return;
        };
        if project_dir.join(".git").exists() {
            // Already version-controlled — nothing to enable.
            return;
        }
        let items = collect_track_items(project, project_dir);
        let intro_text = format!(
            "Initialise a Git repository at {} and stage every \
             ticked entry in the project as the first commit. \
             From then on, every save commits through libgit2 — \
             including library mutations inside the project's \
             `.snxlib` directories.",
            project_dir.display()
        );
        self.ui_state.enable_version_control = Some(crate::app::EnableVersionControlState {
            scope: crate::app::VersionControlScope::Project,
            project_path: project.path.clone(),
            project_dir: project_dir.to_path_buf(),
            project_name: project.data.name.clone(),
            items,
            use_lfs: false,
            intro_text,
            error: None,
        });
    }

    /// v0.11 library-node: open the same Enable Version Control modal
    /// scoped to a single `.snxlib` directory rather than the whole
    /// project tree. The library context-menu only surfaces this when
    /// the library's `root_dir` has no `.git/` already.
    pub(crate) fn open_library_enable_version_control_dialog(&mut self, tree_path: Vec<usize>) {
        // Tree path under a project's `Libraries` group is
        // `[project_idx, libraries_branch_idx, library_idx]` — see
        // `library_node_path_from_tree` for the canonical lookup.
        let Some(&project_idx) = tree_path.first() else {
            return;
        };
        let Some(&library_idx) = tree_path.get(2) else {
            return;
        };
        let Some(project) = self.document_state.projects.get(project_idx) else {
            return;
        };
        let Some(entry) = project.data.libraries.get(library_idx) else {
            return;
        };
        let library_file_path = project.data.resolve_library_path(entry);
        let Some(root_dir) = library_file_path.parent().map(|p| p.to_path_buf()) else {
            return;
        };
        if root_dir.join(".git").exists() {
            // Already version-controlled — nothing to enable.
            return;
        }
        let library_name = library_file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("library")
            .to_string();
        let items = collect_track_items_for_library(&root_dir);
        let intro_text = format!(
            "Initialise a Git repository at {} and stage the \
             ticked entries as the first commit. The library \
             adapter will pick up `.git/` on the next save and \
             route subsequent edits through libgit2.",
            root_dir.display()
        );
        // `project_path` is purely informational; point it at a
        // `library.toml` (whether it exists yet or not) inside the
        // library so the modal can mirror the project-scope pattern.
        self.ui_state.enable_version_control = Some(crate::app::EnableVersionControlState {
            scope: crate::app::VersionControlScope::Library,
            project_path: root_dir.join("library.toml"),
            project_dir: root_dir,
            project_name: library_name,
            items,
            use_lfs: false,
            intro_text,
            error: None,
        });
    }

    pub(crate) fn handle_enable_version_control_confirm(&mut self) {
        let Some(state) = self.ui_state.enable_version_control.clone() else {
            return;
        };
        // Build the `.gitignore` body from unticked rows. An empty
        // body (every row ticked) is passed through as `None` so the
        // library skips writing the file entirely — keeps the
        // working tree clean for the all-tracked case. The library
        // handles the actual write + rollback atomically alongside
        // `git init`, so disk state never goes half-applied even on
        // failure.
        let gitignore = build_gitignore_body(&state.items);
        let gitignore_arg = if gitignore.is_empty() {
            None
        } else {
            Some(gitignore.as_str())
        };
        match try_init_project_repo(&state.project_dir, state.use_lfs, gitignore_arg) {
            Ok(()) => {
                // v0.22 Phase 8.6 — for project scope, overwrite the
                // `.gitattributes` with the richer spec
                // (`text eol=lf` for every `.snx*`, `binary` for
                // step/wrl/png/pdf, optional LFS for 3D models). The
                // library helper above wrote LFS-only rules; this
                // adds the cross-platform line-ending discipline +
                // binary markers, then captures the updated file in
                // a follow-up commit so the working tree stays
                // clean.
                //
                // Project-scope only — `.snxlib` libraries already
                // ship their own `.gitattributes` via the original
                // LocalGitAdapter init path.
                if matches!(state.scope, crate::app::VersionControlScope::Project) {
                    // v0.23 — gate `enable_git=true` on the adapter
                    // succeeding. v0.22's path silently swallowed
                    // `open_or_init` failures and still flipped the
                    // flag, leaving the project in a mismatch state.
                    // Now any failure logs through diagnostics AND
                    // skips the flag flip so the user re-runs Enable
                    // VC after fixing the underlying issue.
                    let mut adapter_ok = false;
                    match signex_library::adapters::local_git_project::LocalGitProjectAdapter::open_or_init(
                        state.project_dir.clone(),
                    ) {
                        Ok(adapter) => match adapter.write_gitattributes(state.use_lfs) {
                            Ok(()) => {
                                if let Err(e) = adapter.commit_path(
                                    std::path::Path::new(".gitattributes"),
                                    "Sync .gitattributes to Signex v0.22 spec",
                                ) {
                                    crate::diagnostics::log_warning(format!(
                                        "[git] commit .gitattributes failed: {e}"
                                    ));
                                }
                                adapter_ok = true;
                            }
                            Err(e) => {
                                crate::diagnostics::log_warning(format!(
                                    "[git] write_gitattributes failed: {e} — \
                                     enable_git left off; re-run Enable VC after fix"
                                ));
                            }
                        },
                        Err(e) => {
                            crate::diagnostics::log_warning(format!(
                                "[git] open_or_init failed for {}: {e} — \
                                 enable_git left off; re-run Enable VC after fix",
                                state.project_dir.display()
                            ));
                        }
                    }

                    // Flip `enable_git = true` on the matching
                    // project + mark it dirty so the .snxprj save
                    // captures the flag. Match by `data.dir`. Only
                    // flips when the adapter setup succeeded — see
                    // comment above.
                    if adapter_ok {
                        let target_dir = state.project_dir.clone();
                        let mut snxprj_path: Option<std::path::PathBuf> = None;
                        if let Some(loaded) = self
                            .document_state
                            .projects
                            .iter_mut()
                            .find(|p| std::path::Path::new(&p.data.dir) == target_dir)
                        {
                            loaded.data.enable_git = true;
                            snxprj_path = Some(loaded.path.clone());
                        }
                        if let Some(p) = snxprj_path {
                            self.document_state.dirty_paths.insert(p);
                        }
                    }
                }

                self.ui_state.enable_version_control = None;
                self.refresh_panel_ctx();
                let scope_label = match state.scope {
                    crate::app::VersionControlScope::Project => "repository",
                    crate::app::VersionControlScope::Library => "library repository",
                };
                crate::diagnostics::log_info(format!(
                    "[git] initialised {scope_label} at {}",
                    state.project_dir.display()
                ));
            }
            Err(error) => {
                if let Some(s) = self.ui_state.enable_version_control.as_mut() {
                    s.error = Some(error.to_string());
                }
            }
        }
    }
}

/// Thin wrapper around `signex_library::enable_project_version_control`
/// — kept here so the dispatch handler can stay synchronous and
/// surface the `LibraryError` as a user-facing string. `gitignore`
/// is the body of the `.gitignore` to write before init (one line
/// per pattern, trailing newline); `None` skips the write entirely
/// so a fully-tracked initial commit stays bit-identical.
fn try_init_project_repo(
    project_dir: &std::path::Path,
    use_lfs: bool,
    gitignore: Option<&str>,
) -> Result<(), signex_library::LibraryError> {
    signex_library::enable_project_version_control(project_dir, use_lfs, gitignore)
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
            label: "Project".to_string(),
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
            label: "Schematic".to_string(),
            is_directory: false,
            tracked: true,
        });
    }
    // Optional PCB file.
    if let Some(pcb) = project.data.pcb_file.as_ref() {
        items.push(crate::app::TrackItem {
            absolute: project_dir.join(pcb),
            relative: pcb.clone(),
            label: "PCB".to_string(),
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
            label: "Library".to_string(),
            is_directory: true,
            tracked: true,
        });
    }
    items
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
    let files: &[(&str, &str)] = &[("library.toml", "Config"), ("components.tsv", "Components")];
    for (name, label) in files {
        let abs = root_dir.join(name);
        if !abs.exists() {
            continue;
        }
        items.push(crate::app::TrackItem {
            absolute: abs,
            relative: (*name).to_string(),
            label: (*label).to_string(),
            is_directory: false,
            tracked: true,
        });
    }
    // Canonical subdirectories — surfaced as `Folder` rows. Skip the
    // ones that aren't on disk yet so the picker stays accurate.
    let dirs: &[&str] = &["classes", "symbols", "footprints", "sims", "3dmodels"];
    for name in dirs {
        let abs = root_dir.join(name);
        if !abs.is_dir() {
            continue;
        }
        items.push(crate::app::TrackItem {
            absolute: abs,
            relative: (*name).to_string(),
            label: "Folder".to_string(),
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
