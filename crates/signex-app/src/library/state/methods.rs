//! `LibraryState` / `OpenLibrary` methods (open/close/refresh/reload).

use super::*;

impl Default for LibraryState {
    fn default() -> Self {
        let mut settings = DistributorSettings::default();
        // Rehydrate the preferred-order list from
        // `<config_dir>/signex/distributors.toml`.
        settings.preferred_order = super::super::settings::persistence::load_preferred_order();
        Self {
            set: LibrarySet::new(),
            open_libraries: Vec::new(),
            editors: HashMap::new(),
            where_used: WhereUsedIndex::new(),
            picker: None,
            settings,
            expanded: Vec::new(),
            panel_search: String::new(),
            new_component: None,
            close_library_confirm: None,
            template_registry: Arc::new(TemplateRegistry::new_with_builtins()),
            library_browsers: HashMap::new(),
            primitive_picker: None,
            document_options: None,
            recovery: None,
            create_options: None,
            library_updates: None,
            skipped_updates_for: std::collections::HashSet::new(),
            installed_libraries: Vec::new(),
            global_libraries: Vec::new(),
            components_panel: ComponentsPanelState::default(),
        }
    }
}

impl LibraryState {
    /// Look up an open library by its on-disk root path.
    pub fn library_at(&self, path: &Path) -> Option<&OpenLibrary> {
        self.open_libraries.iter().find(|lib| lib.root == path)
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
        // `LibrarySet::mount` rejects duplicate `library_id`s — this
        // surfaces the case where the user has copy-pasted a `.snxlib/`
        // without regenerating the manifest UUID, so cross-library
        // `PrimitiveRef`s can't silently resolve to the wrong file.
        self.set.mount(Box::new(adapter))?;
        let mut entry = OpenLibrary {
            root,
            display_name,
            library_id,
            tables: HashMap::new(),
            cached_components: Vec::new(),
            cached_symbols: Vec::new(),
            cached_footprints: Vec::new(),
            cached_sims: Vec::new(),
            display: LibraryDisplaySettings::default(),
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
        self.expanded.push(true);
        Ok(())
    }

    /// Drop the library backing `root` — unmounts from `set` and drops
    /// every editor pointing at it. (TODO(v0.9): unsaved-edits prompt
    /// is wired from the dispatcher via `dirty_editors_for_library`.)
    pub fn close_library(&mut self, root: &Path) {
        if let Some(idx) = self.open_libraries.iter().position(|lib| lib.root == root) {
            let entry = self.open_libraries.remove(idx);
            self.set.unmount(entry.library_id);
            if idx < self.expanded.len() {
                self.expanded.remove(idx);
            }
            self.editors.retain(|key, _| key.library_path != root);
        }
    }

    /// Refresh the cached table contents for a library — re-reads every
    /// TSV via the mounted adapter's `list_tables` + `read_table`.
    ///
    /// Returns `LibraryError::NotFound` when the library at `root`
    /// isn't mounted, otherwise the underlying adapter error. The
    /// cached `Vec<ComponentSummary>` is rebuilt alongside `tables`
    /// so the picker (summary tier) and the panel grid (row tier)
    /// stay coherent.
    pub fn refresh_components(&mut self, root: &Path) -> Result<(), LibraryError> {
        let library_id = self
            .library_at(root)
            .map(|lib| lib.library_id)
            .ok_or_else(|| LibraryError::NotFound(root.display().to_string()))?;
        // Snapshot the table data through the adapter, then move it
        // onto the OpenLibrary entry. Two passes keep the borrow
        // checker happy — `set.get` and `library_at_mut` both
        // straddle `&self`/`&mut self`.
        let adapter = self
            .set
            .get(library_id)
            .ok_or_else(|| LibraryError::NotFound(root.display().to_string()))?;
        let names = adapter.list_tables()?;
        let mut tables: HashMap<String, Vec<ComponentRow>> = HashMap::new();
        let mut summaries: Vec<ComponentSummary> = Vec::new();
        for name in names {
            match adapter.read_table(&name) {
                Ok(rows) => {
                    for row in &rows {
                        summaries.push(ComponentSummary {
                            row_id: row.row_id,
                            internal_pn: row.internal_pn.clone(),
                            mpn: row.primary_mpn.mpn.clone(),
                            state: row.state,
                            description: String::new(),
                        });
                    }
                    tables.insert(name, rows);
                }
                Err(e) => {
                    tracing::warn!(
                        target: "signex::library",
                        table = %name,
                        error = %e,
                        "refresh_components: read_table failed; entry left empty"
                    );
                    tables.insert(name, Vec::new());
                }
            }
        }
        // Snapshot primitive listings before the second mut borrow so
        // we don't have to call adapter methods while `library_at_mut`
        // holds &mut self.
        let symbols = adapter.list_symbols().unwrap_or_default();
        let footprints = adapter.list_footprints().unwrap_or_default();
        let sims = adapter.list_sims().unwrap_or_default();
        if let Some(lib) = self.library_at_mut(root) {
            lib.tables = tables;
            lib.cached_components = summaries;
            lib.cached_symbols = symbols;
            lib.cached_footprints = footprints;
            lib.cached_sims = sims;
        }
        Ok(())
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

    /// Replace the Where-Used entries for one `(project, sheet)` with
    /// `refs` — `(row_id, instance_id)` tuples. The index keys by
    /// `RowId` directly; revisions and per-instance version pins are
    /// not part of the DBLib model.
    #[allow(dead_code)]
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
        let mut keys: Vec<EditorAddress> = self
            .editors
            .iter()
            .filter(|(key, st)| key.library_path == root && st.dirty)
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
    #[allow(dead_code)]
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
    /// Per-library canvas display settings — Altium "Document
    /// Options" parity. Shared across every `.snxsym` /
    /// `.snxfpt` tab opened from this `.snxlib` (so a user
    /// switching between symbols in the same library keeps the
    /// same grid / unit / background colour). In-memory only as
    /// of v0.9; v0.9.x can persist to `library.toml`.
    pub display: LibraryDisplaySettings,
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
    /// When on, pins are selectable/draggable by their name or
    /// number label, and a selected pin's labels glow with it.
    /// Grid-toggle-style per-tab setting.
    pub pin_label_grab: bool,
}

impl Default for LibraryDisplaySettings {
    fn default() -> Self {
        // Altium SchLib defaults: cream sheet + fine 50-mil (1.27 mm)
        // grid + visible-on. Matches the look in the user's reference
        // screenshot of an Altium .SchLib document.
        Self {
            unit: Unit::Mm,
            grid_size_mm: crate::fonts::read_symbol_grid_size_mm_pref(),
            grid_visible: true,
            sheet_color: SheetColor::Cream,
            pin_label_grab: false,
        }
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
        let names = adapter.list_tables()?;
        let mut tables: HashMap<String, Vec<ComponentRow>> = HashMap::new();
        let mut summaries: Vec<ComponentSummary> = Vec::new();
        for name in names {
            match adapter.read_table(&name) {
                Ok(rows) => {
                    for row in &rows {
                        summaries.push(ComponentSummary {
                            row_id: row.row_id,
                            internal_pn: row.internal_pn.clone(),
                            mpn: row.primary_mpn.mpn.clone(),
                            state: row.state,
                            description: String::new(),
                        });
                    }
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
        self.cached_components = summaries;
        self.reload_primitives(adapter);
        Ok(())
    }

    /// Refresh the cached `(symbols, footprints, sims)` summary
    /// lists. Called by `reload_tables` (full open/refresh path) and
    /// by `save_primitive_tab_at` after a standalone editor write so
    /// the picker modal sees the new primitive without re-scanning
    /// the filesystem on every view tick.
    ///
    /// Adapter errors degrade to empty lists with a tracing warn —
    /// the picker just shows fewer entries until the next refresh.
    pub fn reload_primitives(&mut self, adapter: &dyn LibraryAdapter) {
        match adapter.list_symbols() {
            Ok(v) => self.cached_symbols = v,
            Err(e) => {
                tracing::warn!(
                    target: "signex::library",
                    library = %self.display_name,
                    error = %e,
                    "list_symbols failed; cache left empty"
                );
                self.cached_symbols = Vec::new();
            }
        }
        match adapter.list_footprints() {
            Ok(v) => self.cached_footprints = v,
            Err(e) => {
                tracing::warn!(
                    target: "signex::library",
                    library = %self.display_name,
                    error = %e,
                    "list_footprints failed; cache left empty"
                );
                self.cached_footprints = Vec::new();
            }
        }
        match adapter.list_sims() {
            Ok(v) => self.cached_sims = v,
            Err(e) => {
                tracing::warn!(
                    target: "signex::library",
                    library = %self.display_name,
                    error = %e,
                    "list_sims failed; cache left empty"
                );
                self.cached_sims = Vec::new();
            }
        }
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

