//! Library subsystem command helpers.
//!
//! Thin wrappers around `LibraryAdapter` calls that the dispatcher
//! pulls in — keeping the dispatch file small and the iced glue out
//! of the library layer. Each helper takes `&mut LibraryState` and
//! returns a `Result`; non-fatal errors surface via `tracing::warn!`
//! with structured fields, matching the rest of the codebase.

use std::path::{Path, PathBuf};

use signex_library::{
    Component, ComponentClass, ComponentId, ComponentSummary, DatasheetRef, Footprint,
    InternalPn, LibraryError, LifecycleState, ManufacturerPart, ParamMap, PlmReserved,
    PrimitiveRef, Revision, Symbol, Version,
};
use uuid::Uuid;

use super::state::{ComponentEditorState, LibraryState};

/// Open a `*.snxlib/` and refresh its component list.
pub fn open_library(state: &mut LibraryState, root: PathBuf) -> Result<(), LibraryError> {
    state.open_library(root.clone())?;
    if let Err(e) = state.refresh_components(&root) {
        tracing::warn!(target: "signex::library", path = %root.display(), error = %e, "refresh_components failed; UI starts with empty list");
    }
    Ok(())
}

/// Build a fresh `ComponentEditorState` for the given `(library, id)` pair.
pub fn load_component_for_editor(
    state: &mut LibraryState,
    library_root: &Path,
    id: ComponentId,
) -> Result<ComponentEditorState, LibraryError> {
    let library_id = state
        .library_at(library_root)
        .map(|lib| lib.library_id)
        .ok_or_else(|| LibraryError::NotFound(library_root.display().to_string()))?;
    let adapter = state
        .set
        .adapter(library_id)
        .ok_or_else(|| LibraryError::NotFound(library_root.display().to_string()))?;
    let component = adapter.get_component(id)?;
    let review_required = adapter.manifest().workflow.review_required;
    Ok(ComponentEditorState::from_head(
        library_root.to_path_buf(),
        component,
        review_required,
    ))
}

/// Save the editor's draft revision locally.
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

    let library_id = state
        .library_at(&library_root)
        .map(|lib| lib.library_id)
        .ok_or_else(|| LibraryError::NotFound(library_root.display().to_string()))?;
    let adapter = state
        .set
        .adapter(library_id)
        .ok_or_else(|| LibraryError::NotFound(library_root.display().to_string()))?;
    adapter.save_revision(id, revision, "save draft (signex-app phase 1)")?;
    if let Err(e) = state.refresh_components(&library_root) {
        tracing::warn!(target: "signex::library", path = %library_root.display(), error = %e, "post-save refresh failed");
    }
    Ok(())
}

/// Commit the current draft as a new revision.
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

    let library_id = state
        .library_at(&library_root)
        .map(|lib| lib.library_id)
        .ok_or_else(|| LibraryError::NotFound(library_root.display().to_string()))?;
    let adapter = state
        .set
        .adapter(library_id)
        .ok_or_else(|| LibraryError::NotFound(library_root.display().to_string()))?;
    adapter.save_revision(id, revision.clone(), message)?;
    Ok(revision)
}

// ─────────────────────────────────────────────────────────────────────
// WS-E: New Component create-flow
// ─────────────────────────────────────────────────────────────────────

/// Errors specific to the New-Component create flow. Wraps both
/// validation issues (UI surface) and adapter persistence errors.
///
/// Hand-rolled `Display` + `From<LibraryError>` (signex-app doesn't
/// pull `thiserror` in directly).
#[derive(Debug)]
pub enum NewComponentError {
    EmptyInternalPn,
    NoLibrarySelected,
    LibraryNotOpen(String),
    Library(LibraryError),
}

impl std::fmt::Display for NewComponentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyInternalPn => f.write_str("internal PN cannot be empty"),
            Self::NoLibrarySelected => f.write_str("pick a target library before submitting"),
            Self::LibraryNotOpen(p) => write!(f, "library not open: {p}"),
            Self::Library(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for NewComponentError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Library(e) => Some(e),
            _ => None,
        }
    }
}

impl From<LibraryError> for NewComponentError {
    fn from(e: LibraryError) -> Self {
        Self::Library(e)
    }
}

/// Result of a successful create — used by the dispatcher to open
/// the editor on the new component.
pub struct CreatedComponent {
    pub library_root: PathBuf,
    pub component_id: ComponentId,
}

/// Create a new draft component:
///
/// 1. Mints a [`Symbol`] primitive with one default pin (`"1"`).
/// 2. Mints a [`Footprint`] primitive with no pads (the user fills
///    these in via the Footprint tab — WS-F).
/// 3. Persists both primitives via `LibrarySet::save_*` (currently a
///    no-op stub — WS-C will wire the adapter).
/// 4. Creates a [`Component`] holding one Draft revision binding both
///    primitives by `PrimitiveRef`, and persists it via
///    `adapter.save_revision`.
///
/// Returns the new component's `(library_root, uuid)` so the caller
/// can open the editor on it.
pub fn create_component(
    state: &mut LibraryState,
    internal_pn: &str,
    library_idx: usize,
    class: ComponentClass,
    category: &str,
) -> Result<CreatedComponent, NewComponentError> {
    let internal_pn = internal_pn.trim();
    if internal_pn.is_empty() {
        return Err(NewComponentError::EmptyInternalPn);
    }
    let library = state
        .open_libraries
        .get(library_idx)
        .ok_or(NewComponentError::NoLibrarySelected)?;
    let library_root = library.root.clone();
    let library_id = library.library_id;

    // Seed the empty primitives.
    let symbol = Symbol::empty(internal_pn);
    let footprint = Footprint::empty(internal_pn);
    let symbol_ref = PrimitiveRef::new(library_id, symbol.uuid);
    let footprint_ref = PrimitiveRef::new(library_id, footprint.uuid);

    // Persist primitives. WS-C wires the real adapter calls; WS-E ships
    // a no-op shim so the create flow runs end-to-end today.
    state
        .set
        .save_symbol(library_id, &symbol, "new component: seed symbol")?;
    state
        .set
        .save_footprint(library_id, &footprint, "new component: seed footprint")?;

    // Build the binding component with one Draft revision.
    let now = chrono::Utc::now();
    let head_version = Version::new(0, 1);
    let mut revision = Revision {
        version: head_version,
        state: LifecycleState::Draft,
        created: now,
        author: String::new(),
        message: format!("draft: {internal_pn}"),
        symbol_ref,
        footprint_ref: Some(footprint_ref),
        sim_ref: None,
        pin_map_overrides: Vec::new(),
        primary_mpn: ManufacturerPart::draft("", ""),
        alternates: Vec::new(),
        supply: Vec::new(),
        datasheet: DatasheetRef::default(),
        parameters: ParamMap::new(),
        plm: PlmReserved::default(),
        content_hash: [0u8; 32],
    };
    revision.refresh_content_hash();

    let category_path = if category.trim().is_empty() {
        std::path::PathBuf::new()
    } else {
        std::path::PathBuf::from(category.trim())
    };

    let component = Component {
        uuid: Uuid::now_v7(),
        internal_pn: InternalPn::new(internal_pn),
        class,
        category: category_path,
        family: None,
        revisions: vec![revision.clone()],
        head: head_version,
    };

    // Persist the component via the adapter.
    let adapter = state
        .set
        .adapter(library_id)
        .ok_or_else(|| NewComponentError::LibraryNotOpen(library_root.display().to_string()))?;
    adapter
        .save_revision(component.uuid, revision, "new component (signex-app)")
        .map_err(NewComponentError::Library)?;

    // Best-effort refresh of the cached component list. Failure here
    // doesn't void the create — surface as a warning only.
    if let Err(e) = state.refresh_components(&library_root) {
        tracing::warn!(
            target: "signex::library",
            path = %library_root.display(),
            error = %e,
            "post-create refresh failed; panel may be stale until next refresh"
        );
    }

    Ok(CreatedComponent {
        library_root,
        component_id: component.uuid,
    })
}

/// Re-run a query against every open library — picker filter helper.
pub fn list_components_filtered(
    state: &LibraryState,
    text_filter: &str,
) -> Vec<(PathBuf, ComponentSummary)> {
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
/// editor's Where-Used tab handed back.
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
