//! In-memory state for the Library subsystem.
//!
//! Owned by [`crate::app::Signex::library`]. The main pieces:
//!
//! * `set` — `signex_library::LibrarySet`, the cross-library resolver that
//!   maps `library_id → Box<dyn LibraryAdapter>`. Editors and renderers
//!   hand a `PrimitiveRef` to `set.resolve_*` to load
//!   `Symbol`/`Footprint`/`SimModel` primitives without knowing which library
//!   they live in.
//! * `open_libraries` — display caches per `*.snxlib/`. Each entry holds
//!   the root path, display name, and a cached `Vec<ComponentSummary>` so
//!   the panel doesn't re-scan disk between renders. The actual adapter
//!   lives on `set`, keyed by `library_id`.
//! * `editors` — one entry per Component Editor keyed by
//!   `EditorAddress(library_path, component_id)`. The editor lives
//!   as a tab in the main window's tab bar; the user can detach it
//!   into its own OS window via the existing tab-undock flow, in
//!   which case `Signex::ui_state.windows` registers the new id as
//!   `WindowKind::ComponentEditor` referring to the same address.
//!   (WS-I: tab-not-window — Wave 2 keyed by `iced::window::Id`.)
//! * `picker` — Phase 1 component picker modal state.
//! * `settings` — Distributor-APIs panel local state.
//!
//! WS-F note: this module was simplified during the v0.9 refactor —
//! the legacy `SchematicSide` / `PcbSide` / `SharedSide` + `SpiceModel`
//! UI surfaces compiled against pre-refactor `signex-library`. WS-E
//! owns rebuilding the proper editor state on top of the new
//! `Symbol` / `Footprint` / `Component` (binding) shape; WS-F here
//! threads only the Symbol/Footprint primitive editing surface and
//! stubs every other tab as "WS-E pending".

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use signex_library::{
    ComponentClass, ComponentRow, ComponentSummary, DistributorSource, Footprint, LibraryAdapter,
    LibraryError, LibrarySet, LocalGitAdapter, RowId, SimModel, Symbol, TemplateRegistry, UseSite,
    WhereUsedIndex,
};
use uuid::Uuid;

// WS-5 (DBLib): the v0.9-refactor-2 data model dropped the per-revision
// `Component` / `Revision` / `Version` shapes — components are TSV rows
// now. The Component Editor surface (WS-6 territory) hasn't been
// retargeted yet; until it ships, expose thin type aliases so the
// editor-state struct compiles. WS-6 replaces `ComponentEditorState`
// with a row-shaped `ComponentPreviewState` and these aliases drop.
//
// The aliases are `pub` so other in-flight modules in the WS-6/7/8
// scope can `use crate::library::state::Component` without manually
// rewriting every signature in the same patch — the contract types
// keep flowing while each slice ships its part of the refactor.
#[allow(dead_code)]
pub type Component = ComponentRow;
#[allow(dead_code)]
pub type ComponentId = Uuid;
#[allow(dead_code)]
pub type Revision = ComponentRow;
#[allow(dead_code)]
pub type Version = u32;

// WS-I: tab-not-window
/// Identity for an open Component Editor — the same shape that lives
/// on `TabKind::ComponentEditor` and (when the user undocks) on
/// `WindowKind::ComponentEditor`. Used as the lookup key for
/// [`LibraryState::editors`] and as the address that editor view
/// closures clone into messages.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EditorAddress {
    pub library_path: PathBuf,
    pub component_id: ComponentId,
}

impl EditorAddress {
    pub fn new(library_path: PathBuf, component_id: ComponentId) -> Self {
        Self {
            library_path,
            component_id,
        }
    }

    /// Synthetic on-disk identity for a Component Editor tab — used by
    /// `TabInfo.path` so the tab bar, undock detector, and dirty-paths
    /// machinery have a single unique `PathBuf` per editor without
    /// needing to teach them about a second identity scheme. Mirrors
    /// the `<library>/components/<uuid>.snxprt` storage layout from
    /// the v0.9 plan §3 so the synthetic path lines up with where the
    /// component actually persists on disk.
    pub fn synthetic_tab_path(&self) -> PathBuf {
        self.library_path
            .join("components")
            .join(format!("{}.snxprt", self.component_id))
    }
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
    // WS-I: tab-not-window
    /// Component Editor states currently open. Keyed by
    /// `(library_path, component_id)` so the same editor surface
    /// renders whether the editor is hosted inline in the main
    /// window's tab bar or detached into its own window via the
    /// existing tab-undock flow. The window-id-keyed
    /// `HashMap<window::Id, ComponentEditorState>` from Wave 2 is
    /// gone — Component Editors are tabs first; undocking is a
    /// rendering host swap, not a separate state owner.
    pub editors: HashMap<EditorAddress, ComponentEditorState>,
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
    #[allow(dead_code)]
    pub close_library_confirm: Option<CloseLibraryConfirmState>,
    /// WS-F: in-memory primitive resolver while WS-C's `LibrarySet`
    /// trait + cross-library dependency mounting hasn't merged. Symbol
    /// and Footprint primitives currently load through this map; once
    /// WS-C ships, callers swap the `Box<dyn LibrarySet>` resolver in
    /// without touching the editor surface.
    /// TODO(merge-with-WS-C): replace with `LibrarySet::resolve_*`.
    pub set: LibrarySet,
}

impl Default for LibraryState {
    fn default() -> Self {
        let mut settings = DistributorSettings::default();
        // UI-WS7: rehydrate the preferred-order list from
        // `<config_dir>/signex/distributors.toml`.
        settings.preferred_order = super::settings::persistence::load_preferred_order();
        Self {
            set: LibrarySet::new(),
            open_libraries: Vec::new(),
            // WS-I: tab-not-window
            editors: HashMap::new(),
            where_used: WhereUsedIndex::new(),
            picker: None,
            settings,
            expanded: Vec::new(),
            panel_search: String::new(),
            new_component: None,
            close_library_confirm: None,
            set: LibrarySet::default(),
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

    /// Open the `*.snxlib/` at `root`, mounting the adapter under its
    /// `library_id` on `set` and registering the display entry in
    /// `open_libraries`. Idempotent.
    ///
    /// WS-5 (DBLib): also primes the per-library `tables` cache by
    /// running `list_tables` + `read_table` for every table the
    /// adapter exposes. Read errors warn through `tracing` and the
    /// affected entries are left empty — one bad table doesn't sink
    /// the open flow.
    pub fn open_library(&mut self, root: PathBuf) -> Result<(), LibraryError> {
        if self.library_at(&root).is_some() {
            return Ok(());
        }
        let adapter = LocalGitAdapter::open(&root)?;
        let manifest = adapter.manifest();
        let display_name = manifest.library.name.clone();
        let library_id = manifest.library.library_id;
        self.set.mount(Box::new(adapter));
        let mut entry = OpenLibrary {
            root,
            display_name,
            library_id,
            tables: HashMap::new(),
            cached_components: Vec::new(),
        };
        if let Some(adapter) = self.set.get(library_id) {
            if let Err(e) = entry.reload_tables(adapter) {
                tracing::warn!(
                    target: "signex::library",
                    library_id = %library_id,
                    error = %e,
                    "open_library: reload_tables failed; library opens with empty cache"
                );
            }
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
            // WS-I: tab-not-window
            self.editors.retain(|key, _| key.library_path != root);
        }
    }

    /// Refresh the cached table contents for a library — re-reads every
    /// TSV via the mounted adapter's `list_tables` + `read_table`.
    ///
    /// WS-5 (DBLib): replaces the per-revision `search` call from
    /// v0.9-original. Returns `LibraryError::NotFound` when the
    /// library at `root` isn't mounted, otherwise the underlying
    /// adapter error.
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
        if let Some(lib) = self.library_at_mut(root) {
            lib.tables = tables;
            lib.cached_components = summaries;
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
    /// `refs` — `(row_id, instance_id)` tuples.
    ///
    /// WS-5 (DBLib): `WhereUsedIndex::ingest_sheet` keys by `RowId`
    /// directly now that revisions are gone — the per-instance
    /// version pin from v0.9-original folds away.
    #[allow(dead_code)]
    pub fn ingest_sheet(&mut self, project: &Path, sheet: &Path, refs: &[(Uuid, String, Version)]) {
        let trimmed: Vec<(RowId, String)> = refs
            .iter()
            .map(|(uuid, inst, _ver)| (RowId::from_uuid(*uuid), inst.clone()))
            .collect();
        self.where_used.ingest_sheet(project, sheet, &trimmed);
    }

    /// Look up the use-sites for a component row.
    ///
    /// WS-5 (DBLib): the v0.9-refactor-2 `where_used` index keys by
    /// `RowId` directly — there's no per-revision pin since rows
    /// don't have revisions anymore. The `version` argument is kept
    /// for source-compat with the legacy callers and ignored.
    pub fn where_used_for(&self, uuid: ComponentId, _version: Option<Version>) -> Vec<UseSite> {
        self.where_used.where_used(RowId::from_uuid(uuid))
    }

    // WS-I: tab-not-window
    /// Editor addresses currently pointing at `root` that have unsaved edits.
    #[allow(dead_code)]
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
                .then_with(|| a.component_id.cmp(&b.component_id))
        });
        keys
    }

    /// Existing editor for `(library_root, component_id)`, if any.
    #[allow(dead_code)]
    pub fn editor_for(
        &self,
        library_root: &Path,
        component_id: ComponentId,
    ) -> Option<&ComponentEditorState> {
        self.editors.get(&EditorAddress::new(
            library_root.to_path_buf(),
            component_id,
        ))
    }
}

/// One open `*.snxlib/` directory — display cache only. The owning
/// `LibraryAdapter` lives on [`LibraryState::set`] keyed by
/// `library_id`.
///
/// WS-5 (DBLib): in the v0.9-refactor-2 model components are rows in
/// per-category TSV tables, not standalone files. The display cache
/// is keyed by table name; each entry holds the full row payload so
/// the panel can render a grid view per category without re-reading
/// disk between view ticks. Hot tables can be re-read on edit; v0.9
/// keeps it simple — every row write triggers a full table reload.
pub struct OpenLibrary {
    pub root: PathBuf,
    pub display_name: String,
    pub library_id: Uuid,
    /// Cached table contents — keyed by table filename stem. Populated
    /// on `open_library` by scanning every TSV via `list_tables` +
    /// `read_table`. The Library panel renders a category node per
    /// entry and an inline row grid per category.
    pub tables: HashMap<String, Vec<ComponentRow>>,
    /// Last-loaded summary list. Compatibility shim used by the
    /// `picker.rs` modal until WS-6 retargets it at the row tier;
    /// kept in lock-step with `tables` by `reload_tables`.
    pub cached_components: Vec<ComponentSummary>,
}

impl OpenLibrary {
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
        Ok(())
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
// WS-E: New Component flow
// ─────────────────────────────────────────────────────────────────────

/// Built-in component classes — keep this list in sync with
/// `v0.9-library-refactor-plan.md` §4.1. The string is what gets
/// stored on `Component::class`; the label is what the picker shows.
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
/// inserts a row into the chosen target table.
#[derive(Debug, Clone)]
pub struct NewComponentState {
    /// Live edit buffer for the Internal PN field.
    pub internal_pn: String,
    /// Selected target library — index into `open_libraries`.
    pub library_idx: Option<usize>,
    /// Picked target table (filename stem) — `None` until the user picks
    /// one. WS-8 (DBLib model): rows live inside category tables, so the
    /// New Component modal needs the user to pick a table along with the
    /// library + class. When the manifest carries no `[[tables]]`
    /// overrides we still surface the picker with the default
    /// `<class>s` filename so the user always sees the destination.
    pub table: Option<String>,
    /// Picked component class — defaults to "generic".
    pub class: ComponentClass,
    /// Tree-style category path ("Passives/Resistors/0805"). Free-form
    /// — validation happens at submit time.
    pub category: String,
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
            error: None,
        }
    }
}

/// "Close Library — Unsaved Drafts" confirmation modal state.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct CloseLibraryConfirmState {
    pub library_path: PathBuf,
    pub library_name: String,
    // WS-I: tab-not-window — editors are addressed by
    // `(library_path, component_id)` now that they live as tabs, not
    // OS windows.
    pub dirty_editors: Vec<EditorAddress>,
}

/// WS-F stub for the upcoming WS-C `LibrarySet`. Holds Symbol /
/// Footprint primitives keyed by uuid so the editor can resolve a
/// `PrimitiveRef::uuid` without a real adapter call. Cross-library
/// resolution by `library_id` is a no-op until WS-C ships.
///
/// TODO(merge-with-WS-C): replace this whole struct with
/// `signex_library::adapters::library_set::LibrarySet`.
#[derive(Debug, Default)]
pub struct LibrarySet {
    pub symbols: HashMap<Uuid, Symbol>,
    pub footprints: HashMap<Uuid, Footprint>,
}

impl LibrarySet {
    pub fn resolve_symbol(&self, uuid: Uuid) -> Option<Symbol> {
        self.symbols.get(&uuid).cloned()
    }

    pub fn resolve_footprint(&self, uuid: Uuid) -> Option<Footprint> {
        self.footprints.get(&uuid).cloned()
    }

    pub fn save_symbol(&mut self, sym: Symbol) {
        self.symbols.insert(sym.uuid, sym);
    }

    pub fn save_footprint(&mut self, fp: Footprint) {
        self.footprints.insert(fp.uuid, fp);
    }
}

/// WS-F stub for the upcoming WS-C `LibrarySet`. Holds Symbol /
/// Footprint primitives keyed by uuid so the editor can resolve a
/// `PrimitiveRef::uuid` without a real adapter call. Cross-library
/// resolution by `library_id` is a no-op until WS-C ships.
///
/// TODO(merge-with-WS-C): replace this whole struct with
/// `signex_library::adapters::library_set::LibrarySet`.
#[derive(Debug, Default)]
pub struct LibrarySet {
    pub symbols: HashMap<Uuid, Symbol>,
    pub footprints: HashMap<Uuid, Footprint>,
}

impl LibrarySet {
    pub fn resolve_symbol(&self, uuid: Uuid) -> Option<Symbol> {
        self.symbols.get(&uuid).cloned()
    }

    pub fn resolve_footprint(&self, uuid: Uuid) -> Option<Footprint> {
        self.footprints.get(&uuid).cloned()
    }

    pub fn save_symbol(&mut self, sym: Symbol) {
        self.symbols.insert(sym.uuid, sym);
    }

    pub fn save_footprint(&mut self, fp: Footprint) {
        self.footprints.insert(fp.uuid, fp);
    }
}

/// Component Editor window state — one per editor window.
///
/// WS-F refactor: instead of carrying the legacy
/// `SchematicSide.symbol.sexpr` blob and round-tripping it into a
/// `SymbolDoc`, the editor now holds a typed [`Symbol`] primitive
/// loaded by reference (`Revision::symbol_ref`) and a typed
/// [`Footprint`] primitive (`Revision::footprint_ref`). Save dispatches
/// `LibraryMessage::SaveSymbol` / `SaveFootprint` onto the adapter via
/// the `LibrarySet`.
pub struct ComponentEditorState {
    pub library_root: PathBuf,
    pub component_id: ComponentId,
    /// Internal PN at the time the editor opened.
    pub display_internal_pn: String,
    /// Currently-displayed lifecycle state (header bar) — sourced
    /// from `draft.state` while editing.
    pub displayed_version: Version,
    /// Active editor tab — defaults to Overview.
    pub active_tab: EditorTab,
    /// Mutable working draft. Save Draft writes this via
    /// `adapter.save_revision`; Commit auto-bumps the version.
    pub draft: Revision,
    /// Whole-component view (head + every revision). Refreshed on
    /// open and after every successful Commit. Used by the History
    /// tab and the version dropdown.
    pub component: Component,
    /// Selected revision in the History tab. Defaults to `component.head`.
    pub history_selected: Option<Version>,
    /// Whether the workflow requires reviews — drives the "Submit
    /// for Review" footer button.
    pub review_required: bool,
    /// WS-F: editable [`Symbol`] primitive — loaded via
    /// `LibrarySet::resolve_symbol(rev.symbol_ref.uuid)` on editor
    /// open. Save dispatches `LibraryMessage::SaveSymbol` to the
    /// dispatcher, which routes through the adapter.
    pub symbol: Symbol,
    /// WS-F: editable [`Footprint`] primitive — loaded via
    /// `LibrarySet::resolve_footprint(rev.footprint_ref.uuid)`. `None`
    /// when the binding has no footprint (a chip with symbol-only
    /// representation, e.g. a power port).
    pub footprint: Option<Footprint>,
    /// Active tool on the Symbol-tab canvas.
    pub symbol_tool: super::editor::symbol::canvas::SymbolTool,
    /// Selection on the Symbol canvas — pin / field / etc.
    pub symbol_selected: Option<super::editor::symbol::state::SymbolSelection>,
    /// AI-stub PDF preview — populated after a successful PDF pick,
    /// dismissed on Apply / Cancel.
    pub symbol_ai_preview: Option<super::editor::symbol::ai_stub::AiPinoutPreview>,
    /// Footprint canvas interaction state — kept across redraws.
    pub footprint_state:
        Option<crate::library::editor::footprint::state::FootprintEditorState>,
    /// Cache reused across redraws so `iced::widget::Canvas`'s draw
    /// path only retessellates when the model actually changes.
    /// Held in `OnceLock` because the canvas program needs a
    /// borrowed reference; the lock is initialised once on the
    /// first render and cleared on every model mutation.
    #[allow(dead_code)]
    pub symbol: Option<Symbol>,
    /// Resolved footprint primitive.
    #[allow(dead_code)]
    pub footprint: Option<Footprint>,
    /// Resolved SimModel primitive.
    #[allow(dead_code)]
    pub sim: Option<SimModel>,
    /// Pin Map tab state — placeholder until WS-G fills it in.
    #[allow(dead_code)]
    pub pin_map: PinMapTabState,

    // ── WS-F2: Symbol tab canvas state ──────────────────────────────
    /// Active drawing tool on the Symbol canvas.
    #[allow(dead_code)]
    pub symbol_tool: super::editor::symbol::canvas::SymbolTool,
    /// Currently-selected symbol element (pin index / field key).
    #[allow(dead_code)]
    pub symbol_selected: Option<super::editor::symbol::state::SymbolSelection>,
    /// Live AI-from-datasheet preview — populated after the user picks
    /// a PDF and the heuristic returns a guess. `None` while no preview
    /// is in flight.
    #[allow(dead_code)]
    pub symbol_ai_preview: Option<super::editor::symbol::ai_stub::AiPinoutPreview>,

    // ── WS-F2: Footprint tab canvas state ───────────────────────────
    /// Footprint canvas mirror of the primitive's pad list. Built lazily
    /// the first time the user switches into the Footprint tab; once
    /// populated, mutations are mirrored back onto `editor.footprint`
    /// via `FootprintEditorState::sync_pads_to_primitive`.
    #[allow(dead_code)]
    pub footprint_state: Option<super::editor::footprint::state::FootprintEditorState>,
    /// Iced canvas geometry cache — invalidated by the canvas
    /// program on pan / zoom / mutation. Wrapped in `OnceLock` so the
    /// view tree gets a stable reference without taking `&mut self`
    /// in `view`.
    #[allow(dead_code)]
    pub footprint_canvas_cache: std::sync::OnceLock<iced::widget::canvas::Cache>,

    // ── WS-L: Sim tab ───────────────────────────────────────────────
    /// Live `text_editor::Content` for the SPICE deck. Mirrored from
    /// `editor.sim.body` on tab-switch / sim-load and back into
    /// `editor.sim.body` on every `SimBodyAction`. None when no sim
    /// model is bound.
    ///
    /// `Content` is RefCell-backed so it's neither `Clone` nor
    /// `PartialEq` in the form we need; keeping it as a separate live
    /// UI state alongside `editor.sim` avoids dragging that interior
    /// mutability into the typed primitive.
    #[allow(dead_code)]
    pub sim_body: Option<iced::widget::text_editor::Content>,

    // ── Modal flags ─────────────────────────────────────────────────
    /// True while the SubmitForReview modal is up.
    pub review_dialog_open: bool,
    pub review_notes_buf: String,
    pub review_status: Option<String>,
    pub review_in_flight: bool,
    /// True if any inline form edit has been applied since the last
    /// Save Draft / Commit. Drives the close_library dirty prompt.
    #[allow(dead_code)]
    pub dirty: bool,

    // ── WS-J: Params tab ────────────────────────────────────────────
    /// Live edit buffers for numeric / measurement inputs. Keyed by
    /// parameter name; flushed to `draft.parameters` on commit
    /// (Enter / blur / valid-parse). Follows the
    /// `reference_erasable_numeric_input` pattern: a `text_input`
    /// bound directly to `f64` fights typing because every keystroke
    /// has to re-parse the in-progress text.
    pub params_edit_buf: HashMap<String, String>,
}

impl std::fmt::Debug for ComponentEditorState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ComponentEditorState")
            .field("library_root", &self.library_root)
            .field("component_id", &self.component_id)
            .field("display_internal_pn", &self.display_internal_pn)
            .field("displayed_version", &self.displayed_version)
            .field("active_tab", &self.active_tab)
            .field("symbol_uuid", &self.symbol.uuid)
            .field("footprint_uuid", &self.footprint.as_ref().map(|f| f.uuid))
            .field("dirty", &self.dirty)
            .finish()
    }
}

impl std::fmt::Debug for ComponentEditorState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ComponentEditorState")
            .field("library_root", &self.library_root)
            .field("component_id", &self.component_id)
            .field("display_internal_pn", &self.display_internal_pn)
            .field("displayed_version", &self.displayed_version)
            .field("active_tab", &self.active_tab)
            .field("symbol_uuid", &self.symbol.uuid)
            .field("footprint_uuid", &self.footprint.as_ref().map(|f| f.uuid))
            .field("dirty", &self.dirty)
            .finish()
    }
}

/// Component Editor tabs in display order. Mirrors LIBRARY_PLAN §10.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EditorTab {
    Overview,
    Symbol,
    Footprint,
    PinMap,
    Params,
    Supply,
    Sim,
    History,
    WhereUsed,
}

impl EditorTab {
    pub const ORDER: &'static [EditorTab] = &[
        EditorTab::Overview,
        EditorTab::Symbol,
        EditorTab::Footprint,
        EditorTab::PinMap,
        EditorTab::Params,
        EditorTab::Supply,
        EditorTab::Sim,
        EditorTab::History,
        EditorTab::WhereUsed,
    ];

    pub fn label(self) -> &'static str {
        match self {
            EditorTab::Overview => "Overview",
            EditorTab::Symbol => "Symbol",
            EditorTab::Footprint => "Footprint",
            EditorTab::PinMap => "Pin Map",
            EditorTab::Params => "Params",
            EditorTab::Supply => "Supply",
            EditorTab::Sim => "Sim",
            EditorTab::History => "History",
            EditorTab::WhereUsed => "Where-Used",
        }
    }
}

// ── WS-G: Pin Map ────────────────────────────────────────────────────
/// Per-window UI state for the Pin Map tab. The Pin/Pad bindings
/// themselves live on `Revision::pin_map_overrides`; this struct only
/// holds the inline-editor flags (which row is being overridden, the
/// live edit buffer for the new pad-number text-input).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PinMapTabState {
    /// `Some(pin_number)` while the override editor is expanded for
    /// that specific pin row. `None` when collapsed.
    pub expanded_row: Option<String>,
    /// Live buffer for the target pad-number text input. Cleared on
    /// open / save / cancel.
    pub override_buf: String,
}
// ── /WS-G ────────────────────────────────────────────────────────────

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

impl ComponentEditorState {
    /// Build a fresh editor state from a `ComponentRow`.
    ///
    /// WS-5 (DBLib): the v0.9-original `from_head(component, …)` took
    /// a per-revision `Component` and read its `head_revision()`. With
    /// the row tier, a row IS the editable unit and there's no
    /// `head_revision` to look up. The body of the editor still
    /// expects a working `draft` + `component` pair until WS-6
    /// retargets it at `ComponentPreviewState`; this stub keeps the
    /// type-shape intact (both fields point at the same row) so the
    /// rest of the file compiles. Editor logic is broken — that's
    /// WS-6 territory per the v0.9-refactor-2 plan §11.
    pub fn from_head(library_root: PathBuf, row: ComponentRow, review_required: bool) -> Self {
        let internal_pn = row.internal_pn.as_str().to_string();
        let component_id = row.row_id;
        let displayed_version = 0u32;
        let draft = row.clone();
        Self {
            library_root,
            component_id,
            display_internal_pn: internal_pn,
            displayed_version,
            active_tab: EditorTab::Overview,
            history_selected: None,
            draft,
            component: row,
            review_required,
            symbol: None,
            footprint: None,
            sim: None,
            pin_map: PinMapTabState::default(),
            symbol_tool: super::editor::symbol::canvas::SymbolTool::Select,
            symbol_selected: None,
            symbol_ai_preview: None,
            footprint_state: None,
            footprint_canvas_cache: std::sync::OnceLock::new(),
            // WS-L: Sim tab — seeded lazily on tab switch into Sim.
            sim_body: None,
            review_dialog_open: false,
            review_notes_buf: String::new(),
            review_status: None,
            review_in_flight: false,
            dirty: false,
            // WS-J: Params tab
            params_edit_buf: HashMap::new(),
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

    /// Lazily initialise the Footprint tab's in-memory canvas state
    /// from the current footprint primitive. Idempotent — subsequent
    /// calls are no-ops.
    pub fn ensure_footprint_state(&mut self) {
        if self.footprint_state.is_none() {
            let parsed = match self.footprint.as_ref() {
                Some(fp) => crate::library::editor::footprint::state::FootprintEditorState::from_footprint(fp),
                None => crate::library::editor::footprint::state::FootprintEditorState::empty(),
            };
            self.footprint_state = Some(parsed);
        }
    }

    /// Clear the canvas cache (called after every footprint mutation
    /// so the next draw rebuilds geometry).
    pub fn invalidate_footprint_cache(&mut self) {
        self.footprint_canvas_cache = std::sync::OnceLock::new();
    }
}

/// Internal helper — produce a fresh draft starting at the supplied
/// row. WS-5 (DBLib): the v0.9-refactor-2 model has no per-revision
/// `Revision`, so this is a stub that returns the row itself. WS-6
/// retires this helper when the editor moves to `ComponentPreviewState`.
#[allow(dead_code)]
fn draft_starter(row: &ComponentRow) -> ComponentRow {
    row.clone()
}

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
    fn editor_tab_order_includes_pin_map() {
        assert_eq!(EditorTab::ORDER[0], EditorTab::Overview);
        assert_eq!(EditorTab::ORDER.last(), Some(&EditorTab::WhereUsed));
        assert_eq!(EditorTab::ORDER.len(), 9);
        assert!(EditorTab::ORDER.contains(&EditorTab::PinMap));
    }

    #[test]
    fn editor_tab_labels_are_short_and_distinct() {
        let labels: std::collections::HashSet<&str> =
            EditorTab::ORDER.iter().map(|t| t.label()).collect();
        assert_eq!(labels.len(), EditorTab::ORDER.len());
    }

    #[test]
    fn new_component_state_defaults_to_generic_class() {
        let nc = NewComponentState::default();
        assert!(nc.internal_pn.is_empty());
        assert!(nc.library_idx.is_none());
        // WS-8: table starts unset until the user picks one in the modal.
        assert!(nc.table.is_none());
        assert_eq!(nc.class, ComponentClass::generic());
        assert!(nc.category.is_empty());
    }

        // Empty state → no sites.
        assert!(state.where_used_for(uuid, None).is_empty());

        // Ingest one ref under a project/sheet → one site visible.
        state.ingest_sheet(&project, &sheet, &[(uuid, "U7".into(), v)]);
        let sites = state.where_used_for(uuid, None);
        assert_eq!(sites.len(), 1);
        assert_eq!(sites[0].instance_id, "U7");
        assert_eq!(sites[0].version_pinned, v);
        assert_eq!(sites[0].sheet_path, sheet);

        // Re-ingesting the same sheet with empty refs clears the entry.
        state.ingest_sheet(&project, &sheet, &[]);
        assert!(state.where_used_for(uuid, None).is_empty());
    }

    /// WS-5 (DBLib): `OpenLibrary::total_rows` sums every cached
    /// table's length — feeds the panel's library-node `(N)` count.
    #[test]
    fn open_library_total_rows_sums_tables() {
        let mut lib = OpenLibrary {
            root: PathBuf::from("/tmp/x.snxlib"),
            display_name: "X".into(),
            library_id: Uuid::nil(),
            tables: HashMap::new(),
            cached_components: Vec::new(),
        };
        assert_eq!(lib.total_rows(), 0);
        lib.tables.insert("resistors".into(), Vec::new());
        assert_eq!(lib.total_rows(), 0);
        lib.tables
            .insert("capacitors".into(), vec![fixture_row("C1"), fixture_row("C2")]);
        lib.tables.insert("resistors".into(), vec![fixture_row("R1")]);
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
