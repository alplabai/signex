//! Library subsystem command helpers.
//!
//! Thin wrappers around `LibraryAdapter` calls that the dispatcher
//! pulls in — keeping the dispatch file small and the iced glue out
//! of the library layer. Each helper takes `&mut LibraryState` and
//! returns a `Result`; non-fatal errors surface via `tracing::warn!`
//! with structured fields, matching the rest of the codebase.

use std::path::{Path, PathBuf};

use signex_library::{ComponentId, LibraryError, Revision};

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
