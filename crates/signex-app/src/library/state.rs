//! In-memory state for the Library subsystem (DBLib model).
//!
//! Owned by [`crate::app::Signex::library`]. In the v0.9-refactor-2
//! model, components are rows inside per-category TSV tables under
//! `<lib>/tables/<category>.tsv`, addressed by
//! `(library_path, table, row_id)`. The main pieces:
//!
//! * `set` — `signex_library::LibrarySet`, the cross-library resolver
//!   that maps `library_id → Box<dyn LibraryAdapter>`. Editors and
//!   renderers hand a `PrimitiveRef` to `set.resolve_*` to load
//!   `Symbol`/`Footprint`/`SimModel` primitives without knowing which
//!   library they live in.
//! * `open_libraries` — display caches per `*.snxlib/`. Each entry
//!   holds the root path, display name, and per-table `Vec<ComponentRow>`
//!   so the panel can render an inline grid per category without
//!   re-reading disk between view ticks.
//! * `editors` — one entry per Component Preview tab keyed by
//!   [`EditorAddress`]. The preview lives as a tab in the main
//!   window's tab bar and may be undocked into its own OS window via
//!   the standard tab-undock flow; either way the address is the
//!   stable identity, not the window id.
//! * `picker` — component picker modal state (used by schematic
//!   placement; flattens across every open library).
//! * `new_component` — modal state for the "New Row" flow
//!   (library + table + class + InternalPN).
//! * `template_registry` — bundled + per-library parameter templates,
//!   resolved at component-class lookup time.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use signex_library::{
    ComponentClass, ComponentRow, ComponentSummary, DistributorSource, Footprint, LibraryAdapter,
    LibraryError, LibrarySet, LocalGitAdapter, PrimitiveKind, PrimitiveRef, PrimitiveSummary,
    RowId, SimModel, Symbol, TemplateRegistry, UseSite, WhereUsedIndex,
};
use signex_types::coord::Unit;

use crate::panels::SheetColor;
use uuid::Uuid;

/// Identity for an open Component Preview tab — the lookup key for
/// [`LibraryState::editors`] and the address that preview view closures
/// clone into messages. Rows live in `tables/<name>.tsv` and are
/// addressed by `(library_path, table, row_id)`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EditorAddress {
    pub library_path: PathBuf,
    pub table: String,
    pub row_id: RowId,
}

impl EditorAddress {
    pub fn new(library_path: PathBuf, table: String, row_id: RowId) -> Self {
        Self {
            library_path,
            table,
            row_id,
        }
    }

    /// Synthetic on-disk identity for a Component Preview tab — used by
    /// `TabInfo.path` so the tab bar, undock detector, and dirty-paths
    /// machinery have a single unique `PathBuf` per row without needing
    /// a second identity scheme. The path points at the row's home table
    /// with the `row_id` as a suffix so the synthetic key is unique
    /// per-row even when multiple rows share a table.
    pub fn synthetic_tab_path(&self) -> PathBuf {
        self.library_path
            .join("tables")
            .join(format!("{}.tsv#{}", self.table, self.row_id))
    }
}

/// Per-browser-tab state — owned by a single
/// `TabKind::LibraryBrowser(path)` tab, keyed by the same `path` on
/// `LibraryState::library_browsers`. Deliverable B adds the
/// `edit_modal` field so double-click on a row opens a full-form
/// editor.
#[derive(Debug, Clone)]
pub struct LibraryBrowserState {
    pub library_path: PathBuf,
    /// Which category tab is active (filename stem of `tables/<name>.tsv`).
    /// `None` when a library has no tables yet (fresh library) — the
    /// view shows an empty-state panel with an "Add Component" CTA.
    pub active_table: Option<String>,
    /// Currently selected row in the active table — drives the side
    /// preview pane.
    pub selected_row: Option<RowId>,
    /// Filter text applied to row PN / MPN / manufacturer.
    pub search: String,
    /// Edit Component Details modal — opened by double-clicking a row
    /// in the grid. `None` while closed.
    pub edit_modal: Option<EditRowModalState>,
    /// Per-cell live-edit buffers for inline grid editing
    /// (Deliverable C). Keyed by `(row_id, column_key)` where
    /// column_key is `"internal_pn"`, `"manufacturer"`, `"mpn"`, or
    /// `"parameters.<key>"`.
    pub cell_edit: HashMap<(RowId, String), String>,
    /// Confirmation modal state for Delete Selected (Deliverable D).
    /// `Some` while the confirm modal is open.
    pub delete_confirm: Option<DeleteConfirmState>,
}

impl LibraryBrowserState {
    pub fn new(library_path: PathBuf) -> Self {
        Self {
            library_path,
            active_table: None,
            selected_row: None,
            search: String::new(),
            edit_modal: None,
            cell_edit: HashMap::new(),
            delete_confirm: None,
        }
    }
}

/// Delete-row confirmation modal — displayed when the user clicks
/// Delete Selected on the browser action row.
#[derive(Debug, Clone)]
pub struct DeleteConfirmState {
    pub table: String,
    pub row_id: RowId,
    pub internal_pn: String,
}

/// "Edit Component Details" modal state — opened by double-clicking a
/// row in the browser grid. The user edits a working copy; commit
/// fires `adapter.update_row` via `BrowserEditMsg::Save`.
#[derive(Debug, Clone)]
pub struct EditRowModalState {
    pub address: EditorAddress,
    /// Working copy — committed to the row on Save.
    pub draft: ComponentRow,
    /// Per-parameter live edit buffer — `(value_str, unit_str)` keyed
    /// by parameter name. Mirrors the `params_edit_buf` pattern used
    /// in the Component Preview tab.
    pub param_buf: HashMap<String, (String, String)>,
    /// Inline error text — surfaced if the save fails.
    pub error: Option<String>,
}

impl EditRowModalState {
    pub fn new(address: EditorAddress, draft: ComponentRow) -> Self {
        let param_buf: HashMap<String, (String, String)> = draft
            .parameters
            .iter()
            .map(|(k, v)| {
                let (val, unit) = match v {
                    signex_library::ParamValue::Text(s) => (s.clone(), String::new()),
                    signex_library::ParamValue::Number(n) => (n.to_string(), String::new()),
                    signex_library::ParamValue::Bool(b) => (b.to_string(), String::new()),
                    signex_library::ParamValue::Measurement { value, unit } => {
                        (value.to_string(), unit.clone())
                    }
                };
                (k.clone(), (val, unit))
            })
            .collect();
        Self {
            address,
            draft,
            param_buf,
            error: None,
        }
    }
}

/// Primitive picker modal state — opened when the user clicks
/// "Pick Symbol" or "Pick Footprint" on a Component Preview tab or in
/// the New Component modal.
#[derive(Debug, Clone)]
pub struct PrimitivePickerState {
    pub kind: PrimitiveKind,
    /// Where to send the result — addresses the Component Preview tab
    /// or the New Component modal.
    pub target: PrimitivePickerTarget,
    /// Live filter text.
    pub filter: String,
    /// Inline error, e.g. when filesystem-picked file isn't inside a
    /// `.snxlib`.
    pub error: Option<String>,
}

/// Where the picker should write the picked `PrimitiveRef`.
#[derive(Debug, Clone)]
pub enum PrimitivePickerTarget {
    /// The user picks for an open Component Preview tab — apply +
    /// save the row.
    PreviewRow(EditorAddress),
    /// The user picks while filling out the New Component modal — apply
    /// to `NewComponentState.symbol_ref` / `.footprint_ref`.
    NewComponentForm,
    /// The user picks while editing a row in the Library Browser
    /// grid's Edit Component Details modal (Deliverable B).
    EditRowModal(EditorAddress),
}

/// Top-level Library subsystem state. Stored on
/// [`crate::app::Signex`] as a single field so the dispatcher can
/// borrow it independently of the rest of `DocumentState`.
pub struct LibraryState {
    /// Cross-library resolver — maps `library_id → adapter`. New in
    /// the v0.9 refactor: editors load primitives by `PrimitiveRef`
    /// without knowing which `*.snxlib/` they came from.
    pub set: LibrarySet,
    /// Open `*.snxlib/` directories — display caches keyed by absolute
    /// root path. The adapter for each entry is mounted on `set`
    /// under its `library_id`.
    pub open_libraries: Vec<OpenLibrary>,
    /// Component Preview states currently open. Keyed by
    /// `(library_path, table, row_id)` per the DBLib row identity.
    /// Component Preview tabs are read-only for Symbol+Footprint;
    /// editing happens via standalone `.snxsym` / `.snxfpt` document
    /// tabs.
    pub editors: HashMap<EditorAddress, ComponentPreviewState>,
    /// Reverse "where-used" index — same shape as before.
    pub where_used: WhereUsedIndex,
    /// Picker modal state — `None` while the modal is closed.
    pub picker: Option<PickerState>,
    /// Distributor APIs settings panel state.
    pub settings: DistributorSettings,
    /// True while the Library left-dock panel's expanded library node
    /// at index `i` is open.
    pub expanded: Vec<bool>,
    /// Library left-dock search box buffer.
    pub panel_search: String,
    /// "New Component" modal state — `None` while closed.
    pub new_component: Option<NewComponentState>,
    /// "Close Library — Unsaved Drafts" modal state — `None` while closed.
    pub close_library_confirm: Option<CloseLibraryConfirmState>,
    /// Bundled + per-library parameter templates. Reference-counted
    /// because both the editor and the validator borrow it. The
    /// Component Preview tab + the validator both read through this
    /// registry; this struct owns the field.
    #[allow(dead_code)]
    pub template_registry: Arc<TemplateRegistry>,
    /// Per-browser-tab state, keyed by `.snxlib` root path. One entry
    /// per `TabKind::LibraryBrowser(path)` tab in the main window's
    /// tab bar; insert on tab open, drop on tab close.
    pub library_browsers: HashMap<PathBuf, LibraryBrowserState>,
    /// Primitive picker modal state — `None` while closed. Opened from
    /// the Component Preview tab's Symbol/Footprint pane, the New
    /// Component modal, and the Edit Component Details modal.
    pub primitive_picker: Option<PrimitivePickerState>,
    /// Tools ▸ Document Options modal state — `None` while closed.
    /// Opens against a specific `.snxlib` root path so the modal
    /// edits the matching `OpenLibrary.display`.
    pub document_options: Option<DocumentOptionsModalState>,
    /// Library recovery dialog — Stage 10 of v0.9-snxlib-as-file.
    /// `None` while closed; one of three modal flows when set.
    pub recovery: Option<super::recovery::RecoveryDialog>,
}

/// State for the Tools ▸ Document Options modal — keyed by the
/// containing `.snxlib` root path so the dispatcher knows which
/// `OpenLibrary.display` to mutate. Working draft + the modal's
/// scratch buffer. Apply on Save; discard on Cancel.
#[derive(Debug, Clone)]
pub struct DocumentOptionsModalState {
    pub library_path: PathBuf,
    pub library_name: String,
    pub draft: LibraryDisplaySettings,
}

impl Default for LibraryState {
    fn default() -> Self {
        let mut settings = DistributorSettings::default();
        // Rehydrate the preferred-order list from
        // `<config_dir>/signex/distributors.toml`.
        settings.preferred_order = super::settings::persistence::load_preferred_order();
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
}

impl Default for LibraryDisplaySettings {
    fn default() -> Self {
        // Altium SchLib defaults: cream sheet + fine 50-mil (1.27 mm)
        // grid + visible-on. Matches the look in the user's reference
        // screenshot of an Altium .SchLib document.
        Self {
            unit: Unit::Mm,
            grid_size_mm: 1.27,
            grid_visible: true,
            sheet_color: SheetColor::Cream,
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

/// Picker modal state.
#[derive(Debug, Clone, Default)]
pub struct PickerState {
    pub filter: String,
    pub selected: Option<(PathBuf, ComponentSummary)>,
}

// ─────────────────────────────────────────────────────────────────────
// New Component flow
// ─────────────────────────────────────────────────────────────────────

/// Built-in component classes — keep this list in sync with
/// `v0.9-refactor-2-plan.md`. The string is what gets stored on
/// `ComponentRow::class`; the label is what the picker shows.
pub const BUILTIN_CLASSES: &[(&str, &str)] = &[
    ("resistor", "Resistor"),
    ("capacitor", "Capacitor"),
    ("inductor", "Inductor"),
    ("diode", "Diode"),
    ("led", "LED"),
    ("transistor_bjt", "Transistor — BJT"),
    ("transistor_mosfet", "Transistor — MOSFET"),
    ("transistor_jfet", "Transistor — JFET"),
    ("opamp", "Op-Amp"),
    ("comparator", "Comparator"),
    ("regulator_linear", "Regulator — Linear"),
    ("regulator_switching", "Regulator — Switching"),
    ("mcu", "MCU"),
    ("logic", "Logic"),
    ("memory", "Memory"),
    ("adc", "ADC"),
    ("dac", "DAC"),
    ("connector", "Connector"),
    ("crystal", "Crystal"),
    ("oscillator", "Oscillator"),
    ("sensor", "Sensor"),
    ("mechanical", "Mechanical"),
    ("generic", "Generic"),
];

/// "New Component" modal state — collected before the dispatcher
/// inserts a row into the chosen target table and opens the
/// Component Preview tab.
#[derive(Debug, Clone)]
pub struct NewComponentState {
    /// Live edit buffer for the Internal PN field.
    pub internal_pn: String,
    /// Selected target library — index into `open_libraries`.
    pub library_idx: Option<usize>,
    /// Target table the row will be written to. `None` while the
    /// modal first opens; the dispatcher requires it before
    /// `NewComponentSubmit` can run because rows live in TSV tables
    /// addressed by name. Populated from `manifest().tables()` plus
    /// the default `<class>s` slot when the manifest declares no
    /// overrides.
    pub table: Option<String>,
    /// Picked component class — defaults to "generic".
    pub class: ComponentClass,
    /// Tree-style category path ("Passives/Resistors/0805"). Free-form
    /// — validation happens at submit time.
    pub category: String,
    /// Optional symbol primitive binding picked at modal time.
    /// `None` = leave the row's symbol_ref as the nil sentinel; the
    /// user can bind later via the Component Preview's Pick Symbol.
    pub symbol_ref: Option<PrimitiveRef>,
    /// Optional footprint primitive binding picked at modal time.
    pub footprint_ref: Option<PrimitiveRef>,
    /// Latest validation error.
    pub error: Option<String>,
}

impl Default for NewComponentState {
    fn default() -> Self {
        Self {
            internal_pn: String::new(),
            library_idx: None,
            table: None,
            class: ComponentClass::generic(),
            category: String::new(),
            symbol_ref: None,
            footprint_ref: None,
            error: None,
        }
    }
}

/// "Close Library — Unsaved Drafts" confirmation modal state.
#[derive(Debug, Clone)]
pub struct CloseLibraryConfirmState {
    pub library_path: PathBuf,
    pub library_name: String,
    pub dirty_editors: Vec<EditorAddress>,
}

// ─────────────────────────────────────────────────────────────────────
// Component Preview
// ─────────────────────────────────────────────────────────────────────

/// Component Preview tabs in display order.
///
/// The Component view is preview-only: Symbol and Footprint are
/// read-only renders; editing happens via the standalone
/// `.snxsym` / `.snxfpt` document editors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PreviewTab {
    Preview,
    Parameters,
    Supply,
    Datasheet,
    Simulation,
}

impl PreviewTab {
    pub const ORDER: &'static [PreviewTab] = &[
        PreviewTab::Preview,
        PreviewTab::Parameters,
        PreviewTab::Supply,
        PreviewTab::Datasheet,
        PreviewTab::Simulation,
    ];

    pub fn label(self) -> &'static str {
        match self {
            PreviewTab::Preview => "Preview",
            PreviewTab::Parameters => "Parameters",
            PreviewTab::Supply => "Supply",
            PreviewTab::Datasheet => "Datasheet",
            PreviewTab::Simulation => "Simulation",
        }
    }
}

/// Per-row inline pin-map editor state — which row is currently expanded
/// and the live buffer for the target pad-number input. The pin/pad
/// bindings themselves live on `ComponentRow::pin_map_overrides`; this
/// struct only holds the UI-only flags.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PinMapInlineState {
    /// `Some(pin_number)` while the override editor is expanded for
    /// that specific pin row. `None` when collapsed.
    pub expanded_row: Option<String>,
    /// Live buffer for the target pad-number text input. Cleared on
    /// open / save / cancel.
    pub override_buf: String,
}

/// Component Preview tab state — one per open row.
///
/// Per `v0.9-refactor-2-plan.md` §11: a row is the unit of storage
/// (DBLib model). The preview surface is read-only for Symbol/Footprint;
/// the form-shaped tabs (Parameters / Supply / Datasheet / Simulation)
/// edit `row` in-place and persist via `adapter.update_row(table, row, msg)`.
#[derive(Debug)]
pub struct ComponentPreviewState {
    /// Library this row lives in (absolute `*.snxlib/` directory).
    pub library_path: PathBuf,
    /// Table the row lives in (filename stem; `tables/<table>.tsv` for
    /// LocalGit, `component_rows.table_name = ?` for Database).
    pub table: String,
    /// Mutable working copy of the row. `Save` calls
    /// `adapter.update_row(&table, &row, "edit message")`.
    pub row: ComponentRow,

    // ── Primitive bindings (loaded lazily) ──────────────────────────
    /// Resolved Symbol — `None` until first switch into the Preview
    /// tab or when the primitive ref is missing.
    pub symbol: Option<Symbol>,
    /// Resolved Footprint — `None` when no footprint is bound or the
    /// ref is missing.
    pub footprint: Option<Footprint>,
    /// Resolved SimModel — `None` when the Simulation tab hasn't been
    /// visited yet or no sim is bound.
    pub sim: Option<SimModel>,

    /// Live `text_editor::Content` for the SPICE deck. Mirrors
    /// `state.sim?.body` and is RefCell-backed so it's neither
    /// `Clone` nor `PartialEq` — we keep it alongside the typed
    /// primitive rather than dragging interior mutability into it.
    pub sim_body: Option<iced::widget::text_editor::Content>,

    /// Active preview tab — defaults to Preview.
    pub active_tab: PreviewTab,

    /// Live edit buffers for numeric / measurement inputs on the
    /// Parameters tab. Keyed by parameter name; flushed to
    /// `row.parameters` on Enter / blur / valid-parse. Pattern from
    /// `reference_erasable_numeric_input` — a `text_input` bound
    /// directly to `f64` fights typing.
    pub params_edit_buf: HashMap<String, String>,

    /// Inline pin-map editor state for the Preview tab's pin-map
    /// subsection. Holds expanded_row + override_buf only; the
    /// canonical pin/pad bindings live on `row.pin_map_overrides`.
    pub pin_map_state: PinMapInlineState,

    /// True if any inline form edit has been applied since the last
    /// save. Drives the close-tab dirty prompt.
    pub dirty: bool,
}

impl ComponentPreviewState {
    /// Build a preview state from a freshly-loaded row.
    pub fn from_row(library_path: PathBuf, table: String, row: ComponentRow) -> Self {
        Self {
            library_path,
            table,
            row,
            symbol: None,
            footprint: None,
            sim: None,
            sim_body: None,
            active_tab: PreviewTab::Preview,
            params_edit_buf: HashMap::new(),
            pin_map_state: PinMapInlineState::default(),
            dirty: false,
        }
    }

    #[allow(dead_code)]
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    #[allow(dead_code)]
    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }
}

/// Backwards-compatible alias — other slices still refer to
/// `ComponentEditorState` while their own retarget passes land. Once
/// every consumer (panel / documents / new_component / commands /
/// dispatch) is on `ComponentPreviewState`, this alias goes away.
#[allow(dead_code)]
pub type ComponentEditorState = ComponentPreviewState;

/// Distributor APIs Settings panel state.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct DistributorSettings {
    pub digikey_account_email: Option<String>,
    pub digikey_status: Option<String>,
    pub digikey_in_flight: bool,
    pub digikey_cancel: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    pub mouser_api_key_buf: String,
    pub mouser_status: Option<String>,
    pub mouser_in_flight: bool,
    pub preferred_order: Vec<DistributorSource>,
}

impl Default for DistributorSettings {
    fn default() -> Self {
        Self {
            digikey_account_email: None,
            digikey_status: None,
            digikey_in_flight: false,
            digikey_cancel: None,
            mouser_api_key_buf: String::new(),
            mouser_status: None,
            mouser_in_flight: false,
            preferred_order: vec![
                DistributorSource::DigiKey,
                DistributorSource::Mouser,
                DistributorSource::Lcsc,
                DistributorSource::Jlcpcb,
            ],
        }
    }
}

// `ComponentPreviewState::from_row` is the canonical builder; the legacy
// `from_head` helper that constructed an editor from a `Component` chain
// is gone with the v0.9-refactor-2 DBLib model.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picker_state_defaults_to_empty() {
        let s = PickerState::default();
        assert!(s.filter.is_empty());
        assert!(s.selected.is_none());
    }

    #[test]
    fn distributor_settings_default_order() {
        let s = DistributorSettings::default();
        assert_eq!(s.preferred_order.len(), 4);
        assert_eq!(s.preferred_order[0], DistributorSource::DigiKey);
    }

    #[test]
    fn preview_tab_order_is_five_tabs() {
        assert_eq!(PreviewTab::ORDER[0], PreviewTab::Preview);
        assert_eq!(PreviewTab::ORDER.last(), Some(&PreviewTab::Simulation));
        assert_eq!(PreviewTab::ORDER.len(), 5);
    }

    #[test]
    fn preview_tab_labels_are_short_and_distinct() {
        let labels: std::collections::HashSet<&str> =
            PreviewTab::ORDER.iter().map(|t| t.label()).collect();
        assert_eq!(labels.len(), PreviewTab::ORDER.len());
    }

    #[test]
    fn new_component_state_defaults_to_generic_class() {
        let nc = NewComponentState::default();
        assert!(nc.internal_pn.is_empty());
        assert!(nc.library_idx.is_none());
        // Table starts unset until the user picks one in the modal.
        assert!(nc.table.is_none());
        assert_eq!(nc.class, ComponentClass::generic());
        assert!(nc.category.is_empty());
    }

    #[test]
    fn library_set_mount_unmount_is_symmetric() {
        let mut set = LibrarySet::new();
        assert!(set.is_empty());
        // Mounting requires a real adapter; testing only the bookkeeping
        // shape here. (Full end-to-end test lives in `commands.rs`.)
        let _ = set.unmount(Uuid::nil());
        assert!(set.is_empty());
    }

    /// `OpenLibrary::total_rows` sums every cached table's length —
    /// feeds the panel's library-node `(N)` count.
    #[test]
    fn open_library_total_rows_sums_tables() {
        let mut lib = OpenLibrary {
            root: PathBuf::from("/tmp/x.snxlib"),
            display_name: "X".into(),
            library_id: Uuid::nil(),
            tables: HashMap::new(),
            cached_components: Vec::new(),
            cached_symbols: Vec::new(),
            cached_footprints: Vec::new(),
            cached_sims: Vec::new(),
            display: LibraryDisplaySettings::default(),
        };
        assert_eq!(lib.total_rows(), 0);
        lib.tables.insert("resistors".into(), Vec::new());
        assert_eq!(lib.total_rows(), 0);
        lib.tables.insert(
            "capacitors".into(),
            vec![fixture_row("C1"), fixture_row("C2")],
        );
        lib.tables
            .insert("resistors".into(), vec![fixture_row("R1")]);
        assert_eq!(lib.total_rows(), 3);
    }

    /// Helper — minimal `ComponentRow` for the panel-side cache tests.
    /// The full row schema lives in `signex_library`'s tests.
    fn fixture_row(pn: &str) -> ComponentRow {
        use signex_library::{
            DatasheetRef, InternalPn, LifecycleState, ManufacturerPart, ParamMap, PinPadOverride,
            PlmReserved,
        };
        let _ = (PinPadOverride::new("1", "1"),); // module touch
        ComponentRow {
            row_id: Uuid::new_v4(),
            internal_pn: InternalPn::new(pn),
            class: ComponentClass::generic(),
            datasheet: DatasheetRef::default(),
            state: LifecycleState::Draft,
            symbol_ref: signex_library::PrimitiveRef::new(Uuid::nil(), Uuid::new_v4()),
            footprint_ref: None,
            sim_ref: None,
            pin_map_overrides: Vec::new(),
            primary_mpn: ManufacturerPart::draft("Mfr", "MPN"),
            alternates: Vec::new(),
            supply: Vec::new(),
            parameters: ParamMap::new(),
            plm: PlmReserved::default(),
            created: chrono::Utc::now(),
            updated: chrono::Utc::now(),
            content_hash: [0u8; 32],
        }
    }
}
