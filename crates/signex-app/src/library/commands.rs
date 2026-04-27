//! Library subsystem command helpers.
//!
//! Thin wrappers around `LibraryAdapter` calls that the dispatcher
//! pulls in — keeping the dispatch file small and the iced glue out
//! of the library layer. Each helper takes `&mut LibraryState` and
//! returns a `Result`; non-fatal errors surface via `tracing::warn!`
//! with structured fields, matching the rest of the codebase.

use std::path::PathBuf;

use signex_library::{
    ComponentClass, ComponentRow, ComponentSummary, DatasheetRef, Footprint, InternalPn,
    LibraryError, LibraryMeta, LibraryMode, LifecycleState, LocalGitAdapter, Manifest,
    ManufacturerPart, ParamMap, PlmReserved, PrimitiveRef, RowId, Symbol, UsersConfig,
    WorkflowConfig, hash_row_content,
};
use signex_types::project::{LibraryEntry, LibraryEntryKind, ProjectData};
use uuid::Uuid;

use super::state::LibraryState;

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

    let _adapter = LocalGitAdapter::init(&lib_path, manifest)?;

    state.open_library(lib_path.clone())?;
    if let Err(e) = state.refresh_components(&lib_path) {
        tracing::warn!(
            target: "signex::library",
            path = %lib_path.display(),
            error = %e,
            "freshly-created library failed initial refresh"
        );
    }

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

// ─────────────────────────────────────────────────────────────────────
// WS-8: New Component create-flow (DBLib model — components are rows)
// ─────────────────────────────────────────────────────────────────────

/// Create a new component **row**:
///
/// 1. Mints a [`Symbol`] primitive with one default pin.
/// 2. Mints a [`Footprint`] primitive with no pads.
/// 3. Persists both primitives via `adapter.save_symbol` /
///    `adapter.save_footprint`. Adapters that haven't wired primitive
///    persistence yet keep the in-memory binding so the row insert
///    proceeds.
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

    let symbol_uuid = Uuid::new_v4();
    let footprint_uuid = Uuid::new_v4();
    let mut symbol = Symbol::empty(internal_pn);
    symbol.uuid = symbol_uuid;
    let mut footprint = Footprint::empty(internal_pn);
    footprint.uuid = footprint_uuid;

    let adapter = state
        .set
        .get(library_id)
        .ok_or_else(|| LibraryError::NotFound(format!("library_id={library_id}")))?;

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
        .ok_or_else(|| LibraryError::NotFound(library_root.display().to_string()))?;
    adapter.insert_row(table, row, &commit_msg)?;

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
/// Where-Used handler hands back.
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
