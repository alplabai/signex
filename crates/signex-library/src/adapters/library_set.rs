//! `LibrarySet` — a tiny resolver that composes any number of
//! [`LibraryAdapter`] trait objects into a single lookup surface for
//! cross-library [`PrimitiveRef`] resolution.
//!
//! Per `v0.9-snxlib-as-file-plan.md` §2 Stage B, the primary mount
//! key is the `.snxlib` *file path* (not the library_id). That plan let
//! one library_id appear under several paths and left dedup to the
//! Components Panel, which never did it; lookups by id then picked
//! either copy (#1894). [`LibrarySet::mount`] now refuses a second mount
//! of a library_id, so the same library listed under Project and Global
//! must be the same file.
//!
//! ## Resolution semantics
//!
//! The resolvers return `Result<Option<T>, LibraryError>` because
//! "the primitive is not there" and "the lookup could not be
//! performed" are different answers and the user needs a different
//! message for each:
//!
//! - `Ok(None)` — the reference genuinely does not resolve. Either the
//!   `library_id` isn't mounted (the editor surfaces this as
//!   "unresolved primitive — open dependent library?") or the
//!   `library_id` IS mounted but the primitive UUID isn't in any of
//!   the matching adapters.
//! - `Err(_)` — the adapter could not tell: I/O on `symbols/`, a
//!   corrupt `.snxsym` / `.snxfpt` on disk, or an HTTP failure from
//!   `DatabaseAdapter` talking to a remote library server. Collapsing
//!   these into `Ok(None)` reports a perfectly good binding as a wrong
//!   one, and sends the user off to re-bind a valid UUID or to hunt a
//!   phantom mount problem.
//!
//! Only [`LibraryError::NotFound`] is evidence of absence; every other
//! variant is a failure to look.
//!
//! - Two adapters never share a `library_id`: [`LibrarySet::mount`]
//!   refuses the second (the "user copy-pasted a library" case, #1894),
//!   so every lookup by id has exactly one answer.
//! - Adapters without an on-disk path (e.g. `DatabaseAdapter`) fall
//!   back to keying by `library_id`; the duplicate-id error still
//!   guards those.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use uuid::Uuid;

use crate::adapter::{LibraryAdapter, LibraryError};
use crate::adapters::read_only::ReadOnlyAdapter;
use crate::primitive::{Footprint, PrimitiveRef, SimModel, Symbol};

/// Mount key for an adapter inside a [`LibrarySet`].
///
/// File-backed adapters (`LocalGitAdapter`) key by their absolute
/// `.snxlib` file path so the same `library_id` mounted at two
/// distinct on-disk locations is allowed. Path-less adapters
/// (`DatabaseAdapter`) fall back to keying by `library_id`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MountKey {
    /// File-backed adapter — the `.snxlib` absolute path.
    Path(PathBuf),
    /// Path-less adapter — the adapter's `library_id`.
    Id(Uuid),
}

impl MountKey {
    fn for_adapter(adapter: &dyn LibraryAdapter) -> Self {
        match adapter.library_file_path() {
            Some(p) => MountKey::Path(p.to_path_buf()),
            None => MountKey::Id(adapter.library_id()),
        }
    }
}

/// A bag of mounted libraries, keyed by [`MountKey`].
///
/// `Default` constructs an empty set; call [`Self::mount`] to add adapters.
#[derive(Default)]
pub struct LibrarySet {
    libs: HashMap<MountKey, Arc<dyn LibraryAdapter>>,
}

impl LibrarySet {
    /// Construct an empty `LibrarySet`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Mount a library.
    ///
    /// Returns `Conflict` when the same mount key is already in use
    /// (duplicate `.snxlib` path, or duplicate `library_id` for a
    /// path-less adapter), and when another mount already carries this
    /// adapter's `library_id` (#1894). Every reference and lookup is by
    /// `library_id`, so two mounts sharing one (a copied library folder)
    /// would make reads and writes land in whichever the set finds first.
    ///
    /// Use [`Self::remount`] to replace an existing mount in one step.
    pub fn mount(&mut self, lib: Box<dyn LibraryAdapter>) -> Result<(), LibraryError> {
        let key = MountKey::for_adapter(lib.as_ref());
        if self.libs.contains_key(&key) {
            return Err(LibraryError::Conflict(match &key {
                MountKey::Path(p) => format!("library at {} is already mounted", p.display()),
                MountKey::Id(id) => format!(
                    "library_id {id} is already mounted — duplicate library_id on \
                     a path-less adapter (DB or in-memory) suggests a misconfig"
                ),
            }));
        }
        let id = lib.library_id();
        if let Some(existing) = self.find_key_for_id(id) {
            let existing = match existing {
                MountKey::Path(p) => p.display().to_string(),
                MountKey::Id(_) => "a server library".to_string(),
            };
            let new = match &key {
                MountKey::Path(p) => p.display().to_string(),
                MountKey::Id(_) => "this library".to_string(),
            };
            return Err(LibraryError::Conflict(format!(
                "{new} has the same library id ({id}) as {existing}, which is already open; \
                 it is probably a copy. Close the other one first, or give the copy its own id"
            )));
        }
        self.libs.insert(key, Arc::from(lib));
        Ok(())
    }

    /// Mount a library read-only: every write through the mounted
    /// adapter fails with [`LibraryError::ReadOnly`] regardless of the
    /// filesystem's permissions. Same conflict rules as [`Self::mount`].
    pub fn mount_read_only(&mut self, lib: Box<dyn LibraryAdapter>) -> Result<(), LibraryError> {
        self.mount(Box::new(ReadOnlyAdapter::new(lib)))
    }

    /// True when the file-backed mount at `path` is read-only. An
    /// unmounted path is not read-only (there is nothing to guard).
    pub fn is_read_only_path(&self, path: &Path) -> bool {
        self.get_by_path(path).is_some_and(|a| a.is_read_only())
    }

    /// Replace whatever adapter is mounted at `lib`'s mount key with
    /// `lib`. Drops the previous adapter and returns it (or `None` if
    /// no previous mount existed).
    ///
    /// Returns `Conflict`, leaving the set unchanged, when another mount
    /// carries `lib`'s `library_id` (#1894): a replacement must not hand
    /// the set a second library under an id it already serves.
    pub fn remount(
        &mut self,
        lib: Box<dyn LibraryAdapter>,
    ) -> Result<Option<Arc<dyn LibraryAdapter>>, LibraryError> {
        let key = MountKey::for_adapter(lib.as_ref());
        let id = lib.library_id();
        if let Some(other) = self
            .libs
            .iter()
            .filter(|(k, a)| **k != key && a.library_id() == id)
            .map(|(k, _)| k)
            .min()
        {
            return Err(LibraryError::Conflict(format!(
                "cannot replace the library mounted at {}: its library id ({id}) \
                 is already used by {}",
                describe_key(&key),
                describe_key(other)
            )));
        }
        Ok(self.libs.insert(key, Arc::from(lib)))
    }

    /// Unmount the adapter matching `library_id` and return it (or `None`
    /// if no adapter is registered under that id). Mounting keeps ids
    /// unique; callers that hold a path should still prefer
    /// [`Self::unmount_by_path`].
    pub fn unmount(&mut self, library_id: Uuid) -> Option<Arc<dyn LibraryAdapter>> {
        let key = self.find_key_for_id(library_id)?;
        self.libs.remove(&key)
    }

    /// Unmount the file-backed adapter at `path`, returning it.
    pub fn unmount_by_path(&mut self, path: &Path) -> Option<Arc<dyn LibraryAdapter>> {
        self.libs.remove(&MountKey::Path(path.to_path_buf()))
    }

    /// Re-key the file-backed adapter mounted at `old` to `new` after the
    /// `.snxlib` was renamed on disk, telling the adapter its new path.
    ///
    /// Atomic: on any failure (`NotFound` for an unmounted `old`,
    /// `Conflict` when `new` is already mounted, the adapter refusing)
    /// the mount at `old` is left exactly as it was.
    pub fn relocate(&mut self, old: &Path, new: &Path) -> Result<(), LibraryError> {
        let old_key = MountKey::Path(old.to_path_buf());
        let new_key = MountKey::Path(new.to_path_buf());
        if self.libs.contains_key(&new_key) {
            return Err(LibraryError::Conflict(format!(
                "library at {} is already mounted",
                new.display()
            )));
        }
        let mut adapter = self
            .libs
            .remove(&old_key)
            .ok_or_else(|| LibraryError::NotFound(old.display().to_string()))?;
        // An off-thread read (`get_shared`) may still hold the adapter for
        // the few milliseconds a table scan takes; relocating needs
        // exclusive access, so report it busy and leave the mount alone.
        let Some(exclusive) = Arc::get_mut(&mut adapter) else {
            self.libs.insert(old_key, adapter);
            return Err(LibraryError::Conflict(format!(
                "library at {} is being read; try again in a moment",
                old.display()
            )));
        };
        match exclusive.relocate_library_file(new) {
            Ok(()) => {
                self.libs.insert(new_key, adapter);
                Ok(())
            }
            Err(e) => {
                self.libs.insert(old_key, adapter);
                Err(e)
            }
        }
    }

    /// Number of mounted libraries.
    pub fn len(&self) -> usize {
        self.libs.len()
    }

    /// True if no libraries are mounted.
    pub fn is_empty(&self) -> bool {
        self.libs.is_empty()
    }

    /// True if any mounted adapter exposes the given `library_id`.
    pub fn contains(&self, library_id: Uuid) -> bool {
        self.find_key_for_id(library_id).is_some()
    }

    /// True if a file-backed adapter is mounted at `path`.
    pub fn contains_path(&self, path: &Path) -> bool {
        self.libs.contains_key(&MountKey::Path(path.to_path_buf()))
    }

    /// Borrow the first mounted adapter exposing `library_id`, if any.
    pub fn get(&self, library_id: Uuid) -> Option<&dyn LibraryAdapter> {
        let key = self.find_key_for_id(library_id)?;
        self.libs.get(&key).map(|b| b.as_ref())
    }

    /// A shared handle on the first mounted adapter exposing `library_id`,
    /// for work that must leave the UI thread (a table scan in a
    /// `Task`). Reads through it see exactly what [`Self::get`] sees.
    pub fn get_shared(&self, library_id: Uuid) -> Option<Arc<dyn LibraryAdapter>> {
        let key = self.find_key_for_id(library_id)?;
        self.libs.get(&key).map(Arc::clone)
    }

    /// Borrow the file-backed adapter mounted at `path`, if any.
    pub fn get_by_path(&self, path: &Path) -> Option<&dyn LibraryAdapter> {
        self.libs
            .get(&MountKey::Path(path.to_path_buf()))
            .map(|b| b.as_ref())
    }

    /// Iterate over `library_id`s of mounted libraries. Duplicates may
    /// appear when two file-backed adapters share an id.
    pub fn library_ids(&self) -> impl Iterator<Item = Uuid> + '_ {
        self.libs.values().map(|lib| lib.library_id())
    }

    /// Iterate over the file paths of file-backed mounts. Path-less
    /// adapters are skipped.
    pub fn library_paths(&self) -> impl Iterator<Item = &Path> + '_ {
        self.libs.keys().filter_map(|k| match k {
            MountKey::Path(p) => Some(p.as_path()),
            MountKey::Id(_) => None,
        })
    }

    /// Resolve a `PrimitiveRef` to the underlying [`Symbol`].
    ///
    /// `Ok(None)` means the library isn't mounted or the UUID isn't in
    /// it. `Err` means the lookup itself failed and the binding is
    /// **not** known to be wrong — see the module docs.
    pub fn resolve_symbol(&self, r: &PrimitiveRef) -> Result<Option<Symbol>, LibraryError> {
        let Some(lib) = self.find_adapter_for_id(r.library_id) else {
            return Ok(None);
        };
        absence_is_none(lib.get_symbol(r.uuid))
    }

    /// Resolve a `PrimitiveRef` to the underlying [`Footprint`]. Same
    /// `Ok(None)` / `Err` split as [`Self::resolve_symbol`].
    pub fn resolve_footprint(&self, r: &PrimitiveRef) -> Result<Option<Footprint>, LibraryError> {
        let Some(lib) = self.find_adapter_for_id(r.library_id) else {
            return Ok(None);
        };
        absence_is_none(lib.get_footprint(r.uuid))
    }

    /// Resolve a `PrimitiveRef` to the underlying [`SimModel`]. Same
    /// `Ok(None)` / `Err` split as [`Self::resolve_symbol`].
    pub fn resolve_sim(&self, r: &PrimitiveRef) -> Result<Option<SimModel>, LibraryError> {
        let Some(lib) = self.find_adapter_for_id(r.library_id) else {
            return Ok(None);
        };
        absence_is_none(lib.get_sim(r.uuid))
    }

    /// Sweep a stream of references, splitting the ones that genuinely
    /// don't resolve from the ones whose lookup failed. See
    /// [`UnresolvedRefs`] — the two lists carry different meanings and
    /// must not be shown to the user with the same wording.
    pub fn unresolved_refs<'a, I>(&self, refs: I) -> UnresolvedRefs
    where
        I: IntoIterator<Item = &'a PrimitiveRef>,
    {
        let mut out = UnresolvedRefs::default();
        for r in refs {
            let Some(lib) = self.find_adapter_for_id(r.library_id) else {
                out.missing.push(*r);
                continue;
            };
            match probe_every_kind(lib, r) {
                Probe::Resolved => {}
                Probe::Missing => out.missing.push(*r),
                Probe::Undetermined(e) => out.undetermined.push((*r, e.to_string())),
            }
        }
        out
    }

    fn find_adapter_for_id(&self, library_id: Uuid) -> Option<&dyn LibraryAdapter> {
        let key = self.find_key_for_id(library_id)?;
        self.libs.get(&key).map(|b| b.as_ref())
    }

    /// The mount serving `library_id`. Ids are unique by construction;
    /// should two ever share one, the lowest mount key wins every time
    /// instead of whichever the hash map yields first (#1894).
    fn find_key_for_id(&self, library_id: Uuid) -> Option<MountKey> {
        self.libs
            .iter()
            .filter(|(_, lib)| lib.library_id() == library_id)
            .map(|(k, _)| k)
            .min()
            .cloned()
    }
}

fn describe_key(key: &MountKey) -> String {
    match key {
        MountKey::Path(p) => p.display().to_string(),
        MountKey::Id(id) => format!("library {id}"),
    }
}

/// Outcome of a [`LibrarySet::unresolved_refs`] sweep.
///
/// The split exists because the two lists need different wording in
/// the UI: `missing` is a binding the user should fix, `undetermined`
/// is a library the app failed to read and says nothing about whether
/// the binding is right.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct UnresolvedRefs {
    /// References that genuinely do not resolve — the library isn't
    /// mounted, or no primitive of any kind carries the UUID.
    pub missing: Vec<PrimitiveRef>,
    /// References whose resolution could not be decided, paired with
    /// the rendered error. Every kind failed and at least one failed
    /// for a reason other than absence.
    pub undetermined: Vec<(PrimitiveRef, String)>,
}

impl UnresolvedRefs {
    /// True when the sweep found neither a missing nor an undecidable
    /// reference.
    pub fn is_empty(&self) -> bool {
        self.missing.is_empty() && self.undetermined.is_empty()
    }
}

/// What a single reference turned out to be after trying every
/// primitive kind.
enum Probe {
    /// Some kind returned the primitive.
    Resolved,
    /// Every kind reported the UUID absent.
    Missing,
    /// No kind returned the primitive and at least one failed for a
    /// reason other than absence, so absence was never established.
    Undetermined(LibraryError),
}

/// Every primitive-kind lookup, in the order the resolver tries them.
/// Function pointers rather than an array of results so a hit on the
/// first kind doesn't pay for reading the other two off disk.
type KindProbe = fn(&dyn LibraryAdapter, Uuid) -> Result<(), LibraryError>;
const KIND_PROBES: [KindProbe; 3] = [
    |lib, uuid| lib.get_symbol(uuid).map(|_| ()),
    |lib, uuid| lib.get_footprint(uuid).map(|_| ()),
    |lib, uuid| lib.get_sim(uuid).map(|_| ()),
];

/// Try `r` as a symbol, then a footprint, then a sim model, stopping at
/// the first hit. Only an all-`NotFound` sweep proves the reference is
/// missing.
fn probe_every_kind(lib: &dyn LibraryAdapter, r: &PrimitiveRef) -> Probe {
    let mut blocking: Option<LibraryError> = None;
    for probe in KIND_PROBES {
        match probe(lib, r.uuid) {
            Ok(()) => return Probe::Resolved,
            Err(LibraryError::NotFound(_)) => {}
            Err(e) => {
                if blocking.is_none() {
                    blocking = Some(e);
                }
            }
        }
    }
    match blocking {
        Some(e) => Probe::Undetermined(e),
        None => Probe::Missing,
    }
}

/// Fold an adapter lookup into the resolver's `Ok(None)` = "not there"
/// / `Err` = "could not tell" split.
/// Flatten an adapter lookup into the resolver contract: `NotFound` is
/// `Ok(None)` (the primitive is not there), any other error stays an `Err`
/// (the lookup itself failed). For callers that read through a shared
/// adapter handle instead of the set, e.g. on a worker thread.
pub fn absence_is_none<T>(got: Result<T, LibraryError>) -> Result<Option<T>, LibraryError> {
    match got {
        Ok(v) => Ok(Some(v)),
        Err(LibraryError::NotFound(_)) => Ok(None),
        Err(e) => Err(e),
    }
}

impl std::fmt::Debug for LibrarySet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LibrarySet")
            .field("mounted", &self.libs.len())
            .field("keys", &self.libs.keys().collect::<Vec<&MountKey>>())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::LibraryError;
    use crate::manifest::{LibraryMeta, LibraryMode, Manifest, UsersConfig, WorkflowConfig};

    /// Minimal in-memory adapter used to exercise resolver mechanics
    /// without requiring the `local-git` feature. Optionally carries a
    /// fake `library_file_path()` so tests can drive the path-keyed
    /// branch of `MountKey`.
    struct FakeAdapter {
        manifest: Manifest,
        symbols: HashMap<Uuid, Symbol>,
        path: Option<PathBuf>,
        /// When set, every primitive lookup fails with this error
        /// instead of reporting the UUID absent. Models a corrupt
        /// `.snxsym` on disk or a library server returning 500.
        read_failure: Option<String>,
    }

    impl FakeAdapter {
        fn new(library_id: Uuid) -> Self {
            Self {
                manifest: Manifest {
                    library: LibraryMeta {
                        name: "fake".into(),
                        library_id,
                        description: None,
                    },
                    mode: LibraryMode::default(),
                    workflow: WorkflowConfig::default(),
                    users: UsersConfig::default(),
                    tables: Vec::new(),
                },
                symbols: HashMap::new(),
                path: None,
                read_failure: None,
            }
        }

        fn with_symbol(mut self, sym: Symbol) -> Self {
            self.symbols.insert(sym.uuid, sym);
            self
        }

        fn with_path(mut self, path: impl Into<PathBuf>) -> Self {
            self.path = Some(path.into());
            self
        }

        fn with_read_failure(mut self, reason: &str) -> Self {
            self.read_failure = Some(reason.to_string());
            self
        }

        fn failure(&self) -> Option<LibraryError> {
            self.read_failure
                .as_ref()
                .map(|r| LibraryError::Backend(r.clone()))
        }
    }

    impl LibraryAdapter for FakeAdapter {
        fn manifest(&self) -> &Manifest {
            &self.manifest
        }

        fn library_file_path(&self) -> Option<&Path> {
            self.path.as_deref()
        }

        fn get_symbol(&self, uuid: Uuid) -> Result<Symbol, LibraryError> {
            if let Some(e) = self.failure() {
                return Err(e);
            }
            self.symbols
                .get(&uuid)
                .cloned()
                .ok_or_else(|| LibraryError::NotFound(format!("symbol {uuid}")))
        }

        // The trait defaults report `Backend("not implemented")`, which
        // the resolver correctly reads as "could not tell". Real
        // adapters implement all three, so model that here rather than
        // making every test ref look undecidable.
        fn get_footprint(&self, uuid: Uuid) -> Result<Footprint, LibraryError> {
            match self.failure() {
                Some(e) => Err(e),
                None => Err(LibraryError::NotFound(format!("footprint {uuid}"))),
            }
        }

        fn get_sim(&self, uuid: Uuid) -> Result<SimModel, LibraryError> {
            match self.failure() {
                Some(e) => Err(e),
                None => Err(LibraryError::NotFound(format!("sim {uuid}"))),
            }
        }
    }

    fn fixture_symbol(name: &str) -> Symbol {
        Symbol::empty(name)
    }

    #[test]
    fn empty_set_resolves_nothing() {
        let set = LibrarySet::new();
        let r = PrimitiveRef::new(Uuid::now_v7(), Uuid::now_v7());
        assert!(
            set.resolve_symbol(&r)
                .expect("no mount is not a failure")
                .is_none()
        );
        assert!(
            set.resolve_footprint(&r)
                .expect("no mount is not a failure")
                .is_none()
        );
        assert!(
            set.resolve_sim(&r)
                .expect("no mount is not a failure")
                .is_none()
        );
        assert!(set.is_empty());
    }

    #[test]
    fn mount_and_resolve_symbol() {
        let lib_id = Uuid::now_v7();
        let sym = fixture_symbol("OPAMP-DUAL-8");
        let sym_uuid = sym.uuid;
        let adapter = FakeAdapter::new(lib_id).with_symbol(sym);
        let mut set = LibrarySet::new();
        set.mount(Box::new(adapter)).unwrap();

        let r = PrimitiveRef::new(lib_id, sym_uuid);
        let got = set
            .resolve_symbol(&r)
            .expect("lookup succeeds")
            .expect("symbol resolves");
        assert_eq!(got.uuid, sym_uuid);
        assert_eq!(got.name, "OPAMP-DUAL-8");
    }

    #[test]
    fn unresolved_when_library_missing() {
        let set = LibrarySet::new();
        let r = PrimitiveRef::new(Uuid::now_v7(), Uuid::now_v7());
        assert!(set.resolve_symbol(&r).expect("lookup succeeds").is_none());
    }

    #[test]
    fn unresolved_when_uuid_missing_in_mounted_lib() {
        let lib_id = Uuid::now_v7();
        let mut set = LibrarySet::new();
        set.mount(Box::new(FakeAdapter::new(lib_id))).unwrap();
        let r = PrimitiveRef::new(lib_id, Uuid::now_v7());
        assert!(set.resolve_symbol(&r).expect("lookup succeeds").is_none());
    }

    /// A read failure is not evidence that the binding is wrong. The
    /// resolvers must hand the caller the error instead of the same
    /// `None` a genuinely absent UUID produces.
    #[test]
    fn read_failure_is_reported_not_reported_as_a_missing_uuid() {
        let lib_id = Uuid::now_v7();
        let sym = fixture_symbol("U1");
        let sym_uuid = sym.uuid;
        let adapter = FakeAdapter::new(lib_id)
            .with_symbol(sym)
            .with_read_failure("read symbol file U1.snxsym: unexpected end of input");
        let mut set = LibrarySet::new();
        set.mount(Box::new(adapter)).unwrap();

        let r = PrimitiveRef::new(lib_id, sym_uuid);
        let err = set
            .resolve_symbol(&r)
            .expect_err("a corrupt on-disk symbol must not read as an unbound UUID");
        assert!(
            err.to_string().contains("unexpected end of input"),
            "error lost its detail: {err}"
        );
        assert!(set.resolve_footprint(&r).is_err());
        assert!(set.resolve_sim(&r).is_err());
    }

    #[test]
    fn unresolved_refs_filters_to_only_missing() {
        let lib_id = Uuid::now_v7();
        let known = fixture_symbol("R");
        let known_uuid = known.uuid;
        let adapter = FakeAdapter::new(lib_id).with_symbol(known);
        let mut set = LibrarySet::new();
        set.mount(Box::new(adapter)).unwrap();

        let resolves = PrimitiveRef::new(lib_id, known_uuid);
        let stale_lib = PrimitiveRef::new(Uuid::now_v7(), Uuid::now_v7());
        let stale_uuid = PrimitiveRef::new(lib_id, Uuid::now_v7());

        let unresolved = set.unresolved_refs([&resolves, &stale_lib, &stale_uuid]);
        assert_eq!(unresolved.missing.len(), 2);
        assert!(unresolved.missing.contains(&stale_lib));
        assert!(unresolved.missing.contains(&stale_uuid));
        assert!(!unresolved.missing.contains(&resolves));
        assert!(
            unresolved.undetermined.is_empty(),
            "nothing failed to read: {:?}",
            unresolved.undetermined
        );
    }

    /// The sweep must not file an unreadable library under "missing" —
    /// that is the register's wrong-binding report all over again, just
    /// in bulk.
    #[test]
    fn unresolved_refs_separates_unreadable_from_missing() {
        let broken_id = Uuid::now_v7();
        let good_id = Uuid::now_v7();
        let mut set = LibrarySet::new();
        set.mount(Box::new(
            FakeAdapter::new(broken_id)
                .with_path("/tmp/broken/lib.snxlib")
                .with_read_failure("library server returned 500"),
        ))
        .unwrap();
        set.mount(Box::new(
            FakeAdapter::new(good_id).with_path("/tmp/good/lib.snxlib"),
        ))
        .unwrap();

        let unreadable = PrimitiveRef::new(broken_id, Uuid::now_v7());
        let genuinely_missing = PrimitiveRef::new(good_id, Uuid::now_v7());

        let out = set.unresolved_refs([&unreadable, &genuinely_missing]);
        assert_eq!(out.missing, vec![genuinely_missing]);
        assert_eq!(out.undetermined.len(), 1);
        assert_eq!(out.undetermined[0].0, unreadable);
        assert!(
            out.undetermined[0]
                .1
                .contains("library server returned 500"),
            "error lost its detail: {}",
            out.undetermined[0].1
        );
        assert!(!out.is_empty());
    }

    #[test]
    fn unmount_returns_adapter_and_drops_resolution() {
        let lib_id = Uuid::now_v7();
        let sym = fixture_symbol("X");
        let sym_uuid = sym.uuid;
        let adapter = FakeAdapter::new(lib_id).with_symbol(sym);
        let mut set = LibrarySet::new();
        set.mount(Box::new(adapter)).unwrap();

        assert!(set.contains(lib_id));
        let returned = set.unmount(lib_id);
        assert!(returned.is_some());
        assert!(!set.contains(lib_id));

        let r = PrimitiveRef::new(lib_id, sym_uuid);
        assert!(set.resolve_symbol(&r).expect("lookup succeeds").is_none());
    }

    #[test]
    fn library_ids_yields_each_mount() {
        let a = Uuid::now_v7();
        let b = Uuid::now_v7();
        let mut set = LibrarySet::new();
        set.mount(Box::new(FakeAdapter::new(a))).unwrap();
        set.mount(Box::new(FakeAdapter::new(b))).unwrap();
        let ids: Vec<Uuid> = set.library_ids().collect();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&a));
        assert!(ids.contains(&b));
    }

    /// Path-less adapters (DB/in-memory) keep the legacy duplicate-id
    /// safety check — two of those with the same id is a config bug.
    #[test]
    fn mount_rejects_duplicate_library_id_for_pathless_adapters() {
        let lib_id = Uuid::now_v7();
        let mut set = LibrarySet::new();
        set.mount(Box::new(FakeAdapter::new(lib_id))).unwrap();
        let dup = set.mount(Box::new(FakeAdapter::new(lib_id)));
        assert!(matches!(dup, Err(LibraryError::Conflict(_))));
        assert_eq!(set.len(), 1);
    }

    /// #1894: two file-backed adapters sharing an id at different paths
    /// (a copied library folder) are refused, naming both paths; the
    /// first stays mounted and every id lookup keeps resolving to it.
    #[test]
    fn mount_refuses_a_second_library_with_the_same_id() {
        let lib_id = Uuid::now_v7();
        let mut set = LibrarySet::new();
        set.mount(Box::new(
            FakeAdapter::new(lib_id).with_path("/tmp/copy_a/lib.snxlib"),
        ))
        .unwrap();
        let dup = set.mount(Box::new(
            FakeAdapter::new(lib_id).with_path("/tmp/copy_b/lib.snxlib"),
        ));
        let Err(LibraryError::Conflict(message)) = dup else {
            panic!("a second mount of the same library id must be refused");
        };
        assert!(
            message.contains("copy_a") && message.contains("copy_b"),
            "{message}"
        );
        assert_eq!(set.len(), 1);
        assert_eq!(
            set.find_key_for_id(lib_id),
            Some(MountKey::Path(PathBuf::from("/tmp/copy_a/lib.snxlib")))
        );
    }

    /// The read-only mount path goes through the same check.
    #[test]
    fn a_read_only_mount_with_a_mounted_id_is_refused_too() {
        let lib_id = Uuid::now_v7();
        let mut set = LibrarySet::new();
        set.mount(Box::new(
            FakeAdapter::new(lib_id).with_path("/tmp/stock/Stock.snxlib"),
        ))
        .unwrap();
        let dup = set.mount_read_only(Box::new(
            FakeAdapter::new(lib_id).with_path("/tmp/copy/Stock.snxlib"),
        ));
        assert!(matches!(dup, Err(LibraryError::Conflict(_))));
        assert_eq!(set.len(), 1);
    }

    /// Two file-backed adapters at the *same* path collide — that's
    /// always a bug (mount the same .snxlib twice).
    #[test]
    fn mount_rejects_duplicate_path() {
        let mut set = LibrarySet::new();
        set.mount(Box::new(
            FakeAdapter::new(Uuid::now_v7()).with_path("/tmp/x/lib.snxlib"),
        ))
        .unwrap();
        let dup = set.mount(Box::new(
            FakeAdapter::new(Uuid::now_v7()).with_path("/tmp/x/lib.snxlib"),
        ));
        assert!(matches!(dup, Err(LibraryError::Conflict(_))));
        assert_eq!(set.len(), 1);
    }

    /// `unmount_by_path` removes exactly the file-backed mount at that
    /// path, and frees its `library_id` so a copy can then be mounted.
    #[test]
    fn unmount_by_path_removes_specific_mount() {
        let (id_a, id_b) = (Uuid::now_v7(), Uuid::now_v7());
        let path_a = PathBuf::from("/tmp/a/lib.snxlib");
        let path_b = PathBuf::from("/tmp/b/lib.snxlib");
        let mut set = LibrarySet::new();
        set.mount(Box::new(FakeAdapter::new(id_a).with_path(path_a.clone())))
            .unwrap();
        set.mount(Box::new(FakeAdapter::new(id_b).with_path(path_b.clone())))
            .unwrap();
        assert_eq!(set.len(), 2);

        let removed = set.unmount_by_path(&path_a);
        assert!(removed.is_some());
        assert_eq!(set.len(), 1);
        assert!(!set.contains_path(&path_a));
        assert!(set.contains_path(&path_b));
        assert!(!set.contains(id_a));
        // With `a` gone, a copy carrying its id mounts.
        set.mount(Box::new(
            FakeAdapter::new(id_a).with_path("/tmp/a_copy/lib.snxlib"),
        ))
        .expect("the id is free again");
    }

    /// `library_paths` lists file-backed mounts only; path-less
    /// adapters are filtered out.
    #[test]
    fn library_paths_skips_pathless_mounts() {
        let mut set = LibrarySet::new();
        set.mount(Box::new(FakeAdapter::new(Uuid::now_v7())))
            .unwrap();
        set.mount(Box::new(
            FakeAdapter::new(Uuid::now_v7()).with_path("/tmp/p/lib.snxlib"),
        ))
        .unwrap();
        let paths: Vec<&Path> = set.library_paths().collect();
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0], Path::new("/tmp/p/lib.snxlib"));
    }

    #[test]
    fn remount_replaces_previous_adapter_and_returns_old() {
        let lib_id = Uuid::now_v7();
        let first = fixture_symbol("First");
        let first_uuid = first.uuid;
        let second = fixture_symbol("Second");
        let second_uuid = second.uuid;

        let mut set = LibrarySet::new();
        set.mount(Box::new(
            FakeAdapter::new(lib_id).with_symbol(first.clone()),
        ))
        .unwrap();
        // remount under same mount key replaces the adapter and
        // returns the previous one so the caller can decide to drop it
        // or hand it elsewhere.
        let prev = set
            .remount(Box::new(FakeAdapter::new(lib_id).with_symbol(second)))
            .expect("same key, same id");
        assert!(prev.is_some());

        // First UUID no longer resolves; second does.
        assert!(
            set.resolve_symbol(&PrimitiveRef::new(lib_id, first_uuid))
                .expect("lookup succeeds")
                .is_none()
        );
        assert!(
            set.resolve_symbol(&PrimitiveRef::new(lib_id, second_uuid))
                .expect("lookup succeeds")
                .is_some()
        );
        // Still only one mount because the key was the same.
        assert_eq!(set.len(), 1);

        // Suppress unused warning while keeping the symbol around for clarity.
        let _ = first.name;
    }

    /// #1894: replacing the mount at one path with a library that carries
    /// another mount's id is refused and changes nothing.
    #[test]
    fn remount_refuses_an_id_another_mount_serves() {
        let (id_a, id_b) = (Uuid::now_v7(), Uuid::now_v7());
        let mut set = LibrarySet::new();
        set.mount(Box::new(
            FakeAdapter::new(id_a).with_path("/tmp/a/a.snxlib"),
        ))
        .unwrap();
        set.mount(Box::new(
            FakeAdapter::new(id_b).with_path("/tmp/b/b.snxlib"),
        ))
        .unwrap();
        let clash = set.remount(Box::new(
            FakeAdapter::new(id_a).with_path("/tmp/b/b.snxlib"),
        ));
        let Err(LibraryError::Conflict(message)) = clash else {
            panic!("remount onto another mount's id must be refused");
        };
        assert!(message.contains("/tmp/a/a.snxlib"), "{message}");
        assert_eq!(set.len(), 2);
        assert_eq!(
            set.get(id_b).and_then(|l| l.library_file_path()),
            Some(Path::new("/tmp/b/b.snxlib"))
        );
        assert_eq!(
            set.get(id_a).and_then(|l| l.library_file_path()),
            Some(Path::new("/tmp/a/a.snxlib"))
        );
    }

    /// #1894: even with several mounts under one id, a lookup answers the
    /// same mount every time, not whichever the hash map yields first.
    #[test]
    fn a_lookup_under_a_shared_id_answers_the_same_mount_every_time() {
        let id = Uuid::now_v7();
        for _ in 0..32 {
            let mut set = LibrarySet::new();
            for path in ["/tmp/z/z.snxlib", "/tmp/m/m.snxlib", "/tmp/a/a.snxlib"] {
                let lib: Arc<dyn LibraryAdapter> = Arc::new(FakeAdapter::new(id).with_path(path));
                set.libs.insert(MountKey::Path(PathBuf::from(path)), lib);
            }
            assert_eq!(
                set.get(id).and_then(|l| l.library_file_path()),
                Some(Path::new("/tmp/a/a.snxlib"))
            );
            assert_eq!(
                set.find_key_for_id(id),
                Some(MountKey::Path(PathBuf::from("/tmp/a/a.snxlib")))
            );
        }
    }
}
