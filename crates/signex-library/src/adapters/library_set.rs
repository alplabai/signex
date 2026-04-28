//! `LibrarySet` — a tiny resolver that composes any number of
//! [`LibraryAdapter`] trait objects into a single lookup surface for
//! cross-library [`PrimitiveRef`] resolution.
//!
//! Per `v0.9-library-refactor-plan.md` §2.6 / §8 step C4, primitives are
//! addressed by `(library_id, uuid)` tuples. When the editor or renderer
//! holds a `Revision`, it carries `symbol_ref / footprint_ref / sim_ref`
//! values whose `library_id` may point at *another* library entirely (the
//! Altium Database-Library shape). `LibrarySet::mount` registers each open
//! adapter under its own `library_id`, and `resolve_*` looks up the right
//! adapter and asks it for the primitive.
//!
//! ## Resolution semantics
//!
//! - A reference whose `library_id` isn't mounted resolves to `None` —
//!   the editor surfaces this as "unresolved primitive — open dependent
//!   library?" (plan §2.6).
//! - A reference whose `library_id` IS mounted but whose primitive UUID
//!   isn't in that adapter ALSO resolves to `None` — same UI surfacing.
//! - The set holds owned `Box<dyn LibraryAdapter>` values; mounting moves
//!   the adapter into the set. `unmount(library_id)` returns it back so the
//!   UI can hand it off without keeping a phantom mount alive.

use std::collections::HashMap;

use uuid::Uuid;

use crate::adapter::{LibraryAdapter, LibraryError};
use crate::primitive::{Footprint, PrimitiveRef, SimModel, Symbol};

/// A bag of mounted libraries, keyed by `library_id`.
///
/// `Default` constructs an empty set; call [`Self::mount`] to add adapters.
#[derive(Default)]
pub struct LibrarySet {
    libs: HashMap<Uuid, Box<dyn LibraryAdapter>>,
}

impl LibrarySet {
    /// Construct an empty `LibrarySet`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Mount a library — the adapter's [`LibraryAdapter::library_id`] is
    /// used as the key.
    ///
    /// Returns `LibraryError::Conflict` when a library with the same
    /// `library_id` is already mounted. Two `.snxlib/` directories with
    /// duplicate `library_id`s (e.g. the user copy-pasted a library to
    /// start a new one without regenerating the manifest UUID) would
    /// otherwise have the second mount silently shadow the first, and
    /// every cross-library `PrimitiveRef` for that id would resolve to
    /// the wrong file. The caller surfaces the conflict to the UI so
    /// the user can rename one of the libraries.
    ///
    /// To replace an existing mount intentionally, use
    /// [`Self::remount`] which drops the old adapter and installs the
    /// new one in one step.
    pub fn mount(&mut self, lib: Box<dyn LibraryAdapter>) -> Result<(), LibraryError> {
        let id = lib.library_id();
        if self.libs.contains_key(&id) {
            return Err(LibraryError::Conflict(format!(
                "library_id {id} is already mounted — duplicate library_id across two .snxlib/ \
                 directories. Open only one, or regenerate the manifest UUID on the duplicate."
            )));
        }
        self.libs.insert(id, lib);
        Ok(())
    }

    /// Replace whatever adapter is mounted at `lib.library_id` with
    /// `lib`. Drops the previous adapter and returns it (or `None` if
    /// no previous mount existed). Use [`Self::mount`] when you want
    /// the duplicate-id case to be an explicit error.
    pub fn remount(&mut self, lib: Box<dyn LibraryAdapter>) -> Option<Box<dyn LibraryAdapter>> {
        let id = lib.library_id();
        self.libs.insert(id, lib)
    }

    /// Unmount and return a previously-mounted library, or `None` if no
    /// adapter is registered under that `library_id`.
    pub fn unmount(&mut self, library_id: Uuid) -> Option<Box<dyn LibraryAdapter>> {
        self.libs.remove(&library_id)
    }

    /// Number of mounted libraries.
    pub fn len(&self) -> usize {
        self.libs.len()
    }

    /// True if no libraries are mounted.
    pub fn is_empty(&self) -> bool {
        self.libs.is_empty()
    }

    /// True if a library with this id is mounted.
    pub fn contains(&self, library_id: Uuid) -> bool {
        self.libs.contains_key(&library_id)
    }

    /// Borrow the mounted library, if any.
    pub fn get(&self, library_id: Uuid) -> Option<&dyn LibraryAdapter> {
        self.libs.get(&library_id).map(|b| b.as_ref())
    }

    /// Iterate over `library_id`s of mounted libraries.
    pub fn library_ids(&self) -> impl Iterator<Item = Uuid> + '_ {
        self.libs.keys().copied()
    }

    /// Resolve a `PrimitiveRef` to the underlying [`Symbol`], if both the
    /// library and the primitive UUID exist.
    pub fn resolve_symbol(&self, r: &PrimitiveRef) -> Option<Symbol> {
        self.libs.get(&r.library_id)?.get_symbol(r.uuid).ok()
    }

    /// Resolve a `PrimitiveRef` to the underlying [`Footprint`].
    pub fn resolve_footprint(&self, r: &PrimitiveRef) -> Option<Footprint> {
        self.libs.get(&r.library_id)?.get_footprint(r.uuid).ok()
    }

    /// Resolve a `PrimitiveRef` to the underlying [`SimModel`].
    pub fn resolve_sim(&self, r: &PrimitiveRef) -> Option<SimModel> {
        self.libs.get(&r.library_id)?.get_sim(r.uuid).ok()
    }

    /// Filter a stream of references down to those that don't currently
    /// resolve, regardless of primitive kind. The caller decides which
    /// primitive flavour each reference is — the resolver tries all three
    /// (symbol, footprint, sim) and keeps refs that miss in every flavour.
    ///
    /// This is the canonical helper for the "unresolved primitives" panel
    /// the editor surfaces when a dependent library is closed.
    pub fn unresolved_refs<'a, I>(&self, refs: I) -> Vec<PrimitiveRef>
    where
        I: IntoIterator<Item = &'a PrimitiveRef>,
    {
        refs.into_iter()
            .filter(|r| {
                // Library is missing entirely → unresolved.
                let Some(lib) = self.libs.get(&r.library_id) else {
                    return true;
                };
                // Library is mounted; the ref is unresolved iff it doesn't
                // exist as any of the three primitive kinds. Adapters use
                // separate stores, so we have to ask each.
                lib.get_symbol(r.uuid).is_err()
                    && lib.get_footprint(r.uuid).is_err()
                    && lib.get_sim(r.uuid).is_err()
            })
            .copied()
            .collect()
    }
}

impl std::fmt::Debug for LibrarySet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LibrarySet")
            .field("mounted", &self.libs.len())
            .field("ids", &self.libs.keys().copied().collect::<Vec<Uuid>>())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::LibraryError;
    use crate::manifest::{LibraryMeta, LibraryMode, Manifest, UsersConfig, WorkflowConfig};
    use crate::primitive::{PinElectricalType, PinOrientation, SymbolPin};

    /// Minimal in-memory adapter used to exercise resolver mechanics
    /// without requiring the `local-git` feature. Holds a single symbol so
    /// we can verify mount/resolve/unresolved-refs end-to-end.
    struct FakeAdapter {
        manifest: Manifest,
        symbols: HashMap<Uuid, Symbol>,
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
            }
        }

        fn with_symbol(mut self, sym: Symbol) -> Self {
            self.symbols.insert(sym.uuid, sym);
            self
        }
    }

    impl LibraryAdapter for FakeAdapter {
        fn manifest(&self) -> &Manifest {
            &self.manifest
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
        let adapter = FakeAdapter::new(lib_id).with_symbol(sym.clone());
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

    #[test]
    fn mount_rejects_duplicate_library_id() {
        let lib_id = Uuid::now_v7();
        let mut set = LibrarySet::new();
        set.mount(Box::new(FakeAdapter::new(lib_id))).unwrap();
        let dup = set.mount(Box::new(FakeAdapter::new(lib_id)));
        assert!(matches!(dup, Err(LibraryError::Conflict(_))));
        assert_eq!(set.len(), 1);
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
        // remount under same library_id replaces the adapter and
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
