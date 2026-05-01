//! Library subsystem command helpers.
//!
//! Thin wrappers around `LibraryAdapter` calls that the dispatcher
//! pulls in — keeping the dispatch file small and the iced glue out
//! of the library layer. Each helper takes `&mut LibraryState` and
//! returns a `Result`; non-fatal errors surface via `tracing::warn!`
//! with structured fields, matching the rest of the codebase.

use std::path::PathBuf;

use signex_library::adapters::local_git::LibraryInitOptions;
use signex_library::{
    ComponentClass, ComponentRow, ComponentSummary, DatasheetRef, FORMAT_TOKEN, InternalPn,
    LibraryError, LibrarySection, LifecycleState, LocalGitAdapter, ManufacturerPart, ParamMap,
    PlmReserved, PrimitiveRef, RowId, SnxlibManifest, UsersConfig, WorkflowConfig,
    hash_row_content,
};
// Legacy `Manifest` / `LibraryMeta` / `LibraryMode` imports were retired
// when `LocalGitAdapter::init` switched to the new `SnxlibManifest`
// shape. The remaining sub-stages (Stage 13 workflow mode, Stage 14
// versioning) will introduce richer manifest fields; keeping the
// imports tight here keeps the v0.9-snxlib-as-file refactor auditable.
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

// ── Library lifecycle helpers ────────────────────────────────────────

/// Create a fresh `.snxlib/` library at `lib_path`. The directory's
/// final filename stem (`<name>.snxlib`) becomes the library's
/// display name in the manifest. The library is registered on
/// `project.libraries` as `ProjectLocal` when `lib_path` lives
/// inside `project.dir`, otherwise `Shared` — so the same call site
/// handles both the right-click "Add New ▸ Component Library"
/// project-local case and "save my new symbol into a global library
/// directory" shared case.
///
/// `use_lfs` (Stage 11 of `v0.9-snxlib-as-file-plan.md`) controls
/// whether `LocalGitAdapter::init` writes a `.gitattributes` opting
/// `*.step` / `*.stp` / `*.wrl` / `*.iges` into Git LFS at create
/// time. The library-create UI surfaces this through the "Library
/// Options" modal that pops up after the Save-As dialog; non-UI
/// callers (tests, fixtures) pass `false` to stay independent of a
/// local `git lfs` install.
#[allow(clippy::too_many_arguments)]
pub fn create_library_at(
    state: &mut LibraryState,
    project: &mut ProjectData,
    lib_path: PathBuf,
    enable_git: bool,
    use_lfs: bool,
) -> Result<Uuid, LibraryError> {
    // Library directories must use the `.snxlib` extension so the
    // library detector elsewhere (ancestor walk, dock open dialog,
    // adapter resolution) can identify them. Reject anything else
    // up-front rather than failing later with a confusing
    // adapter-init error.
    let ext_ok = lib_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("snxlib"))
        .unwrap_or(false);
    if !ext_ok {
        return Err(LibraryError::Conflict(format!(
            "library path must end with `.snxlib`: {}",
            lib_path.display()
        )));
    }
    let stem = lib_path
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .unwrap_or_default();
    if stem.is_empty() {
        return Err(LibraryError::Conflict(
            "library name (filename stem) is empty".to_string(),
        ));
    }
    if stem
        .chars()
        .any(|c| matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
    {
        return Err(LibraryError::Conflict(format!(
            "library name {stem:?} contains illegal path characters"
        )));
    }
    if lib_path.exists() {
        return Err(LibraryError::Conflict(format!(
            "{} already exists",
            lib_path.display()
        )));
    }

    let library_id = Uuid::now_v7();
    // Seed the per-library class registry from the user's
    // `prefs.json::component_classes` so freshly-created libraries
    // start with the user's preferred taxonomy. The user can edit
    // the registry per-library afterwards via the Library
    // Properties pane (which then writes back to this `.snxlib`).
    let seed_classes: Vec<signex_library::ClassEntry> = crate::fonts::read_component_classes_pref()
        .into_iter()
        .map(|e| signex_library::ClassEntry {
            key: e.key,
            label: e.label,
        })
        .collect();
    let manifest = SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id,
        library: LibrarySection {
            name: stem.clone(),
            description: None,
        },
        // Mode/workflow/users default — Stage 13 will surface the
        // workflow-mode picker (Personal / Team) at create time.
        mode: Default::default(),
        workflow: WorkflowConfig::default(),
        users: UsersConfig::default(),
        classes: seed_classes,
    };

    // LFS opt-in (Stage 11): the "Library Options" modal that pops up
    // after the New Library Save-As dialog feeds `use_lfs` here. The
    // adapter writes `.gitattributes` for `*.step`/`*.stp`/`*.wrl`/
    // `*.iges` and stages it into the initial commit when `true`.
    let _adapter = LocalGitAdapter::init(
        &lib_path,
        manifest,
        LibraryInitOptions {
            enable_git,
            // LFS only matters when version control is on; force off
            // when git is disabled so `.gitattributes` doesn't appear
            // in a non-git directory.
            use_lfs: enable_git && use_lfs,
        },
    )?;

    // Register the library on the project FIRST — the on-disk
    // `.snxlib` is the source of truth, and the project should
    // reference it whether the runtime open/refresh succeeds or
    // not. Putting open_library before this push meant any
    // open-time error (LFS not installed, transient git lock,
    // permission glitch on a fresh dir) bailed the function early
    // with the file already on disk but no LibraryEntry — so the
    // tree never showed the new library and the user had to delete
    // the orphan folder manually.
    let project_dir = PathBuf::from(&project.dir);
    let (entry_path, entry_kind) = if !project_dir.as_os_str().is_empty()
        && let Ok(rel) = lib_path.strip_prefix(&project_dir)
    {
        (rel.to_path_buf(), LibraryEntryKind::ProjectLocal)
    } else {
        (lib_path.clone(), LibraryEntryKind::Shared)
    };

    project.libraries.push(LibraryEntry {
        path: entry_path,
        kind: entry_kind,
        library_id: Some(library_id),
    });

    // Best-effort runtime mount + initial component-cache refresh.
    // Errors here downgrade to warnings so the entry stays
    // registered and the user can retry from the tree later.
    if let Err(e) = state.open_library(lib_path.clone()) {
        tracing::warn!(
            target: "signex::library",
            path = %lib_path.display(),
            error = %e,
            "freshly-created library failed initial open — entry registered, retry from tree"
        );
    } else if let Err(e) = state.refresh_components(&lib_path) {
        tracing::warn!(
            target: "signex::library",
            path = %lib_path.display(),
            error = %e,
            "freshly-created library failed initial refresh"
        );
    }

    Ok(library_id)
}

/// Convenience wrapper — create a project-local library named
/// `<name>` under `<project.dir>/<name>.snxlib`. Keeps the legacy
/// call sites that don't go through the Save-As dialog working
/// (currently none — all new code goes through `create_library_at`,
/// which lets the user pick the location).
#[allow(dead_code)]
pub fn create_library(
    state: &mut LibraryState,
    project: &mut ProjectData,
    name: &str,
) -> Result<Uuid, LibraryError> {
    let trimmed = name.trim().to_string();
    if trimmed.is_empty() {
        return Err(LibraryError::Conflict("library name is empty".to_string()));
    }
    let project_dir = PathBuf::from(&project.dir);
    if project_dir.as_os_str().is_empty() {
        return Err(LibraryError::Conflict(
            "project has no directory on disk yet".to_string(),
        ));
    }
    let lib_path = project_dir.join(format!("{trimmed}.snxlib"));
    // Legacy convenience wrapper — defaults LFS off so existing
    // callers don't change behaviour. UI flows go through the
    // "Library Options" modal which carries `use_lfs` explicitly.
    create_library_at(state, project, lib_path, false, false)
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
// New Component create-flow (components are TSV rows in the DBLib model)
// ─────────────────────────────────────────────────────────────────────

/// Create a new component **row**:
///
/// 1. Builds a [`ComponentRow`] with the user-supplied PN / class and
///    sentinel `Uuid::nil()` symbol/footprint refs (the primitive
///    binding is the user's explicit choice — picked from existing
///    `.snxsym` / `.snxfpt` files post-creation, never auto-minted).
/// 2. Computes the canonical content hash via [`hash_row_content`].
/// 3. Inserts the row into the chosen table via `adapter.insert_row`.
///
/// Returns the new row's `RowId` so the caller can open it as a
/// Component Preview tab via `LibraryMessage::OpenComponentRow`. The
/// preview's "Pick Symbol / Pick Footprint" affordance (Phase 2) lets
/// the user bind the primitives.
pub fn create_component_row(
    state: &mut LibraryState,
    library_idx: usize,
    table: &str,
    internal_pn: &str,
    class: ComponentClass,
    symbol_ref: Option<PrimitiveRef>,
    footprint_ref: Option<PrimitiveRef>,
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

    // Component creation does NOT mint new primitive files. Symbol +
    // footprint are either picked through the Pick Symbol / Pick
    // Footprint affordances inside the New Component modal, or bound
    // later via the Component Preview tab. When the user submits with
    // unbound refs, the row starts with sentinel `Uuid::nil()` and the
    // Component Preview surfaces an "Unbound — pick a symbol" prompt.

    let row_id = RowId::new();
    let now = chrono::Utc::now();
    let resolved_symbol = symbol_ref.unwrap_or_else(|| PrimitiveRef::new(library_id, Uuid::nil()));
    let mut row = ComponentRow {
        row_id: row_id.as_uuid(),
        internal_pn: InternalPn::new(internal_pn),
        class,
        datasheet: DatasheetRef::default(),
        state: LifecycleState::Draft,
        symbol_ref: resolved_symbol,
        footprint_ref,
        sim_ref: None,
        pin_map_overrides: Vec::new(),
        primary_mpn: ManufacturerPart::draft("", ""),
        alternates: Vec::new(),
        supply: Vec::new(),
        parameters: ParamMap::new(),
        plm: PlmReserved::default(),
        // Stage 14: every new row defaults to v0.0.1 + not-released.
        // Personal-mode auto-bumps on save, Team-mode requires the
        // bump dialog once `released` flips to true.
        version: "0.0.1".into(),
        released: false,
        symbol_version: String::new(),
        footprint_version: String::new(),
        sim_version: String::new(),
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
    // `UseSite::version_pinned` is gone in the DBLib model — past
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
