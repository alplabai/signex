//! Library subsystem command helpers.
//!
//! Thin wrappers around `LibraryAdapter` calls that the dispatcher
//! pulls in — keeping the dispatch file small and the iced glue out
//! of the library layer. Each helper takes `&mut LibraryState` and
//! returns a `Result`; non-fatal errors surface via `tracing::warn!`
//! with structured fields, matching the rest of the codebase.

use std::path::{Path, PathBuf};

use signex_library::adapters::local_git::LibraryInitOptions;
use signex_library::{
    ComponentClass, ComponentRow, DatasheetRef, FORMAT_TOKEN, InternalPn, LibraryError,
    LibrarySection, LifecycleState, LocalGitAdapter, ManufacturerPart, ParamMap, PlmReserved,
    PrimitiveRef, RowId, SnxlibManifest, UsersConfig, WorkflowConfig, hash_row_content,
};
// Legacy `Manifest` / `LibraryMeta` / `LibraryMode` imports were retired
// when `LocalGitAdapter::init` switched to the new `SnxlibManifest`
// shape. The remaining sub-stages (Stage 13 workflow mode, Stage 14
// versioning) will introduce richer manifest fields; keeping the
// imports tight here keeps the v0.9-snxlib-as-file refactor auditable.
use signex_types::project::{LibraryEntry, ProjectData};
use uuid::Uuid;

use super::state::LibraryState;
use crate::app::CaseFold;
use crate::app::library_entry::library_entry_location;

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
/// `.snxlib` via `create_pending_library`). Closes
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

/// Names Windows refuses as a file stem regardless of extension.
const RESERVED_STEMS: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Shared pre-flight for a new `.snxlib` path: extension, portable
/// non-empty stem, and the target must not already exist. Rejecting
/// here beats a confusing adapter-init error later.
pub(crate) fn validate_new_library_path(lib_path: &Path) -> Result<(), LibraryError> {
    let ext_ok = lib_path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("snxlib"));
    if !ext_ok {
        return Err(LibraryError::Conflict(format!(
            "library path must end with `.snxlib`: {}",
            lib_path.display()
        )));
    }
    let stem = lib_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    if stem.is_empty() {
        return Err(LibraryError::Conflict(
            "library name (filename stem) is empty".to_string(),
        ));
    }
    if stem.chars().any(|c| {
        c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
    }) || stem.ends_with(['.', ' '])
        || RESERVED_STEMS
            .iter()
            .any(|r| r.eq_ignore_ascii_case(stem.split('.').next().unwrap_or(stem)))
    {
        return Err(LibraryError::Conflict(format!(
            "library name {stem:?} is not a portable file name"
        )));
    }
    if lib_path.exists() {
        return Err(LibraryError::Conflict(format!(
            "{} already exists",
            lib_path.display()
        )));
    }
    Ok(())
}

/// The suggested `.snxlib` for a new library of the project `stem` in
/// `project_dir`: `<project_dir>/<stem>-lib/<stem>-lib.snxlib`. The library
/// lives in a subfolder of its own so a version-controlled project and its
/// library each get their own repository (a library at the project folder
/// would reopen the project's repo). A name is taken when its folder exists,
/// when the old-style `<project_dir>/<name>.snxlib` exists, or when a library
/// in `pending` (registered, not yet written) sits at either path; a taken
/// name gets a `-2`, `-3`, … suffix.
pub fn default_new_library_path(project_dir: &Path, stem: &str, pending: &[PathBuf]) -> PathBuf {
    let candidate = |name: &str| project_dir.join(name).join(format!("{name}.snxlib"));
    let flat = |name: &str| project_dir.join(format!("{name}.snxlib"));
    let taken = |name: &str| {
        project_dir.join(name).exists()
            || flat(name).exists()
            || pending
                .iter()
                .any(|p| *p == candidate(name) || *p == flat(name))
    };
    let base = format!("{stem}-lib");
    if !taken(&base) {
        return candidate(&base);
    }
    (2..=99)
        .map(|n| format!("{stem}-lib-{n}"))
        .find(|name| !taken(name))
        .map_or_else(|| candidate(&base), |name| candidate(&name))
}

/// Register a library creation request without touching disk.
///
/// Validates via `validate_new_library_path` (extension must be
/// `.snxlib`, stem non-empty and portable, target must not exist),
/// pre-mints a `library_id`, and stashes a
/// [`PendingLibrarySpec`] under that id on the caller-provided
/// pending map. The actual `.snxlib` directory + manifest +
/// optional `git init` happen at project-save time via
/// [`create_pending_library`].
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
    validate_new_library_path(&lib_path)?;
    let stem = lib_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_string();

    let library_id = Uuid::now_v7();
    let spec = PendingLibrarySpec {
        lib_path,
        enable_git,
        use_lfs,
        display_name: stem,
    };
    Ok((library_id, spec))
}

/// Create a previously-registered pending library on disk: the `.snxlib/`
/// directory, manifest and git scaffolding, under the pre-minted
/// `library_id` so the manifest matches what the project already stages in
/// `LoadedProject.pending_libraries`.
///
/// Blocking IO and no state: the project save runs it on a worker, then
/// `update` registers and mounts the library only if the project still wants
/// it (`docs/architecture/path-ops.md`, "Project file save"). On failure the
/// entry stays pending so the user can retry on the next save.
pub(crate) fn create_pending_library(
    library_id: Uuid,
    spec: &PendingLibrarySpec,
    defaults: &super::state::NewLibraryDefaults,
) -> Result<(), LibraryError> {
    if spec.lib_path.exists() {
        // A failed or interrupted init leaves this exact library behind.
        // Adopt it only once its version control is what was asked for:
        // finish the missing steps, or refuse and say why. Anything else at
        // the path is not ours to touch.
        if existing_library_id(&spec.lib_path) == Some(library_id) {
            let opts = LibraryInitOptions {
                enable_git: spec.enable_git,
                use_lfs: spec.enable_git && spec.use_lfs,
            };
            return LocalGitAdapter::complete_init(&spec.lib_path, opts).map_err(|e| {
                LibraryError::Conflict(format!(
                    "a partly created library is at {}: version control was not set up ({e})",
                    spec.lib_path.display()
                ))
            });
        }
        return Err(LibraryError::Conflict(format!(
            "{} already exists (pending materialise)",
            spec.lib_path.display()
        )));
    }
    if spec.enable_git
        && let Some(reason) = signex_library::vcs_target_guard::library_repo_refusal(&spec.lib_path)
    {
        return Err(LibraryError::VersionControlRefused(reason));
    }
    let manifest = seed_manifest(library_id, spec.display_name.clone(), defaults);
    LocalGitAdapter::init(
        &spec.lib_path,
        manifest,
        LibraryInitOptions {
            enable_git: spec.enable_git,
            use_lfs: spec.enable_git && spec.use_lfs,
        },
    )?;
    Ok(())
}

/// The `library_id` of the library file already at `path`, if it parses as
/// one. Read from the file alone: a half-made repo beside it must not hide it.
fn existing_library_id(path: &Path) -> Option<Uuid> {
    let text = std::fs::read_to_string(path).ok()?;
    let file = signex_library::library_file::LibraryFile::parse(&text).ok()?;
    Some(file.manifest.library_id)
}

/// Create a library on its own at `lib_path`, not attached to any project:
/// plain files, no version control. Used by the stock-copy flow, which
/// writes straight to the library the user asked for (#813). Refuses a
/// path that is not a new, portable `.snxlib`.
pub(crate) fn create_library_file(
    lib_path: &Path,
    defaults: &super::state::NewLibraryDefaults,
) -> Result<(), LibraryError> {
    validate_new_library_path(lib_path)?;
    let stem = lib_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_string();
    let manifest = seed_manifest(Uuid::now_v7(), stem, defaults);
    LocalGitAdapter::init(lib_path, manifest, LibraryInitOptions::default())?;
    Ok(())
}

/// Manifest for a brand-new library. The class registry is seeded from
/// the user's `prefs.json::component_classes` so a fresh library starts
/// with their preferred taxonomy (editable per-library afterwards);
/// mode/workflow/users stay at their defaults.
pub(crate) fn seed_manifest(
    library_id: Uuid,
    name: String,
    defaults: &super::state::NewLibraryDefaults,
) -> SnxlibManifest {
    let classes = defaults
        .component_classes
        .iter()
        .cloned()
        .map(|e| signex_library::ClassEntry {
            key: e.key,
            label: e.label,
        })
        .collect();
    SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id,
        library: LibrarySection {
            name,
            description: None,
        },
        mode: Default::default(),
        workflow: WorkflowConfig::default(),
        users: UsersConfig::default(),
        classes,
    }
}

/// Register `lib_path` on the project as ProjectLocal or Shared
/// (decided by `library_entry_location`).
pub(crate) fn register_library_entry(
    project: &mut ProjectData,
    lib_path: &Path,
    library_id: Uuid,
    fold: CaseFold,
) {
    let project_dir = PathBuf::from(&project.dir);
    let (path, kind) = library_entry_location(&project_dir, lib_path, fold);
    project.libraries.push(LibraryEntry {
        path,
        kind,
        library_id: Some(library_id),
        unknown: Default::default(),
    });
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
pub fn create_library_at(
    state: &mut LibraryState,
    project: &mut ProjectData,
    lib_path: PathBuf,
    enable_git: bool,
    use_lfs: bool,
    fold: CaseFold,
) -> Result<Uuid, LibraryError> {
    validate_new_library_path(&lib_path)?;
    let stem = lib_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_string();

    let library_id = Uuid::now_v7();
    let manifest = seed_manifest(library_id, stem, &state.new_library_defaults);

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
    register_library_entry(project, &lib_path, library_id, fold);

    // Best-effort runtime mount. Errors here downgrade to warnings so
    // the entry stays registered and the user can retry from the tree
    // later.
    //
    // No `refresh_components` chaser (#530) — same proof as
    // `create_pending_library`: `lib_path` was rejected above if it
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
/// `tests/integration/library_open_cache.rs` pins both halves — that the cold path
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

/// A draft component row, built and ready to be inserted.
pub struct NewComponentRow {
    /// The `.snxlib` of the library the row goes into.
    pub library_root: std::path::PathBuf,
    pub table: String,
    pub row_id: RowId,
    pub row: ComponentRow,
    pub commit_message: String,
    /// "symbol" / "footprint" the row has nothing bound for.
    pub unbound: Vec<&'static str>,
}

/// Build a new component **row** (no IO):
///
/// 1. Builds a [`ComponentRow`] with the user-supplied PN / class and
///    sentinel `Uuid::nil()` symbol/footprint refs (the primitive
///    binding is the user's explicit choice — picked from existing
///    `.snxsym` / `.snxfpt` files post-creation, never auto-minted).
/// 2. Computes the canonical content hash via [`hash_row_content`].
///
/// The caller inserts it with `adapter.insert_row` on a worker (#893) and
/// opens the row as a Component Preview tab via
/// `LibraryMessage::OpenComponentRow` once that has landed. The preview's
/// "Pick Symbol / Pick Footprint" affordance lets the user bind the
/// primitives.
pub fn build_component_row(
    state: &LibraryState,
    library_idx: usize,
    table: &str,
    internal_pn: &str,
    class: ComponentClass,
    symbol_ref: Option<PrimitiveRef>,
    footprint_ref: Option<PrimitiveRef>,
) -> Result<NewComponentRow, LibraryError> {
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

    // Component creation does NOT mint new primitive files: that would
    // write `.snxsym` / `.snxfpt` without an explicit user action. A row
    // created without refs starts with the sentinel `Uuid::nil()`; the
    // browser grid, Properties panel and Component Preview show it as
    // "No symbol bound" / "No footprint bound" with a Pick action, and
    // the creation is reported once to the Messages panel below.

    let row_id = RowId::new();
    let now = chrono::Utc::now();
    let resolved_symbol = symbol_ref.unwrap_or_else(|| PrimitiveRef::new(library_id, Uuid::nil()));
    let mut row = ComponentRow {
        alternate_symbols: Vec::new(),
        row_id: row_id.as_uuid(),
        internal_pn: InternalPn::new(internal_pn),
        class,
        datasheet: DatasheetRef::default(),
        datasheet_label: String::new(),
        description: String::new(),
        more_datasheets: Vec::new(),
        state: LifecycleState::Draft,
        symbol_ref: resolved_symbol,
        footprint_ref,
        alternate_footprints: Vec::new(),
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
        server_revision: None,
    };
    row.content_hash = hash_row_content(&row)?;
    let mut unbound: Vec<&'static str> = Vec::new();
    if row.is_symbol_unbound() {
        unbound.push("symbol");
    }
    if row.is_footprint_unbound() {
        unbound.push("footprint");
    }

    Ok(NewComponentRow {
        library_root,
        table: table.to_string(),
        row_id,
        row,
        commit_message: format!("new component: {internal_pn}"),
        unbound,
    })
}

/// Tell the Messages panel that a freshly created row has nothing bound.
pub(crate) fn report_unbound_row(internal_pn: &str, unbound: &[String]) {
    if unbound.is_empty() {
        return;
    }
    let name = if internal_pn.is_empty() {
        "(unnamed row)"
    } else {
        internal_pn
    };
    tracing::warn!(
        target: "signex::library",
        "New component {name} has no {} bound. Use Pick Symbol / Pick Footprint in the library browser or Properties panel.",
        unbound.join(" and no "),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_library_path_rejects_unportable_names() {
        for bad in [
            "CON.snxlib",
            "nul.snxlib",
            "con.x.snxlib",
            "a..snxlib",
            "x .snxlib",
            "a?b.snxlib",
            "lib.txt",
            ".snxlib",
        ] {
            assert!(validate_new_library_path(Path::new(bad)).is_err(), "{bad}");
        }
        assert!(validate_new_library_path(Path::new("ok-lib.snxlib")).is_ok());
    }

    /// A repo at `dir` with one commit, standing in for a version-controlled
    /// project. Returns the head commit id.
    fn project_repo(dir: &Path) -> git2::Oid {
        signex_library::git_identity::test_identity::install();
        std::fs::write(dir.join("p.snxprj"), "{}").unwrap();
        let repo = git2::Repository::init(dir).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("p.snxprj")).unwrap();
        index.write().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let sig = git2::Signature::now("t", "t@example.org").unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "project", &tree, &[])
            .unwrap()
    }

    fn git_spec(lib_path: PathBuf) -> PendingLibrarySpec {
        register_pending_library(lib_path, true, false).unwrap().1
    }

    #[test]
    fn default_new_library_path_skips_an_old_style_library_and_a_pending_one() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        std::fs::write(dir.join("p-lib.snxlib"), "x").unwrap();
        assert_eq!(
            default_new_library_path(dir, "p", &[]),
            dir.join("p-lib-2").join("p-lib-2.snxlib"),
            "old default library"
        );
        let pending = [dir.join("p-lib-2").join("p-lib-2.snxlib")];
        assert_eq!(
            default_new_library_path(dir, "p", &pending),
            dir.join("p-lib-3").join("p-lib-3.snxlib"),
            "pending library"
        );
    }

    #[test]
    fn default_new_library_path_skips_a_pending_old_style_path() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        let pending = [dir.join("p-lib.snxlib")];
        assert_eq!(
            default_new_library_path(dir, "p", &pending),
            dir.join("p-lib-2").join("p-lib-2.snxlib")
        );
    }

    #[test]
    fn default_new_library_path_is_a_subfolder_and_skips_taken_names() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        assert_eq!(
            default_new_library_path(dir, "p", &[]),
            dir.join("p-lib").join("p-lib.snxlib")
        );
        std::fs::create_dir(dir.join("p-lib")).unwrap();
        assert_eq!(
            default_new_library_path(dir, "p", &[]),
            dir.join("p-lib-2").join("p-lib-2.snxlib")
        );
    }

    #[test]
    fn a_git_library_at_the_new_default_gets_its_own_repo() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        let project_head = project_repo(dir);
        let project_index = std::fs::read(dir.join(".git").join("index")).unwrap();
        let lib = default_new_library_path(dir, "p", &[]);

        create_pending_library(Uuid::now_v7(), &git_spec(lib.clone()), &Default::default())
            .expect("a nested library repo is created");

        let lib_repo = git2::Repository::open(lib.parent().unwrap()).unwrap();
        // Compare canonical paths: macOS reports the temp dir under
        // `/private`, and Windows can hand back an 8.3 short name.
        assert_eq!(
            std::fs::canonicalize(lib_repo.workdir().unwrap()).unwrap(),
            std::fs::canonicalize(lib.parent().unwrap()).unwrap()
        );
        let first = lib_repo.head().unwrap().peel_to_commit().unwrap();
        assert_eq!(first.parent_count(), 0, "the library's own first commit");
        let project = git2::Repository::open(dir).unwrap();
        assert_eq!(project.head().unwrap().target(), Some(project_head));
        assert_eq!(
            std::fs::read(dir.join(".git").join("index")).unwrap(),
            project_index,
            "the project's index is untouched"
        );
    }

    #[test]
    fn a_git_library_at_the_project_folder_is_refused_and_writes_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        project_repo(dir);
        let lib = dir.join("p-lib.snxlib");

        let err =
            create_pending_library(Uuid::now_v7(), &git_spec(lib.clone()), &Default::default())
                .expect_err("the project's repo cannot host the library");

        assert!(
            matches!(err, LibraryError::VersionControlRefused(_)),
            "{err}"
        );
        assert!(!lib.exists(), "nothing written");
    }

    #[test]
    fn a_non_git_project_takes_the_new_default() {
        signex_library::git_identity::test_identity::install();
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        std::fs::write(dir.join("p.snxprj"), "{}").unwrap();
        let lib = default_new_library_path(dir, "p", &[]);
        create_pending_library(Uuid::now_v7(), &git_spec(lib.clone()), &Default::default())
            .expect("created");
        assert!(lib.exists());
        let repo = git2::Repository::open(lib.parent().unwrap()).unwrap();
        assert!(repo.head().unwrap().peel_to_commit().is_ok());
        assert!(!dir.join(".git").exists(), "the project stays unversioned");
    }
}
