//! Library subsystem command helpers.
//!
//! Thin wrappers around `LibraryAdapter` calls that the dispatcher
//! pulls in — keeping the dispatch file small and the iced glue out
//! of the library layer. Each helper takes `&mut LibraryState` and
//! returns a `Result`; non-fatal errors surface via `tracing::warn!`
//! with structured fields, matching the rest of the codebase.

use std::path::{Path, PathBuf};

use signex_library::{
    ComponentId, LibraryError, LibraryMeta, LibraryMode, LocalGitAdapter, Manifest, Revision,
    UsersConfig, WorkflowConfig,
};
use signex_types::project::{LibraryEntry, LibraryEntryKind, ProjectData};
use uuid::Uuid;

use super::state::{ComponentEditorState, LibraryState};

/// Open a `*.snxlib/` and refresh its component list. Used by both
/// the menu entry and (Phase 2) the auto-open-on-startup flow.
pub fn open_library(state: &mut LibraryState, root: PathBuf) -> Result<(), LibraryError> {
    state.open_library(root.clone())?;
    if let Err(e) = state.refresh_components(&root) {
        tracing::warn!(target: "signex::library", path = %root.display(), error = %e, "refresh_components failed; UI starts with empty list");
    }
    Ok(())
}

// WS-H: Project tree library wiring ─────────────────────────────────
//
// The plan (`v0.9-library-refactor-plan.md` §13) documents
// `create_library(set: &mut LibrarySet, project: &mut Project, name)`
// against a `LibrarySet` API that ships with WS-E. This crate
// currently uses `LibraryState::open_library` (which is the same
// idea — a list of mounted adapters keyed by absolute path), so the
// helper signature wraps that directly. WS-E may rename the
// receiver type without touching the body.

/// Create a fresh project-local library at `<project_dir>/<name>.snxlib/`.
///
/// Steps (per plan §13 H4):
/// 1. Mint a new `library_id` UUID and assemble the default
///    `library.toml` manifest (LocalGit mode, no review required).
/// 2. Hand the directory + manifest to [`LocalGitAdapter::init`],
///    which creates the layout, writes `library.toml`, and seeds
///    the initial commit.
/// 3. Re-open via `LibraryState::open_library` so the adapter
///    registers in `open_libraries` and the panel can drill in.
/// 4. Push a [`LibraryEntry`] onto `project.libraries` so the
///    project tree shows the library on the next refresh and the
///    next project-open auto-mounts it.
///
/// Returns the new `library_id` so the caller can update tabs /
/// breadcrumbs without re-querying the manifest.
pub fn create_library(
    state: &mut LibraryState,
    project: &mut ProjectData,
    name: &str,
) -> Result<Uuid, LibraryError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(LibraryError::Conflict(
            "library name is empty".to_string(),
        ));
    }
    if trimmed
        .chars()
        .any(|c| matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
    {
        return Err(LibraryError::Conflict(format!(
            "library name {trimmed:?} contains illegal path characters"
        )));
    }
    let project_dir = PathBuf::from(&project.dir);
    if project_dir.as_os_str().is_empty() {
        return Err(LibraryError::Conflict(
            "project has no directory on disk yet".to_string(),
        ));
    }
    let dir_name = format!("{trimmed}.snxlib");
    let lib_path = project_dir.join(&dir_name);
    if lib_path.exists() {
        return Err(LibraryError::Conflict(format!(
            "{} already exists",
            lib_path.display()
        )));
    }

    // 1. Manifest with a fresh library_id. Other defaults match the
    //    `LocalGit` mode + open-workflow profile that the new-project
    //    flow expects (no review, single designer role).
    let library_id = Uuid::new_v4();
    let manifest = Manifest {
        library: LibraryMeta {
            name: trimmed.to_string(),
            library_id,
            description: None,
        },
        mode: LibraryMode::default(),
        workflow: WorkflowConfig::default(),
        users: UsersConfig::default(),
    };

    // 2. `LocalGitAdapter::init` lays out the directory, writes
    //    library.toml, runs `git init`, and seeds the initial commit
    //    in one shot. The plan listed each `fs::create_dir_all` call
    //    explicitly (symbols/, footprints/, sims/, components/,
    //    step/, templates/, locks/) — those subdirectories are
    //    populated lazily by the adapter when components are first
    //    written, matching the LIBRARY_PLAN §13 layout.
    let _adapter = LocalGitAdapter::init(&lib_path, manifest)?;

    // 3. Mount via the existing `open_library` helper so the panel
    //    sees the new library immediately and the picker can pull
    //    its empty component list.
    state.open_library(lib_path.clone())?;
    if let Err(e) = state.refresh_components(&lib_path) {
        tracing::warn!(
            target: "signex::library",
            path = %lib_path.display(),
            error = %e,
            "freshly-created library failed initial refresh"
        );
    }

    // 4. Record the library on the project so the project tree
    //    surfaces it under the Libraries node and the next session
    //    auto-mounts.
    project.libraries.push(LibraryEntry {
        path: PathBuf::from(&dir_name),
        kind: LibraryEntryKind::ProjectLocal,
        library_id: Some(library_id),
    });

    Ok(library_id)
}

/// Auto-mount every library referenced by `project.libraries`. Called
/// once when a project loads. Failures are logged and skipped — a
/// missing or corrupt library shouldn't block the rest of the project
/// from opening.
pub fn auto_mount_project_libraries(
    state: &mut LibraryState,
    project: &ProjectData,
) -> usize {
    let mut mounted = 0usize;
    for entry in &project.libraries {
        let resolved = project.resolve_library_path(entry);
        match state.open_library(resolved.clone()) {
            Ok(()) => {
                if let Err(e) = state.refresh_components(&resolved) {
                    tracing::warn!(
                        target: "signex::library",
                        path = %resolved.display(),
                        error = %e,
                        "auto-mount: refresh_components failed"
                    );
                }
                mounted += 1;
            }
            Err(e) => {
                tracing::warn!(
                    target: "signex::library",
                    path = %resolved.display(),
                    error = %e,
                    "auto-mount: open_library failed; skipping"
                );
            }
        }
    }
    mounted
}

/// Build a fresh `ComponentEditorState` for the given `(library, id)`
/// pair. The caller is responsible for opening the editor window and
/// stashing the returned state under the new window's id.
pub fn load_component_for_editor(
    state: &mut LibraryState,
    library_root: &Path,
    id: ComponentId,
) -> Result<ComponentEditorState, LibraryError> {
    let lib = state
        .library_at_mut(library_root)
        .ok_or_else(|| LibraryError::NotFound(library_root.display().to_string()))?;
    let component = lib.adapter.get_component(id)?;
    let review_required = lib.adapter.manifest().workflow.review_required;
    Ok(ComponentEditorState::from_head(
        library_root.to_path_buf(),
        component,
        review_required,
    ))
}

/// Save the editor's draft revision locally. In Phase 1 this just
/// hashes + writes via `save_revision`; the local-git adapter
/// commits behind the scenes. Phase 2 splits "draft" from "commit"
/// using the `parts/.draft/` location.
pub fn save_draft(
    state: &mut LibraryState,
    window_id: iced::window::Id,
) -> Result<(), LibraryError> {
    let editor = state
        .open_editors
        .get_mut(&window_id)
        .ok_or_else(|| LibraryError::NotFound(format!("editor window {window_id:?}")))?;
    editor.draft.refresh_content_hash();
    let library_root = editor.library_root.clone();
    let id = editor.component_id;
    let revision = editor.draft.clone();

    let lib = state
        .library_at_mut(&library_root)
        .ok_or_else(|| LibraryError::NotFound(library_root.display().to_string()))?;
    lib.adapter
        .save_revision(id, revision, "save draft (signex-app phase 1)")?;
    // Refresh so the History tab + Where-Used surface the new revision.
    if let Err(e) = state.refresh_components(&library_root) {
        tracing::warn!(target: "signex::library", path = %library_root.display(), error = %e, "post-save refresh failed");
    }
    Ok(())
}

/// Commit the current draft as a new revision. Phase 1 reuses
/// `save_revision` — the adapter's local-git implementation is the
/// commit. Phase 2 layers an auto-bump dialog and changelog prompt.
pub fn commit_revision(
    state: &mut LibraryState,
    window_id: iced::window::Id,
    message: &str,
) -> Result<Revision, LibraryError> {
    let editor = state
        .open_editors
        .get_mut(&window_id)
        .ok_or_else(|| LibraryError::NotFound(format!("editor window {window_id:?}")))?;
    editor.draft.refresh_content_hash();
    let library_root = editor.library_root.clone();
    let id = editor.component_id;
    let revision = editor.draft.clone();

    let lib = state
        .library_at_mut(&library_root)
        .ok_or_else(|| LibraryError::NotFound(library_root.display().to_string()))?;
    lib.adapter.save_revision(id, revision.clone(), message)?;
    Ok(revision)
}

/// Re-run a query against every open library. Returns the flat list
/// the picker modal renders, deduped only by `(library_path,
/// component uuid)` — a part with the same `internal_pn` in two
/// libraries shows up twice on purpose.
pub fn list_components_filtered(
    state: &LibraryState,
    text_filter: &str,
) -> Vec<(PathBuf, signex_library::ComponentSummary)> {
    let needle = text_filter.trim().to_lowercase();
    state
        .all_components()
        .into_iter()
        .filter(|(_path, summary)| {
            if needle.is_empty() {
                return true;
            }
            summary
                .internal_pn
                .as_str()
                .to_lowercase()
                .contains(&needle)
                || summary.mpn.to_lowercase().contains(&needle)
                || summary.description.to_lowercase().contains(&needle)
        })
        .collect()
}

/// Stub: emit `tracing::info!` with the use-site coordinates the
/// editor's Where-Used tab handed back. Phase 2 replaces this with
/// project navigation + instance selection.
pub fn jump_to_use_site(site: &signex_library::UseSite) {
    tracing::info!(
        target: "signex::library",
        project = %site.project_path.display(),
        sheet = %site.sheet_path.display(),
        instance = %site.instance_id,
        version = %site.version_pinned,
        "jump-to-use-site requested (phase-2 follow-up)"
    );
}

// WS-H: regression tests for `create_library` and
// `auto_mount_project_libraries` are deferred until WS-A/B/E close
// out — those workstreams broke `signex-app`'s test profile by
// reshaping `Revision` (no more `pcb`/`shared`/`schematic` fields),
// and the bin can't currently compile under `cargo test`. The
// `signex-types` tests (`crates/signex-types/src/project.rs`)
// cover the data-model side of WS-H end to end. Once WS-E lands
// and the bin's test profile builds again, the file/dir-scoped
// `create_library` smoke tests live here:
//   - reject empty / path-separator names
//   - smoke: create + manifest exists + project records entry
//   - reject existing-dir clobber
//   - auto-mount smoke: round-trip across sessions
// (Local-tested at WS-H authoring time against a hand-rolled
// LibraryState stub — kept off-tree so the merge doesn't fight
// over WS-E's actual `LibraryState` shape.)
