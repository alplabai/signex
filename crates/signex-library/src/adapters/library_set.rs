//! `LibrarySet` — a tiny resolver that composes any number of
//! [`LibraryAdapter`] trait objects into a single lookup surface for
//! cross-library [`PrimitiveRef`] resolution.
//!
//! Per `v0.9-snxlib-as-file-plan.md` §2 Stage B, the primary mount
//! key is the `.snxlib` *file path* (not the library_id). The
//! Components Panel (§5) presents the same library_id across
//! Project / Installed / Global lists — a library_id can therefore
//! legitimately appear under multiple file paths in a single
//! `LibrarySet`, so the duplicate-id check that used to gate
//! mounting has moved out to the panel layer (where dedup happens
//! at presentation time).
//!
//! ## Resolution semantics
//!
//! - A reference whose `library_id` isn't mounted resolves to `None` —
//!   the editor surfaces this as "unresolved primitive — open dependent
//!   library?".
//! - A reference whose `library_id` IS mounted but whose primitive UUID
//!   isn't in any of the matching adapters ALSO resolves to `None`.
//! - When two adapters share a `library_id` (the "user copy-pasted a
//!   library" case), [`Self::resolve_symbol`] / `_footprint` / `_sim`
//!   pick the first match. UI dedup at the Components Panel level is
//!   the right place to warn the user about the collision.
//! - Adapters without an on-disk path (e.g. `DatabaseAdapter`) fall
//!   back to keying by `library_id`; the duplicate-id error still
//!   guards those.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::adapter::{LibraryAdapter, LibraryError};
use crate::primitive::{Footprint, PrimitiveRef, SimModel, Symbol};

/// Mount key for an adapter inside a [`LibrarySet`].
///
/// File-backed adapters (`LocalGitAdapter`) key by their absolute
/// `.snxlib` file path so the same `library_id` mounted at two
/// distinct on-disk locations is allowed. Path-less adapters
/// (`DatabaseAdapter`) fall back to keying by `library_id`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
    libs: HashMap<MountKey, Box<dyn LibraryAdapter>>,
}

impl LibrarySet {
    /// Construct an empty `LibrarySet`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Mount a library.
    ///
    /// Returns `Conflict` when the same mount key is already in use:
    /// duplicate `.snxlib` file path for file-backed adapters, or
    /// duplicate `library_id` for path-less adapters. Two file-backed
    /// adapters with the same `library_id` at *different* paths are
    /// allowed — the Components Panel dedups by id at presentation
    /// time.
    ///
    /// Use [`Self::remount`] to replace an existing mount in one step.
    pub fn mount(&mut self, lib: Box<dyn LibraryAdapter>) -> Result<(), LibraryError> {
        let key = MountKey::for_adapter(lib.as_ref());
        if self.libs.contains_key(&key) {
            return Err(LibraryError::Conflict(match &key {
                MountKey::Path(p) => format!(
                    "library at {} is already mounted",
                    p.display()
                ),
                MountKey::Id(id) => format!(
                    "library_id {id} is already mounted — duplicate library_id on \
                     a path-less adapter (DB or in-memory) suggests a misconfig"
                ),
            }));
        }
        self.libs.insert(key, lib);
        Ok(())
    }

    /// Replace whatever adapter is mounted at `lib`'s mount key with
    /// `lib`. Drops the previous adapter and returns it (or `None` if
    /// no previous mount existed).
    pub fn remount(&mut self, lib: Box<dyn LibraryAdapter>) -> Option<Box<dyn LibraryAdapter>> {
        let key = MountKey::for_adapter(lib.as_ref());
        self.libs.insert(key, lib)
    }

    /// Unmount the first adapter matching `library_id` and return it
    /// (or `None` if no adapter is registered under that id).
    ///
    /// When two file-backed adapters share an id, the "first match"
    /// is unspecified — callers needing a specific mount should use
    /// [`Self::unmount_by_path`].
    pub fn unmount(&mut self, library_id: Uuid) -> Option<Box<dyn LibraryAdapter>> {
        let key = self.find_key_for_id(library_id)?;
        self.libs.remove(&key)
    }

    /// Unmount the file-backed adapter at `path`, returning it.
    pub fn unmount_by_path(&mut self, path: &Path) -> Option<Box<dyn LibraryAdapter>> {
        self.libs.remove(&MountKey::Path(path.to_path_buf()))
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

    /// Resolve a `PrimitiveRef` to the underlying [`Symbol`], if both the
    /// library and the primitive UUID exist.
    pub fn resolve_symbol(&self, r: &PrimitiveRef) -> Option<Symbol> {
        let lib = self.find_adapter_for_id(r.library_id)?;
        lib.get_symbol(r.uuid).ok()
    }

    /// Resolve a `PrimitiveRef` to the underlying [`Footprint`].
    pub fn resolve_footprint(&self, r: &PrimitiveRef) -> Option<Footprint> {
        let lib = self.find_adapter_for_id(r.library_id)?;
        lib.get_footprint(r.uuid).ok()
    }

    /// Resolve a `PrimitiveRef` to the underlying [`SimModel`].
    pub fn resolve_sim(&self, r: &PrimitiveRef) -> Option<SimModel> {
        let lib = self.find_adapter_for_id(r.library_id)?;
        lib.get_sim(r.uuid).ok()
    }

    /// Filter a stream of references down to those that don't currently
    /// resolve, regardless of primitive kind.
    pub fn unresolved_refs<'a, I>(&self, refs: I) -> Vec<PrimitiveRef>
    where
        I: IntoIterator<Item = &'a PrimitiveRef>,
    {
        refs.into_iter()
            .filter(|r| {
                let Some(lib) = self.find_adapter_for_id(r.library_id) else {
                    return true;
                };
                lib.get_symbol(r.uuid).is_err()
                    && lib.get_footprint(r.uuid).is_err()
                    && lib.get_sim(r.uuid).is_err()
            })
            .copied()
            .collect()
    }

    fn find_adapter_for_id(&self, library_id: Uuid) -> Option<&dyn LibraryAdapter> {
        self.libs
            .values()
            .find(|lib| lib.library_id() == library_id)
            .map(|b| b.as_ref())
    }

    fn find_key_for_id(&self, library_id: Uuid) -> Option<MountKey> {
        self.libs
            .iter()
            .find(|(_, lib)| lib.library_id() == library_id)
            .map(|(k, _)| k.clone())
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
    }

    impl LibraryAdapter for FakeAdapter {
        fn manifest(&self) -> &Manifest {
            &self.manifest
        }

        fn library_file_path(&self) -> Option<&Path> {
            self.path.as_deref()
        }

        fn get_symbol(&self, uuid: Uuid) -> Result<Symbol, LibraryError> {
            self.symbols
                .get(&uuid)
                .cloned()
                .ok_or_else(|| LibraryError::NotFound(format!("symbol {uuid}")))
        }
    }

    fn fixture_symbol(name: &str) -> Symbol {
        Symbol::empty(name)
    }

    #[test]
    fn empty_set_resolves_nothing() {
        let set = LibrarySet::new();
        let r = PrimitiveRef::new(Uuid::now_v7(), Uuid::now_v7());
        assert!(set.resolve_symbol(&r).is_none());
        assert!(set.resolve_footprint(&r).is_none());
        assert!(set.resolve_sim(&r).is_none());
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
        let got = set.resolve_symbol(&r).expect("symbol resolves");
        assert_eq!(got.uuid, sym_uuid);
        assert_eq!(got.name, "OPAMP-DUAL-8");
    }

    #[test]
    fn unresolved_when_library_missing() {
        let set = LibrarySet::new();
        let r = PrimitiveRef::new(Uuid::now_v7(), Uuid::now_v7());
        assert!(set.resolve_symbol(&r).is_none());
    }

    #[test]
    fn unresolved_when_uuid_missing_in_mounted_lib() {
        let lib_id = Uuid::now_v7();
        let mut set = LibrarySet::new();
        set.mount(Box::new(FakeAdapter::new(lib_id))).unwrap();
        let r = PrimitiveRef::new(lib_id, Uuid::now_v7());
        assert!(set.resolve_symbol(&r).is_none());
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
        assert_eq!(unresolved.len(), 2);
        assert!(unresolved.contains(&stale_lib));
        assert!(unresolved.contains(&stale_uuid));
        assert!(!unresolved.contains(&resolves));
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
        assert!(set.resolve_symbol(&r).is_none());
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

    /// Two file-backed adapters sharing an id but at different paths
    /// are allowed under the new model — the Components Panel dedups
    /// at display time.
    #[test]
    fn mount_allows_duplicate_library_id_at_different_paths() {
        let lib_id = Uuid::now_v7();
        let mut set = LibrarySet::new();
        set.mount(Box::new(
            FakeAdapter::new(lib_id).with_path("/tmp/copy_a/lib.snxlib"),
        ))
        .unwrap();
        set.mount(Box::new(
            FakeAdapter::new(lib_id).with_path("/tmp/copy_b/lib.snxlib"),
        ))
        .expect("duplicate library_id at a different path is allowed");
        assert_eq!(set.len(), 2);
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

    /// `unmount_by_path` removes a specific file-backed mount when
    /// callers can't disambiguate by `library_id` alone.
    #[test]
    fn unmount_by_path_removes_specific_mount() {
        let lib_id = Uuid::now_v7();
        let path_a = PathBuf::from("/tmp/a/lib.snxlib");
        let path_b = PathBuf::from("/tmp/b/lib.snxlib");
        let mut set = LibrarySet::new();
        set.mount(Box::new(FakeAdapter::new(lib_id).with_path(path_a.clone())))
            .unwrap();
        set.mount(Box::new(FakeAdapter::new(lib_id).with_path(path_b.clone())))
            .unwrap();
        assert_eq!(set.len(), 2);

        let removed = set.unmount_by_path(&path_a);
        assert!(removed.is_some());
        assert_eq!(set.len(), 1);
        assert!(!set.contains_path(&path_a));
        assert!(set.contains_path(&path_b));
        // Library_id still resolves through the surviving mount.
        assert!(set.contains(lib_id));
    }

    /// `library_paths` lists file-backed mounts only; path-less
    /// adapters are filtered out.
    #[test]
    fn library_paths_skips_pathless_mounts() {
        let mut set = LibrarySet::new();
        set.mount(Box::new(FakeAdapter::new(Uuid::now_v7()))).unwrap();
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
        let prev = set.remount(Box::new(FakeAdapter::new(lib_id).with_symbol(second)));
        assert!(prev.is_some());

        // First UUID no longer resolves; second does.
        assert!(
            set.resolve_symbol(&PrimitiveRef::new(lib_id, first_uuid))
                .is_none()
        );
        assert!(
            set.resolve_symbol(&PrimitiveRef::new(lib_id, second_uuid))
                .is_some()
        );
        // Still only one mount because the key was the same.
        assert_eq!(set.len(), 1);

        // Suppress unused warning while keeping the symbol around for clarity.
        let _ = first.name;
    }
}
