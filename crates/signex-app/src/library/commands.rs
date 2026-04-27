//! Library subsystem command helpers.
//!
//! Thin wrappers around `LibraryAdapter` calls that the dispatcher
//! pulls in — keeping the dispatch file small and the iced glue out
//! of the library layer. Each helper takes `&mut LibraryState` and
//! returns a `Result`; non-fatal errors surface via `tracing::warn!`
//! with structured fields, matching the rest of the codebase.

use std::path::{Path, PathBuf};

use signex_library::{
    Component, ComponentClass, ComponentId, ComponentSummary, DatasheetRef, Footprint, InternalPn,
    LibraryError, LibraryMeta, LibraryMode, LifecycleState, LocalGitAdapter, ManufacturerPart,
    Manifest, ParamMap, PlmReserved, PrimitiveRef, Revision, Symbol, UsersConfig, Version,
    WorkflowConfig,
};
use signex_types::project::{LibraryEntry, LibraryEntryKind, ProjectData};
use uuid::Uuid;

use super::state::{ComponentEditorState, EditorAddress, LibraryState};

/// Open a `*.snxlib/` and refresh its component list.
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
// WS-I: tab-not-window — editors are addressed by
// `EditorAddress(library_path, component_id)` instead of by window id.
pub fn save_draft(
    state: &mut LibraryState,
    address: &EditorAddress,
) -> Result<(), LibraryError> {
    let editor = state
        .editors
        .get_mut(address)
        .ok_or_else(|| LibraryError::NotFound(format!("editor {address:?}")))?;
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
// WS-I: tab-not-window
pub fn commit_revision(
    state: &mut LibraryState,
    address: &EditorAddress,
    message: &str,
) -> Result<Revision, LibraryError> {
    let editor = state
        .editors
        .get_mut(address)
        .ok_or_else(|| LibraryError::NotFound(format!("editor {address:?}")))?;
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
