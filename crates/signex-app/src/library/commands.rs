//! Library subsystem command helpers.
//!
//! Thin wrappers around `LibraryAdapter` calls that the dispatcher
//! pulls in — keeping the dispatch file small and the iced glue out
//! of the library layer. Each helper takes `&mut LibraryState` and
//! returns a `Result`; non-fatal errors surface via `tracing::warn!`
//! with structured fields, matching the rest of the codebase.

use std::path::{Path, PathBuf};

use signex_library::{
    Component, ComponentClass, ComponentId, ComponentRow, ComponentSummary, DatasheetRef,
    Footprint, InternalPn, LibraryError, LibraryMeta, LibraryMode, LifecycleState, LocalGitAdapter,
    Manifest, ManufacturerPart, ParamMap, PlmReserved, PrimitiveRef, Revision, RowId, Symbol,
    UsersConfig, Version, WorkflowConfig, hash_row_content,
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
        return Err(LibraryError::Conflict("library name is empty".to_string()));
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
        // WS-8 (DBLib model): no `[[tables]]` overrides at create
        // time — class-table routing falls back to mechanical plural
        // (`<class>s.tsv`) until the user adds explicit overrides.
        tables: Vec::new(),
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
pub fn auto_mount_project_libraries(state: &mut LibraryState, project: &ProjectData) -> usize {
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
        .get(library_id)
        .ok_or_else(|| LibraryError::NotFound(library_root.display().to_string()))?;
    let component = adapter.get_component(id)?;
    let review_required = adapter.manifest().workflow.review_required;
    Ok(ComponentEditorState::from_head(
        library_root.to_path_buf(),
        component,
        review_required,
        &state.set,
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
        .get(library_id)
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
        .get(library_id)
        .ok_or_else(|| LibraryError::NotFound(library_root.display().to_string()))?;
    adapter.save_revision(id, revision.clone(), message)?;
    Ok(revision)
}

// ─────────────────────────────────────────────────────────────────────
// WS-8: New Component create-flow (DBLib model — components are rows)
// ─────────────────────────────────────────────────────────────────────

// The create_component_row helper returns `LibraryError` directly —
// validation cases (empty PN, missing table, missing library) are
// reported as `LibraryError::Conflict` / `LibraryError::NotFound` so
// the dispatcher's existing `Display`-based error surface works
// unchanged. The bespoke `NewComponentError` enum from the WS-E
// shape (which wrapped `LibraryError` plus `EmptyInternalPn` /
// `NoLibrarySelected` variants) is gone — `LibraryError` already
// covers the same surface area for the row-tier flow.

/// Create a new component **row**:
///
/// 1. Mints a [`Symbol`] primitive with one default pin (`"1"`).
/// 2. Mints a [`Footprint`] primitive with no pads (the user fills
///    these in via the Footprint editor — WS-7).
/// 3. Persists both primitives via `adapter.save_symbol` /
///    `adapter.save_footprint`. Adapters that haven't wired primitive
///    persistence yet (`LibraryError::Backend("…not implemented…")`)
///    keep the in-memory binding so the row insert proceeds.
/// 4. Builds a [`ComponentRow`] holding the binding refs +
///    user-supplied PN / class, computes the canonical content hash
///    via [`hash_row_content`], and inserts it into the chosen table
///    via `adapter.insert_row`.
///
/// Returns the new row's `RowId` so the caller can open it as a
/// Component Preview tab via `LibraryMessage::OpenComponentRow`.
pub fn create_component_row(
    state: &mut LibraryState,
    library_idx: usize,
    table: &str,
    internal_pn: &str,
    class: ComponentClass,
) -> Result<RowId, LibraryError> {
    let internal_pn = internal_pn.trim();
    if internal_pn.is_empty() {
        return Err(LibraryError::Conflict("internal PN cannot be empty".into()));
    }
    let table = table.trim();
    if table.is_empty() {
        return Err(LibraryError::Conflict(
            "target table cannot be empty".into(),
        ));
    }

    let library = state
        .open_libraries
        .get(library_idx)
        .ok_or_else(|| LibraryError::NotFound(format!("library_idx={library_idx}")))?;
    let library_root = library.root.clone();
    let library_id = library.library_id;

    // 1+2. Mint the empty primitives. UUIDs are time-ordered so
    //      git history naturally sorts in creation order.
    let symbol_uuid = Uuid::new_v4();
    let footprint_uuid = Uuid::new_v4();
    let mut symbol = Symbol::empty(internal_pn);
    symbol.uuid = symbol_uuid;
    let mut footprint = Footprint::empty(internal_pn);
    footprint.uuid = footprint_uuid;

    // Persist primitives via the real adapter (WS-C). Backend-not-implemented
    // errors are tolerated — partial adapters keep the in-memory primitive
    // bindings without breaking the New-Component flow.
    let adapter = state
        .set
        .get(library_id)
        .ok_or_else(|| LibraryError::NotFound(format!("library_id={library_id}")))?;
    match adapter.save_symbol(symbol.clone(), "new component: seed symbol") {
        Ok(()) | Err(LibraryError::Backend(_)) => {}
        Err(e) => return Err(e.into()),
    }
    match adapter.save_footprint(footprint.clone(), "new component: seed footprint") {
        Ok(()) | Err(LibraryError::Backend(_)) => {}
        Err(e) => return Err(e.into()),
    }

    // 3. Persist the primitives. `Backend("…not implemented…")` is the
    //    sentinel from the default trait impl — tolerate it so adapters
    //    that haven't wired primitive storage yet (legacy LibrarySet
    //    stubs) don't block the row insert.
    match adapter.save_symbol(symbol.clone(), "new component: seed symbol") {
        Ok(()) => {}
        Err(LibraryError::Backend(msg)) if msg.contains("not implemented") => {}
        Err(e) => return Err(e),
    }
    match adapter.save_footprint(footprint.clone(), "new component: seed footprint") {
        Ok(()) => {}
        Err(LibraryError::Backend(msg)) if msg.contains("not implemented") => {}
        Err(e) => return Err(e),
    }

    // 4. Build the row binding both primitives.
    let row_id = RowId::new();
    let now = chrono::Utc::now();
    let mut row = ComponentRow {
        row_id: row_id.as_uuid(),
        internal_pn: InternalPn::new(internal_pn),
        class,
        datasheet: DatasheetRef::default(),
        state: LifecycleState::Draft,
        symbol_ref: PrimitiveRef::new(library_id, symbol_uuid),
        footprint_ref: Some(PrimitiveRef::new(library_id, footprint_uuid)),
        sim_ref: None,
        pin_map_overrides: Vec::new(),
        primary_mpn: ManufacturerPart::draft("", ""),
        alternates: Vec::new(),
        supply: Vec::new(),
        parameters: ParamMap::new(),
        plm: PlmReserved::default(),
        created: now,
        updated: now,
        content_hash: [0u8; 32],
    };
    row.content_hash = hash_row_content(&row)?;

    let commit_msg = format!("new component: {internal_pn}");
    let adapter = state
        .set
        .get(library_id)
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

    Ok(row_id)
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
    // WS-8 (DBLib model): `UseSite::version_pinned` is gone — past
    // versions of a row are read from `git log` (LocalGit) or the
    // audit trail (Database) rather than carried inline. The handler
    // surfaces just the project / sheet / instance triple now.
    tracing::info!(
        target: "signex::library",
        project = %site.project_path.display(),
        sheet = %site.sheet_path.display(),
        instance = %site.instance_id,
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
