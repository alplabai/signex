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

/// Open a `*.snxlib/` and, when it was already mounted, refresh its
/// component list.
///
/// The refresh is warm-path only — the same split #528 made in
/// `auto_mount_project_libraries`, applied here to the interactive path
/// (#530). Not a micro-optimisation: the duplicate scan measured
/// **128.653 ms** for a medium library (500 symbols + 500 footprints) and
/// **540.508 ms** for a large one (2000 + 2000), and every manual "open a
/// library" gesture paid it.
///
/// COLD (not yet mounted): [`LibraryState::open_library`] has just run
/// `reload_tables` → `reload_primitives`, priming all five caches off the
/// exact adapter calls a refresh would repeat. Refreshing here recomputes
/// identical values.
///
/// WARM (already mounted): `open_library` early-returns at
/// `state/methods.rs:83-85` and never reaches `reload_tables`, so this
/// refresh is the *only* thing that rescans. Dropping it unconditionally
/// would leave an already-mounted library showing a stale snapshot —
/// which is why this is a guard and not a deletion. Reachable from every
/// caller: `self.library` is app-global, so re-opening an
/// already-mounted library lands here.
pub fn open_library(state: &mut LibraryState, root: PathBuf) -> Result<(), LibraryError> {
    let already_open = state.library_at(&root).is_some();
    state.open_library(root.clone())?;
    if already_open && let Err(e) = state.refresh_components(&root) {
        tracing::warn!(target: "signex::library", path = %root.display(), error = %e, "refresh_components failed; UI starts with empty list");
    }
    Ok(())
}

// ── Library lifecycle helpers ────────────────────────────────────────

/// Captured shape of a New Library request that hasn't been written
/// to disk yet. Lives on `LoadedProject.pending_libraries` between
/// the user clicking "Create Library" on the Library Options modal
/// and the next successful project save (which materialises the
/// `.snxlib` via `materialize_pending_library`). Closes
/// `feedback_no_disk_writes_without_user_save.md`'s "wait for
/// explicit user save" invariant — modal confirm flips the project
/// dirty bit but leaves disk untouched; only `Ctrl+S` actually
/// writes anything.
#[derive(Debug, Clone)]
pub struct PendingLibrarySpec {
    pub lib_path: PathBuf,
    pub enable_git: bool,
    pub use_lfs: bool,
    /// Tree-display name. Derived from the `.snxlib` stem at register
    /// time so the project tree can show the entry without having to
    /// touch disk.
    pub display_name: String,
}

/// Register a library creation request without touching disk.
///
/// Validates the target path the same way `create_library_at` does
/// (extension must be `.snxlib`, stem non-empty + portable, target
/// must not already exist), pre-mints a `library_id`, and stashes a
/// [`PendingLibrarySpec`] under that id on the caller-provided
/// pending map. The actual `.snxlib` directory + manifest +
/// optional `git init` happen at project-save time via
/// [`materialize_pending_library`].
///
/// Returns the pre-minted `library_id`. Caller is responsible for:
///   1. Storing the `(library_id, PendingLibrarySpec)` on the
///      project's `pending_libraries` map.
///   2. Marking the project dirty so the user knows Save is pending.
pub fn register_pending_library(
    lib_path: PathBuf,
    enable_git: bool,
    use_lfs: bool,
) -> Result<(Uuid, PendingLibrarySpec), LibraryError> {
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
    let spec = PendingLibrarySpec {
        lib_path,
        enable_git,
        use_lfs,
        display_name: stem,
    };
    Ok((library_id, spec))
}

/// Materialise a previously-registered pending library: do the
/// `.snxlib/` directory + manifest + git scaffolding + project
/// registration, using the pre-minted `library_id` so the on-disk
/// manifest matches whatever the user already has staged in
/// `LoadedProject.pending_libraries`.
///
/// This is the disk-write side of the deferred flow. Called from
/// `save_active_project_if_dirty` once per pending entry; on
/// success the caller drains the entry from the pending map. On
/// failure the entry stays pending so the user can retry on next
/// save (e.g. they free up the target path).
///
/// Atomic-with-rollback discipline matches the original
/// `create_library_at` (single library fn, never split across app +
/// lib layers — `feedback_no_disk_writes_without_user_save.md`).
pub fn materialize_pending_library(
    state: &mut LibraryState,
    project: &mut ProjectData,
    library_id: Uuid,
    spec: &PendingLibrarySpec,
) -> Result<(), LibraryError> {
    if spec.lib_path.exists() {
        return Err(LibraryError::Conflict(format!(
            "{} already exists (pending materialise)",
            spec.lib_path.display()
        )));
    }

    let stem = spec.display_name.clone();
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
            name: stem,
            description: None,
        },
        mode: Default::default(),
        workflow: WorkflowConfig::default(),
        users: UsersConfig::default(),
        classes: seed_classes,
    };
    let _adapter = LocalGitAdapter::init(
        &spec.lib_path,
        manifest,
        LibraryInitOptions {
            enable_git: spec.enable_git,
            use_lfs: spec.enable_git && spec.use_lfs,
        },
    )?;

    let project_dir = PathBuf::from(&project.dir);
    let (entry_path, entry_kind) = if !project_dir.as_os_str().is_empty()
        && let Ok(rel) = spec.lib_path.strip_prefix(&project_dir)
    {
        (rel.to_path_buf(), LibraryEntryKind::ProjectLocal)
    } else {
        (spec.lib_path.clone(), LibraryEntryKind::Shared)
    };
    project.libraries.push(LibraryEntry {
        path: entry_path,
        kind: entry_kind,
        library_id: Some(library_id),
    });

    // No `refresh_components` chaser here (#530). This mount is
    // provably cold and the library is provably empty, so a refresh
    // could only recompute what `open_library` just cached:
    //
    // - `spec.lib_path` was rejected above if it existed on disk, and
    //   `LocalGitAdapter::init` created it moments ago. A path that did
    //   not exist cannot have been mounted, because mounting requires
    //   `LocalGitAdapter::open` on an existing path — so the warm branch
    //   that keeps the refresh alive in `open_library` is unreachable.
    // - The library `init` writes has zero rows and no `symbols/` /
    //   `footprints/` / `sims/` entries, so every cache it primes is
    //   empty by construction.
    if let Err(e) = state.open_library(spec.lib_path.clone()) {
        tracing::warn!(
            target: "signex::library",
            path = %spec.lib_path.display(),
            error = %e,
            "freshly-materialised library failed initial open — entry registered, retry from tree"
        );
    }

    Ok(())
}

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

    // Best-effort runtime mount. Errors here downgrade to warnings so
    // the entry stays registered and the user can retry from the tree
    // later.
    //
    // No `refresh_components` chaser (#530) — same proof as
    // `materialize_pending_library`: `lib_path` was rejected above if it
    // existed, `LocalGitAdapter::init` created it moments ago, and a path
    // that did not exist cannot already be mounted. So the mount is
    // provably cold (the warm branch in `open_library` is unreachable)
    // and the library is provably empty (zero rows, no primitive files).
    // A refresh could only recompute the empty caches `open_library` just
    // primed.
    if let Err(e) = state.open_library(lib_path.clone()) {
        tracing::warn!(
            target: "signex::library",
            path = %lib_path.display(),
            error = %e,
            "freshly-created library failed initial open — entry registered, retry from tree"
        );
    }

    Ok(library_id)
}

/// Convenience wrapper — create a project-local library named
/// `<name>` under `<project.dir>/<name>.snxlib`. Keeps the legacy
/// call sites that don't go through the Save-As dialog working
/// (currently none — all new code goes through `create_library_at`,
/// which lets the user pick the location).
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

/// What one `auto_mount_project_libraries` pass decided — issue #99
/// part 2c.
///
/// Two numbers instead of one count, because a cold mount is no longer
/// finished when this function returns: it has only been *recorded*. A
/// single `mounted: usize` would have claimed work that has not happened
/// yet. Total libraries handled is `refreshed + pending.len()`.
#[derive(Debug, Default, Clone)]
pub struct AutoMountOutcome {
    /// Already-mounted libraries refreshed synchronously (the warm path).
    pub refreshed: usize,
    /// `.snxlib` paths recorded as pending mounts. The caller must spawn
    /// one `mount::prepare_mount_off_thread` per path; each lands as a
    /// `LibraryMessage::MountFinished`.
    pub pending: Vec<PathBuf>,
}

/// Auto-mount every library referenced by `project.libraries`. Called
/// once when a project loads. Failures are logged and skipped — a
/// missing or corrupt library shouldn't block the rest of the project
/// from opening.
///
/// **Cold mounts are prepared off the UI thread** (#99 part 2c): this
/// function records them and returns their paths, and the caller fans
/// them out. Six libraries then parse in parallel instead of serially
/// inside `update()` — the measured cost was 826.229 ms after #528, about
/// 50 dropped frames, and a single mount already crosses one 60 Hz frame
/// at ~58 symbols + 58 footprints.
///
/// The `refresh_components` chaser still runs on the warm path **only**.
/// On the cold path the caches are primed by `mount::prepare_mount`
/// (off-thread) exactly as [`LibraryState::open_library`] primed them
/// inline before, so re-running it would be the same pure duplicate work
/// #528 removed: it roughly doubled every project open, costing a
/// six-medium-library project 795.7 ms of its 1 622.571 ms mount.
/// `tests/library_open_cache.rs` pins both halves — that the cold path
/// primes everything, and that the warm path still rescans the primitive
/// directories.
pub fn auto_mount_project_libraries(
    state: &mut LibraryState,
    project: &ProjectData,
) -> AutoMountOutcome {
    use super::mount::{MountIntent, MountRequest};

    let mut refreshed = 0usize;
    let mut pending: Vec<PathBuf> = Vec::new();
    for entry in &project.libraries {
        let resolved = project.resolve_library_path(entry);
        // Standalone `.snxsym` / `.snxfpt` files are tracked on
        // `project.libraries` so the tree shows them, but they're not
        // Component Libraries — skip the mount step. The picker /
        // browser only opens `.snxlib` files; primitive files open as
        // editor tabs via `handle_open_primitive`.
        let is_snxlib = resolved
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("snxlib"))
            .unwrap_or(false);
        if !is_snxlib {
            continue;
        }
        // Cold vs warm mount — do NOT collapse this into an
        // unconditional refresh, and do NOT delete the refresh outright.
        //
        // COLD (not yet mounted): `open_library` has just primed all
        // five caches — `tables` + `cached_components` via
        // `reload_tables`, then `cached_symbols` / `cached_footprints` /
        // `cached_sims` via `reload_primitives` — off exactly the
        // adapter calls a refresh would repeat. Skipping it here is
        // where the whole #99 win lives.
        //
        // WARM (already mounted): `open_library` early-returns `Ok(())`
        // at `state/methods.rs:83-85` and never reaches
        // `reload_tables`, so the refresh below is the only thing that
        // rescans. It is reachable in normal use — `self.library` is
        // app-global, not per-project (see
        // `app/handlers/document_files/open.rs:209`), so a second
        // project referencing an already-mounted `.snxlib` lands here.
        //
        // What the warm refresh actually buys, precisely: `list_symbols`
        // / `list_footprints` / `list_sims` walk `symbols/` /
        // `footprints/` / `sims/` on every call, so it DOES pick up
        // primitive files another process wrote. It does NOT pick up
        // rows added to the `.snxlib` itself — `LocalGitAdapter` parses
        // that once at `open()` into `RwLock<LibraryFile>`
        // (`adapters/local_git/mod.rs:88-92`) and `list_tables` /
        // `read_table` serve from that in-memory copy. Seeing external
        // row edits needs a re-opened adapter, which only
        // `app/handlers/document_files/history.rs:259` does today.
        // COLD vs WARM, decided by `request_mount` rather than by a
        // `library_at` probe here (#99 part 2c). `AlreadyMounted` is the
        // warm path and keeps its synchronous refresh — that refresh is
        // the only thing that rescans `symbols/` / `footprints/` /
        // `sims/`, so dropping it would lose primitive files another
        // process wrote (#528's `library_open_cache.rs` pins this).
        //
        // `Spawn` is the cold path and no longer mounts here: the caller
        // fans the returned paths out through
        // `mount::prepare_mount_off_thread`, so six libraries prepare in
        // parallel off the UI thread instead of serially inside
        // `update()`.
        match state.request_mount(&resolved, MountIntent::Silent) {
            MountRequest::AlreadyMounted => {
                if let Err(e) = state.refresh_components(&resolved) {
                    tracing::warn!(
                        target: "signex::library",
                        path = %resolved.display(),
                        error = %e,
                        "auto-mount: refresh of already-mounted library failed; cache may be stale"
                    );
                }
                refreshed += 1;
            }
            // Another request for this path is already in flight — a
            // second project referencing the same `.snxlib`, or a user
            // double-click that beat the project open. Its completion
            // mounts the library once; recording it twice would spawn a
            // duplicate preparation.
            MountRequest::InFlight => {}
            MountRequest::Spawn => pending.push(resolved),
        }
    }
    AutoMountOutcome { refreshed, pending }
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
    // Inline-add flow (Library Browser "+ Component" button): the row
    // is minted with an empty internal_pn so the user can fill it in
    // via the table's inline cell editor. Lifecycle promotion to
    // Released will require a non-empty PN at that point — the
    // creation step itself stays liberal so the +Component button
    // doesn't need a separate "type a PN first" gate.
    let internal_pn = internal_pn.trim();
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
