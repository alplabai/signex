//! Off-thread `.snxlib` mount — issue #99 part 2c.
//!
//! `update()` must never block on disk IO (MVU rule 3). Mounting a
//! `.snxlib` was the last synchronous file open of any size left in the
//! dispatcher: `LocalGitAdapter::open` parses the library file, then
//! `OpenLibrary::reload_tables` reads every TSV and walks `symbols/` /
//! `footprints/` / `sims/`. A six-medium-library project open still cost
//! **826.229 ms** after #528 dropped the duplicate scan — roughly 50
//! dropped frames at 60 Hz, and a single mount already crosses one frame
//! at ~58 symbols + 58 footprints. The work is CPU-parse-bound, so
//! several libraries prepare in parallel.
//!
//! Shape mirrors the two conversions already in the repo
//! (`open_schematic_file` / `open_pcb_file` in
//! `app/handlers/document_files/open.rs`): `Task::perform` +
//! `tokio::task::spawn_blocking`, an in-flight marker taken before the
//! spawn, and a completion message carrying what the handler needs.
//! Two things differ, both deliberate:
//!
//! 1. The in-flight marker is a **map to an intent**, not a path set.
//!    The same `.snxlib` can be requested by a project auto-mount (no
//!    tab) and by the user double-clicking it (tab follows), and those
//!    two requests race. See [`MountIntent`].
//! 2. The payload is not `Clone`. `Message: Clone` but
//!    `Box<dyn LibraryAdapter>` is not, so it travels in a
//!    [`PreparedMountCell`] the handler `.take()`s. Ugly, and the
//!    alternative is worse: re-running `LocalGitAdapter::open` on the UI
//!    thread at completion time reintroduces exactly the block this
//!    module exists to remove.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use signex_library::{LibraryAdapter, LibraryError, LocalGitAdapter};

use super::state::{LibraryDisplaySettings, LibraryState, OpenLibrary};

/// Why a mount was requested — decides what the completion handler does
/// once the library is on [`LibraryState`].
///
/// A mount already in flight for a path can be re-requested with a
/// *different* intent, and the stronger one has to win. Double-clicking
/// a `.snxlib` while a project auto-mount for the same path is still
/// preparing must still open the browser tab, so `Silent` upgrades to
/// `OpenBrowserTab` — never the reverse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MountIntent {
    /// Project auto-mount. The library has to become resolvable so
    /// `PrimitiveRef`s and the Components Panel find it; no tab opens.
    Silent,
    /// The user opened a `.snxlib`. A `TabKind::LibraryBrowser` tab
    /// follows the mount.
    OpenBrowserTab,
}

impl MountIntent {
    /// Intents only ever escalate. `OpenBrowserTab` absorbs `Silent`;
    /// `Silent` never downgrades an `OpenBrowserTab` already recorded.
    pub fn upgrade(self, other: Self) -> Self {
        match (self, other) {
            (Self::OpenBrowserTab, _) | (_, Self::OpenBrowserTab) => Self::OpenBrowserTab,
            (Self::Silent, Self::Silent) => Self::Silent,
        }
    }
}

/// Everything the UI thread needs to finish a mount, built entirely off
/// the UI thread.
///
/// `entry`'s five caches are already primed here — `reload_tables`
/// populates `tables` + `cached_components` and then chains
/// `reload_primitives` for the three primitive listings. That is the
/// same priming the synchronous [`LibraryState::open_library`] does, so
/// the cold-path invariant #528 pinned still holds: nothing downstream
/// needs a chaser refresh.
pub struct PreparedMount {
    pub(crate) adapter: Box<dyn LibraryAdapter>,
    pub(crate) entry: OpenLibrary,
}

impl PreparedMount {
    /// The `.snxlib` file path this mount was prepared for.
    pub fn path(&self) -> &Path {
        &self.entry.root
    }
}

/// One-shot carrier for a [`PreparedMount`] across the `Task::perform`
/// boundary.
///
/// `LibraryMessage` derives `Debug + Clone`; `PreparedMount` has
/// neither, because `Box<dyn LibraryAdapter>` has neither. The `Arc`
/// makes the *message* cloneable without making the payload cloneable,
/// and [`take`](Self::take) enforces the one-shot contract: whichever
/// clone is dispatched first gets the payload and every later clone sees
/// `None`. iced does not clone a `Task::perform` result today, so the
/// second read should never happen — it degrades to `None` rather than a
/// panic because a dropped mount is a stale cache, not a corrupt one.
#[derive(Clone)]
pub struct PreparedMountCell(Arc<Mutex<Option<Result<PreparedMount, String>>>>);

impl PreparedMountCell {
    fn new(result: Result<PreparedMount, String>) -> Self {
        Self(Arc::new(Mutex::new(Some(result))))
    }

    /// Take the payload. `None` on a second read — see the type docs.
    ///
    /// A poisoned lock is recovered rather than propagated: the only
    /// writer is this type's own `take`, so poisoning means a panic
    /// happened elsewhere while the guard was held and the `Option` is
    /// still structurally sound.
    pub fn take(&self) -> Option<Result<PreparedMount, String>> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take()
    }
}

impl std::fmt::Debug for PreparedMountCell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never reaches for the lock: `Debug` is called from tracing on
        // arbitrary threads and blocking there would be a deadlock risk
        // for zero diagnostic value.
        f.write_str("PreparedMountCell(..)")
    }
}

/// Open the adapter and prime every cache — the `spawn_blocking` body.
///
/// `path` is the `.snxlib` **file** path, not the library root
/// directory. `OpenLibrary::root` holds the file path despite its name
/// (`root_dir()` is the parent), and `LocalGitAdapter::open` agrees;
/// passing a directory fails at runtime through `validate_file_path`
/// with a `Backend` error, not at compile time.
///
/// Errors are stringified because they have to survive the
/// `Task::perform` boundary and `Message` is `Clone` while
/// [`LibraryError`] is not — the same reason `read_and_parse_schematic`
/// returns `Result<_, String>`.
pub fn prepare_mount(path: &Path) -> Result<PreparedMount, String> {
    let adapter = LocalGitAdapter::open(path).map_err(|e: LibraryError| e.to_string())?;
    let manifest = adapter.manifest();
    let mut entry = OpenLibrary {
        root: path.to_path_buf(),
        display_name: manifest.library.name.clone(),
        library_id: manifest.library.library_id,
        tables: HashMap::new(),
        cached_components: Vec::new(),
        cached_symbols: Vec::new(),
        cached_footprints: Vec::new(),
        cached_sims: Vec::new(),
        display: LibraryDisplaySettings::default(),
    };
    // Same tolerance as the synchronous path: one unreadable table must
    // not sink the mount. The library opens with an empty cache and the
    // warn carries the path.
    if let Err(e) = entry.reload_tables(adapter_ref(&adapter)) {
        tracing::warn!(
            target: "signex::library",
            path = %path.display(),
            error = %e,
            "prepare_mount: reload_tables failed; library will mount with an empty cache"
        );
    }
    Ok(PreparedMount {
        adapter: Box::new(adapter),
        entry,
    })
}

/// Widen a concrete adapter to the trait object `reload_tables` takes.
/// Separate fn only so the coercion site reads clearly.
fn adapter_ref(adapter: &LocalGitAdapter) -> &dyn LibraryAdapter {
    adapter
}

/// What [`LibraryState::request_mount`] decided.
///
/// Three outcomes, not two: "already mounted" and "a preparation is in
/// flight" both mean *do not spawn*, but they need opposite follow-ups.
/// An already-mounted path must act **now** (the library is there, so a
/// browser tab can open immediately), while an in-flight path must
/// **not** act now — the completion handler will, once the preparation
/// lands. Collapsing them into one `false` either loses the tab or opens
/// it twice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MountRequest {
    /// The library is already mounted. Nothing was recorded; the caller
    /// should do whatever it wanted to do after the mount, right now.
    AlreadyMounted,
    /// A preparation for this path is already in flight — this call may
    /// have upgraded its intent. The caller must do nothing; the
    /// completion handler acts on the upgraded intent.
    InFlight,
    /// Nothing was in flight. The caller must spawn the preparation.
    Spawn,
}

impl LibraryState {
    /// Record a mount request and report what the caller should do.
    ///
    /// The recorded intent is upgraded in place when a preparation is
    /// already in flight, which is the whole point of the map: the
    /// in-flight preparation finishes and the completion handler reads
    /// the *upgraded* intent rather than one captured at spawn time.
    ///
    /// Already-mounted paths record nothing — `library_at` is the same
    /// idempotence guard `open_library` applies at
    /// `state/methods.rs:83-85`.
    pub fn request_mount(&mut self, path: &Path, intent: MountIntent) -> MountRequest {
        if self.library_at(path).is_some() {
            return MountRequest::AlreadyMounted;
        }
        match self.pending_mounts.get_mut(path) {
            Some(existing) => {
                *existing = existing.upgrade(intent);
                MountRequest::InFlight
            }
            None => {
                self.pending_mounts.insert(path.to_path_buf(), intent);
                MountRequest::Spawn
            }
        }
    }

    /// Take the recorded intent for a finished mount.
    ///
    /// `None` means the request was cancelled while the preparation was
    /// in flight — `close_library` removes the entry, so the map doubles
    /// as the cancellation tombstone and a completion whose path is gone
    /// discards its payload instead of resurrecting a closed library.
    ///
    /// The intent is read **from the map**, never from a value captured
    /// at spawn time. Capturing it would lose the `Silent` →
    /// `OpenBrowserTab` upgrade, which is the subtle half of this design.
    pub fn take_mount_intent(&mut self, path: &Path) -> Option<MountIntent> {
        self.pending_mounts.remove(path)
    }

    /// Finish a mount prepared off-thread. Cheap: a `LibrarySet::mount`
    /// plus two pushes, no disk IO.
    ///
    /// Idempotent in the same way [`LibraryState::open_library`] is — a
    /// path already present wins and the prepared adapter is dropped.
    /// That is reachable: a synchronous `open_library` call site can
    /// mount the same path while this preparation was in flight.
    pub fn mount_prepared(&mut self, prepared: PreparedMount) -> Result<(), LibraryError> {
        if self.library_at(&prepared.entry.root).is_some() {
            return Ok(());
        }
        // `LibrarySet::mount` keys file-backed adapters by `.snxlib`
        // path, so a `Conflict` here means "this same path is already
        // mounted" — not "this library_id is taken". Two copies of a
        // library sharing a `library_id` at different paths both mount
        // fine, by design (`adapters/library_set.rs`, "Resolution
        // semantics").
        self.set.mount(prepared.adapter)?;
        self.open_libraries.push(prepared.entry);
        self.expanded.push(true);
        Ok(())
    }
}

/// Prepare a mount off the UI thread, wrapped for `Task::perform`.
///
/// Split out so both call sites — the browser open and the project
/// auto-mount fan-out — share one mechanism instead of growing two.
pub async fn prepare_mount_off_thread(path: PathBuf) -> PreparedMountCell {
    let result = tokio::task::spawn_blocking(move || prepare_mount(&path))
        .await
        .unwrap_or_else(|e| Err(format!("spawn_blocking: {e}")));
    PreparedMountCell::new(result)
}
