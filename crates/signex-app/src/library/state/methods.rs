//! `LibraryState` / `OpenLibrary` methods (open/close/refresh/reload).

use super::*;

/// Move one primitive listing onto its cache, keeping the previous
/// contents when the adapter failed.
///
/// A listing failure is transient — a lock held by another process, a
/// library server 500, an unreadable `symbols/` directory — but the
/// cache it feeds is the *only* thing the picker renders. Substituting
/// an empty vec turns "the listing failed" into "this library has no
/// symbols", so the user re-creates a primitive that is already on
/// disk. The stale cache is the better answer: it is what the picker
/// showed a moment ago, and the next successful refresh replaces it.
///
/// The failure itself reaches the Messages panel at `warn` — the same
/// treatment `refresh_components` gives a failed `read_table`.
pub(super) fn apply_primitive_listing(
    cache: &mut Vec<PrimitiveSummary>,
    listing: Result<Vec<PrimitiveSummary>, LibraryError>,
    library: &str,
    kind: &str,
) -> bool {
    match listing {
        Ok(entries) => {
            *cache = entries;
            true
        }
        Err(error) => {
            tracing::warn!(
                target: "signex::library",
                library = %library,
                listing = kind,
                error = %error,
                "primitive listing failed; previous cache kept"
            );
            false
        }
    }
}

/// True when every listing succeeded and the symbol listing is not
/// missing unreadable containers: only then can absence be reported.
pub(super) fn listings_complete(ok: [bool; 3], symbols_partial: bool) -> bool {
    ok.iter().all(|listed| *listed) && !symbols_partial
}

impl Default for LibraryState {
    fn default() -> Self {
        let mut settings = DistributorSettings::default();
        // Rehydrate the preferred-order list from
        // `<config_dir>/signex/distributors.toml`.
        settings.preferred_order = super::super::settings::persistence::load_preferred_order();
        Self {
            drift: Default::default(),
            symbol_choice_requests: Default::default(),
            set: LibrarySet::new(),
            open_libraries: Vec::new(),
            editors: HashMap::new(),
            where_used: WhereUsedIndex::new(),
            picker: None,
            settings,
            panel_search: String::new(),
            close_library_confirm: None,
            batch_row_failures: HashMap::new(),
            template_registry: Arc::new(TemplateRegistry::new_with_builtins()),
            library_browsers: HashMap::new(),
            shown_browser: None,
            primitive_picker: None,
            document_options: None,
            recovery: None,
            create_options: None,
            library_updates: None,
            queued_library_updates: Vec::new(),
            skipped_updates_for: std::collections::HashSet::new(),
            installed_libraries: Vec::new(),
            global_libraries: Vec::new(),
            stock_library: None,
            stock_library_root: None,
            components_panel: ComponentsPanelState::default(),
            pending_mounts: HashMap::new(),
            external_poll_in_flight: false,
            external_poll_stat_running: Default::default(),
            external_poll_stall_reported: false,
            external_poll_failure_reported: false,
            pricing_refresh: None,
            clock: chrono::Utc::now(),
            refresh_generations: HashMap::new(),
            refresh_generation_counter: 0,
            refresh_in_flight: HashMap::new(),
            pending_follow_ups: HashMap::new(),
            data_version: 0,
            new_library_defaults: NewLibraryDefaults::default(),
        }
    }
}

impl LibraryState {
    /// Look up an open library by its on-disk root path.
    pub fn library_at(&self, path: &Path) -> Option<&OpenLibrary> {
        self.open_libraries.iter().find(|lib| lib.root == path)
    }

    /// On-disk file of the primitive `r` names, looked up in the open
    /// library `r.library_id` (`<dir>/<uuid>.<ext>`). `None` when that
    /// library is not open.
    pub fn primitive_file(
        &self,
        r: &signex_library::PrimitiveRef,
        dir: &str,
        ext: &str,
    ) -> Option<PathBuf> {
        let lib = self
            .open_libraries
            .iter()
            .find(|lib| lib.library_id == r.library_id)?;
        Some(lib.root_dir()?.join(dir).join(format!("{}.{ext}", r.uuid)))
    }

    pub fn library_at_mut(&mut self, path: &Path) -> Option<&mut OpenLibrary> {
        self.open_libraries.iter_mut().find(|lib| lib.root == path)
    }

    /// Find the open library whose `root_dir` is an ancestor of
    /// `child_path`. Used to resolve `.snxsym` / `.snxfpt` files back
    /// to the `.snxlib` they live alongside — e.g. for sourcing
    /// per-library canvas display settings
    /// ([`LibraryDisplaySettings`]).
    ///
    /// Per `v0.9-snxlib-as-file-plan.md` §2 Stage C the comparison
    /// is against the `.snxlib`'s *parent directory*, not the file
    /// itself, so `<root_dir>/symbols/foo.snxsym` correctly resolves
    /// to its library.
    pub fn containing_library(&self, child_path: &Path) -> Option<&OpenLibrary> {
        self.open_libraries.iter().find(|lib| {
            lib.root_dir()
                .map(|d| child_path.starts_with(d))
                .unwrap_or(false)
        })
    }

    pub fn containing_library_mut(&mut self, child_path: &Path) -> Option<&mut OpenLibrary> {
        self.open_libraries.iter_mut().find(|lib| {
            lib.root_dir()
                .map(|d| child_path.starts_with(d))
                .unwrap_or(false)
        })
    }

    /// Open the `*.snxlib/` at `root`, mounting the adapter under its
    /// `library_id` on `set` and registering the display entry in
    /// `open_libraries`. Idempotent.
    ///
    /// Also primes the per-library `tables` cache by running
    /// `list_tables` + `read_table` for every table the adapter
    /// exposes. Read errors warn through `tracing` and the affected
    /// entries are left empty — one bad table doesn't sink the open
    /// flow.
    pub fn open_library(&mut self, root: PathBuf) -> Result<(), LibraryError> {
        self.open_library_marked(root, false)
    }

    /// Mount the library at `root` read-only — the stock library that
    /// ships with Signex. Every write through the mounted adapter fails
    /// with `LibraryError::ReadOnly` whatever the install directory's
    /// permissions are. Idempotent like [`Self::open_library`].
    pub fn open_library_read_only(&mut self, root: PathBuf) -> Result<(), LibraryError> {
        self.open_library_marked(root, true)
    }

    /// True when the library mounted at `root` is read-only.
    pub fn is_read_only(&self, root: &Path) -> bool {
        self.set.is_read_only_path(root)
    }

    /// Display name of the read-only library whose directory contains
    /// `path`, or `None` when `path` is outside every read-only mount.
    pub fn read_only_library_name_containing(&self, path: &Path) -> Option<String> {
        // Compared on resolved paths: a symlinked, differently cased or
        // `..` spelling of a file inside the mount must not slip past.
        let path = resolved_path(path);
        self.open_libraries
            .iter()
            .find(|lib| {
                self.is_read_only(&lib.root)
                    && lib
                        .root_dir()
                        .is_some_and(|dir| path.starts_with(resolved_path(dir)))
            })
            .map(|lib| lib.display_name.clone())
    }

    /// True when `path` is, or lies inside, a read-only mount, or is
    /// exactly (resolved equality, no prefix match) the stock library
    /// file recorded in `stock_library_root` even when it failed to
    /// mount. Recovery flows that write (git re-init, binding
    /// repair) must never run against such a path.
    pub fn is_read_only_target(&self, path: &Path) -> bool {
        if self.read_only_library_name_containing(path).is_some() || self.is_read_only(path) {
            return true;
        }
        self.stock_library_root
            .as_deref()
            .is_some_and(|root| resolved_path(root) == resolved_path(path))
    }

    fn open_library_marked(&mut self, root: PathBuf, read_only: bool) -> Result<(), LibraryError> {
        if self.library_at(&root).is_some() {
            return Ok(());
        }
        let adapter = LocalGitAdapter::open(&root)?;
        // `manifest()` is on the LibraryAdapter trait; bring it into scope
        // via the trait import here so we don't widen the public surface.
        use signex_library::LibraryAdapter as _;
        let manifest = adapter.manifest();
        let display_name = manifest.library.name.clone();
        let library_id = manifest.library.library_id;
        // `LibrarySet::mount` keys file-backed adapters by their
        // `.snxlib` *path*, so the `Conflict` it can return here means
        // "this same path is already mounted" — not "this library_id is
        // already taken". Two copies of a library sharing a `library_id`
        // at different paths both mount fine, by design (see
        // `adapters/library_set.rs`, "Resolution semantics").
        //
        // So this is NOT a guard against the copy-pasted-`.snxlib`-
        // without-a-regenerated-manifest-UUID case: `set.get(library_id)`
        // returns the *first* match, and a cross-library `PrimitiveRef`
        // can still resolve to whichever copy wins. Detecting and
        // surfacing that collision is the Components Panel's job at
        // presentation time.
        if read_only {
            self.set.mount_read_only(Box::new(adapter))?;
        } else {
            self.set.mount(Box::new(adapter))?;
        }
        let mut entry = OpenLibrary {
            root,
            display_name,
            library_id,
            tables: HashMap::new(),
            cached_components: Vec::new(),
            cached_symbols: Vec::new(),
            cached_footprints: Vec::new(),
            cached_sims: Vec::new(),
            cached_classes: Vec::new(),
            observed_stamp: None,
            dangling: Default::default(),
            pricing: Default::default(),
            own_edit_stamp: None,
            display: LibraryDisplaySettings::seeded(&self.new_library_defaults),
        };
        if let Some(adapter) = self.set.get(library_id)
            && let Err(e) = entry.reload_tables(adapter)
        {
            tracing::warn!(
                target: "signex::library",
                library_id = %library_id,
                error = %e,
                "open_library: reload_tables failed; library opens with empty cache"
            );
        }
        self.open_libraries.push(entry);
        self.touch_data();
        self.sweep_dangling_refs();
        Ok(())
    }

    /// Stop the Refresh All Pricing walk, if any, wherever it is. The walk
    /// checks the flag between rows and again before it saves, so nothing is
    /// written after this returns. Used on app quit.
    pub fn cancel_pricing_refresh(&self) {
        if let Some(run) = &self.pricing_refresh {
            run.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }

    /// Stop the walk if it is for one of `libraries` (a library
    /// unmounting).
    pub fn cancel_pricing_refresh_of(&self, libraries: &[PathBuf]) {
        if let Some(run) = &self.pricing_refresh
            && libraries.contains(&run.library_path)
        {
            run.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }

    /// Drop the library backing `root` — unmounts from `set` and drops
    /// every editor pointing at it. (TODO(v0.9): unsaved-edits prompt
    /// is wired from the dispatcher via `dirty_editors_for_library`.)
    pub fn close_library(&mut self, root: &Path) {
        // A pricing walk of this library must not write after it is closed.
        self.cancel_pricing_refresh_of(&[root.to_path_buf()]);
        // Cancel any mount still being prepared off-thread for this path
        // (#99 part 2c). `pending_mounts` doubles as the cancellation
        // tombstone: `take_mount_intent` returns `None` for a path that
        // is gone, so an in-flight preparation that lands after this
        // point drops its payload instead of re-mounting a library the
        // user just closed. Unconditional — the entry only exists while
        // a preparation is in flight, and closing a library that was
        // mounted synchronously simply has nothing to remove.
        self.pending_mounts.remove(root);
        self.refresh_generations.remove(root);
        self.refresh_in_flight
            .retain(|_, in_flight| in_flight.as_path() != root);
        self.pending_follow_ups.remove(root);
        if let Some(idx) = self.open_libraries.iter().position(|lib| lib.root == root) {
            let entry = self.open_libraries.remove(idx);
            self.touch_data();
            self.set.unmount(entry.library_id);
            self.editors.retain(|key, _| key.library_path != root);
            // Names, drift rows and Update must not use an unmounted library.
            self.drift.check_pending = true;
        }
    }

    /// Each open library with the stamp its caches were filled under — the
    /// input of the external-edit poll.
    pub fn watched_libraries(&self) -> Vec<super::super::external_poll::WatchedLibrary> {
        self.open_libraries
            .iter()
            .map(|lib| (lib.root.clone(), lib.observed_stamp))
            .collect()
    }

    /// Re-read only the class registry of the library at `root`, after a
    /// class mutation. Leaves `observed_stamp` alone on purpose: the file
    /// changed, the table caches were not refreshed with it, and the next
    /// external-edit poll must still notice that. It records the stamp the
    /// write left in `own_edit_stamp`, so that refresh is not reported as an
    /// external edit (#827).
    pub fn refresh_classes(&mut self, root: &Path) {
        let Some(classes) = self
            .library_at(root)
            .and_then(|lib| self.set.get(lib.library_id))
            .map(|adapter| adapter.library_classes())
        else {
            return;
        };
        if let Some(lib) = self.library_at_mut(root) {
            lib.cached_classes = classes;
            lib.own_edit_stamp = signex_library::DiskStamp::of(root).ok();
        }
    }

    /// Aggregate every open library's cached components — used by the
    /// picker modal to flatten across libraries.
    pub fn all_components(&self) -> Vec<(PathBuf, ComponentSummary)> {
        let mut out = Vec::new();
        for lib in &self.open_libraries {
            for c in &lib.cached_components {
                out.push((lib.root.clone(), c.clone()));
            }
        }
        out
    }

    /// The table of the open library at `root` that holds `row_id`.
    pub fn table_of_row(&self, root: &Path, row_id: Uuid) -> Option<String> {
        let lib = self.library_at(root)?;
        lib.tables
            .iter()
            .find(|(_, rows)| rows.iter().any(|r| r.row_id == row_id))
            .map(|(name, _)| name.clone())
    }

    /// Replace the Where-Used entries for one `(project, sheet)` with
    /// `refs` — `(row_id, instance_id)` tuples. The index keys by
    /// `RowId` directly; revisions and per-instance version pins are
    /// not part of the DBLib model.
    pub fn ingest_sheet(&mut self, project: &Path, sheet: &Path, refs: &[(Uuid, String)]) {
        let trimmed: Vec<(RowId, String)> = refs
            .iter()
            .map(|(uuid, inst)| (RowId::from_uuid(*uuid), inst.clone()))
            .collect();
        self.where_used.ingest_sheet(project, sheet, &trimmed);
    }

    /// Look up the use-sites for a row.
    pub fn where_used_for(&self, row_id: RowId) -> Vec<UseSite> {
        self.where_used.where_used(row_id)
    }

    /// Editor addresses currently pointing at `root` that have unsaved edits.
    pub fn dirty_editors_for_library(&self, root: &Path) -> Vec<EditorAddress> {
        self.collect_dirty_editors(|key| key.library_path == root)
    }

    /// Every dirty editor address across all open libraries — the rows
    /// an app-quit prompt has to account for.
    pub fn dirty_editors(&self) -> Vec<EditorAddress> {
        self.collect_dirty_editors(|_| true)
    }

    fn collect_dirty_editors(
        &self,
        include: impl Fn(&EditorAddress) -> bool,
    ) -> Vec<EditorAddress> {
        let mut keys: Vec<EditorAddress> = self
            .editors
            .iter()
            .filter(|(key, st)| st.dirty && include(key))
            .map(|(key, _)| key.clone())
            .collect();
        keys.sort_by(|a, b| {
            a.library_path
                .cmp(&b.library_path)
                .then_with(|| a.table.cmp(&b.table))
                .then_with(|| a.row_id.cmp(&b.row_id))
        });
        keys
    }

    /// Human label for an editor row in a confirm modal: its part
    /// number and table, falling back to the raw address when the
    /// editor is already gone.
    pub fn editor_label(&self, address: &EditorAddress) -> String {
        match self.editors.get(address) {
            Some(st) => format!("{} ({})", st.row.internal_pn.as_str(), address.table),
            None => format!("{} ({})", address.row_id, address.table),
        }
    }

    /// Classify how a mounted library got there — drives the
    /// Components Panel section bucketing (Stage 9). Project
    /// libraries take precedence over Installed/Global so a global
    /// library that's also referenced by the active project
    /// surfaces under the "Project" header.
    pub fn mount_source_for(
        &self,
        path: &Path,
        project_paths: &[PathBuf],
    ) -> ComponentsMountSource {
        if project_paths.iter().any(|p| p == path) {
            return ComponentsMountSource::Project;
        }
        if self.stock_library.as_deref() == Some(path) {
            return ComponentsMountSource::Stock;
        }
        if self.installed_libraries.iter().any(|p| p == path) {
            return ComponentsMountSource::Installed;
        }
        if self.global_libraries.iter().any(|e| e.path == path) {
            return ComponentsMountSource::Global;
        }
        // Default: treat unknown mounts (e.g. a library opened via
        // File ▸ Library ▸ Open Library… before Stage 9) as
        // Installed so they still surface in the panel.
        ComponentsMountSource::Installed
    }

    /// Existing editor for `(library_root, table, row_id)`, if any.
    pub fn editor_for(
        &self,
        library_root: &Path,
        table: &str,
        row_id: RowId,
    ) -> Option<&ComponentPreviewState> {
        self.editors.get(&EditorAddress::new(
            library_root.to_path_buf(),
            table.to_string(),
            row_id,
        ))
    }
}

/// One open `*.snxlib/` directory — display cache only. The owning
/// `LibraryAdapter` lives on [`LibraryState::set`] keyed by
/// `library_id`.
///
/// Components are rows in per-category TSV tables, not standalone
/// files. The display cache is keyed by table name; each entry holds
/// the full row payload so the panel can render a grid view per
/// category without re-reading disk between view ticks. Every row
/// write triggers a full table reload (v0.9 keeps it simple — hot
/// per-row patches are a polish item).
pub struct OpenLibrary {
    /// Absolute path to the `.snxlib` *file* itself. Per
    /// `v0.9-snxlib-as-file-plan.md` §1, a library is a directory
    /// holding a `.snxlib` file plus sibling `symbols/` /
    /// `footprints/` / `sims/` subdirs; this field points at the
    /// `.snxlib` file. The git working tree (and `symbols/` parent)
    /// is reachable via [`OpenLibrary::root_dir`].
    ///
    /// Field name kept as `root` to minimise ripple through ~14
    /// callers; Stage 12+ can rename to `path` once the v0.9 sweep
    /// settles.
    pub root: PathBuf,
    pub display_name: String,
    pub library_id: Uuid,
    /// Cached table contents — keyed by table filename stem. Populated
    /// on `open_library` by scanning every TSV via `list_tables` +
    /// `read_table`. The Library panel renders a category node per
    /// entry and an inline row grid per category.
    pub tables: HashMap<String, Vec<ComponentRow>>,
    /// Last-loaded summary list — used by `picker.rs` (and any
    /// caller that wants a flat per-library view); kept in lock-step
    /// with `tables` by `reload_tables`.
    pub cached_components: Vec<ComponentSummary>,
    /// Cached primitive listings — populated by `reload_tables` /
    /// `reload_primitives` so the per-tick `primitive_picker.rs`
    /// view doesn't have to re-walk the filesystem on every keystroke.
    /// One per primitive kind. Refreshed when a standalone primitive
    /// editor saves (best-effort).
    pub cached_symbols: Vec<PrimitiveSummary>,
    pub cached_footprints: Vec<PrimitiveSummary>,
    pub cached_sims: Vec<PrimitiveSummary>,
    /// The class registry as `view` shows it. `view` must not call
    /// `LibraryAdapter::library_classes` (it `stat`s the file and may
    /// re-parse it); `update` refreshes this on mount, after a class
    /// mutation and when the external-edit poll sees the file change.
    pub cached_classes: Vec<signex_library::ClassEntry>,
    /// Stamp of the `.snxlib` taken just before the caches above were
    /// last filled. The external-edit poll compares the file against it
    /// and refreshes the caches when they differ. `None` = unknown, which
    /// the poll treats as "refresh on the next tick".
    pub observed_stamp: Option<signex_library::DiskStamp>,
    /// Stamp of the file right after the app's own class write, which
    /// [`LibraryState::refresh_classes`] leaves un-re-stamped on purpose. A
    /// poll that finds exactly this stamp refreshes without reporting an
    /// external change; any other stamp is a real external edit (#827).
    pub own_edit_stamp: Option<signex_library::DiskStamp>,
    /// Per-library canvas display settings — Altium "Document
    /// Options" parity. Shared across every `.snxsym` /
    /// `.snxfpt` tab opened from this `.snxlib` (so a user
    /// switching between symbols in the same library keeps the
    /// same grid / unit / background colour). In-memory only as
    /// of v0.9; v0.9.x can persist to `library.toml`.
    pub display: LibraryDisplaySettings,
    /// Rows whose symbol / footprint / sim binding resolves to nothing,
    /// from the last [`LibraryState::sweep_dangling_refs`].
    pub dangling: crate::library::dangling::DanglingSweep,
    /// Cached distributor prices by `row_id`, as the last
    /// [`LibraryState::refresh_components`] read them. `view` reads this;
    /// it never asks the adapter.
    pub pricing: HashMap<Uuid, signex_library::RowPricing>,
}

/// Per-library canvas + UI defaults shared across every primitive
/// editor tab opened from the same `.snxlib`. See [`OpenLibrary::display`].
#[derive(Debug, Clone, Copy)]
pub struct LibraryDisplaySettings {
    /// Coordinate display unit (mm / mil / inch / um) shown in the
    /// per-tab status footer.
    pub unit: Unit,
    /// Visible grid spacing in mm. Cycled through
    /// `crate::canvas::grid::GRID_SIZES_MM`.
    pub grid_size_mm: f32,
    /// Whether the visible dot grid renders.
    pub grid_visible: bool,
    /// Sheet background colour preset — Altium "Sheet Color"
    /// (Black / White / Dark Gray / Light Gray / Cream).
    pub sheet_color: SheetColor,
    /// How a click selects/drags a pin — pin body only (Altium
    /// parity) or pin body plus its name/number labels (which then
    /// glow with the selected pin). Seeded from the
    /// `symbol_pin_selection` preference when a library is opened.
    pub pin_selection: crate::render_config::PinSelectionMode,
}

impl LibraryDisplaySettings {
    /// Altium SchLib defaults: cream sheet + visible grid, with the grid
    /// size and pin selection taken from `defaults` (the model's copy of the
    /// preferences, never a fresh disk read).
    pub fn seeded(defaults: &NewLibraryDefaults) -> Self {
        Self {
            unit: Unit::Mm,
            grid_size_mm: defaults.grid_size_mm,
            grid_visible: true,
            sheet_color: SheetColor::Cream,
            pin_selection: defaults.pin_selection,
        }
    }
}

impl Default for LibraryDisplaySettings {
    /// Same look as [`Self::seeded`] with the shipped defaults. A placeholder
    /// for fixtures and the off-thread mount prepare; the app seeds the real
    /// values from `LibraryState` when the library is mounted.
    fn default() -> Self {
        Self::seeded(&NewLibraryDefaults::default())
    }
}

impl OpenLibrary {
    /// Directory holding the `.snxlib` file — the per-library git
    /// working tree and parent of `symbols/` / `footprints/` /
    /// `sims/`. Returns `None` only when `root` is rooted (no
    /// parent), which shouldn't happen for legitimate libraries
    /// since `.snxlib` always lives inside its parent dir.
    ///
    /// Use this whenever you need to compare paths against the
    /// library's working tree (e.g. "is this `.snxsym` inside this
    /// library?") or join sibling paths
    /// (`root_dir().join("symbols")`).
    pub fn root_dir(&self) -> Option<&Path> {
        self.root.parent()
    }

    /// Re-read every TSV via the supplied adapter. Replaces both
    /// `tables` and `cached_components` atomically — readers see one
    /// consistent snapshot regardless of which view they query.
    ///
    /// `LibraryError::Backend` from the adapter (e.g. an adapter that
    /// hasn't implemented `list_tables` yet) is propagated to the
    /// caller; partial table loads on per-table read errors are
    /// tolerated and merely warn through `tracing`.
    pub fn reload_tables(&mut self, adapter: &dyn LibraryAdapter) -> Result<(), LibraryError> {
        // Stamp before reading: a write racing the read then costs one
        // extra refresh on the next poll, never a missed one.
        let stamp = signex_library::DiskStamp::of(&self.root).ok();
        let names = adapter.list_tables()?;
        let mut tables: HashMap<String, Vec<ComponentRow>> = HashMap::new();
        let mut summaries: Vec<ComponentSummary> = Vec::new();
        let mut pricing: HashMap<Uuid, signex_library::RowPricing> = HashMap::new();
        for name in names {
            match adapter.table_pricing(&name) {
                Ok(cached) => pricing.extend(cached),
                Err(e) => tracing::warn!(
                    target: "signex::library",
                    table = %name,
                    error = %e,
                    "reload_tables: cached prices could not be read; none are shown"
                ),
            }
            match adapter.read_table(&name) {
                Ok(rows) => {
                    summaries.extend(rows.iter().map(summarize_row));
                    tables.insert(name, rows);
                }
                Err(e) => {
                    tracing::warn!(
                        target: "signex::library",
                        table = %name,
                        error = %e,
                        "read_table failed; entry left empty"
                    );
                    tables.insert(name, Vec::new());
                }
            }
        }
        self.tables = tables;
        self.pricing = pricing;
        self.cached_components = summaries;
        self.cached_classes = adapter.library_classes();
        self.observed_stamp = stamp;
        self.own_edit_stamp = None;
        self.reload_primitives(adapter);
        Ok(())
    }

    /// Refresh the cached `(symbols, footprints, sims)` summary
    /// lists. Called by `reload_tables` (full open/refresh path) and
    /// by `save_primitive_tab_at` after a standalone editor write so
    /// the picker modal sees the new primitive without re-scanning
    /// the filesystem on every view tick.
    ///
    /// An adapter error keeps the previous cache and emits a tracing
    /// warn — the picker keeps showing the entries it showed a moment
    /// ago until the next successful refresh. Assigning an empty vec
    /// here (what this did before) reported a transient listing
    /// failure as an empty library.
    pub fn reload_primitives(&mut self, adapter: &dyn LibraryAdapter) {
        let symbols = adapter.list_symbols();
        let symbols_partial = adapter.symbol_listing_is_partial();
        let footprints = adapter.list_footprints();
        let sims = adapter.list_sims();
        let name = &self.display_name;
        let ok = [
            apply_primitive_listing(&mut self.cached_symbols, symbols, name, "symbols"),
            apply_primitive_listing(&mut self.cached_footprints, footprints, name, "footprints"),
            apply_primitive_listing(&mut self.cached_sims, sims, name, "sims"),
        ];
        self.dangling.listings_complete = listings_complete(ok, symbols_partial);
    }

    /// Total number of rows across every cached table.
    pub fn total_rows(&self) -> usize {
        self.tables.values().map(|v| v.len()).sum()
    }
}

impl std::fmt::Debug for OpenLibrary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenLibrary")
            .field("root", &self.root)
            .field("display_name", &self.display_name)
            .field("library_id", &self.library_id)
            .field("tables_len", &self.tables.len())
            .field("total_rows", &self.total_rows())
            .finish()
    }
}

/// `path` with symlinks, `..` and case differences resolved. A path
/// that does not exist yet (a save target) resolves through its deepest
/// existing ancestor, with the remaining components appended after
/// folding `.` and `..` lexically.
pub(crate) fn resolved_path(path: &Path) -> PathBuf {
    if let Ok(real) = path.canonicalize() {
        return real;
    }
    let mut missing: Vec<std::ffi::OsString> = Vec::new();
    let mut base = path.to_path_buf();
    loop {
        if let Ok(real) = base.canonicalize() {
            let mut out = real;
            for part in missing.iter().rev() {
                if part == ".." {
                    out.pop();
                } else if part != "." {
                    out.push(part);
                }
            }
            return out;
        }
        match (base.file_name(), base.parent()) {
            (Some(name), Some(parent)) => {
                missing.push(name.to_os_string());
                base = parent.to_path_buf();
            }
            _ => {
                if base.file_name().is_none() && base.ends_with("..") {
                    missing.push("..".into());
                    base = base.parent().map(Path::to_path_buf).unwrap_or_default();
                    continue;
                }
                return path.to_path_buf();
            }
        }
    }
}

/// Picker/panel summary for one table row.
pub(super) fn summarize_row(row: &ComponentRow) -> ComponentSummary {
    ComponentSummary {
        row_id: row.row_id,
        internal_pn: row.internal_pn.clone(),
        mpn: row.primary_mpn.mpn.clone(),
        state: row.state,
        description: row.description.clone(),
    }
}
