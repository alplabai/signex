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
//! * `new_component` — modal state for the "New Component" flow (PN +
//!   library + class + category).
//! * `template_registry` — bundled + per-library parameter templates,
//!   resolved at component-class lookup time.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use signex_library::{
    ComponentClass, ComponentRow, ComponentSummary, DistributorSource, Footprint, LibraryError,
    LibrarySet, LocalGitAdapter, RowId, SimModel, Symbol, TemplateRegistry, UseSite,
    WhereUsedIndex,
};
use uuid::Uuid;

// v0.9-refactor-2: DBLib model — rows live in `tables/<name>.tsv`, addressed
// by `(library_path, table, row_id)`.
/// Identity for an open Component Preview tab — the lookup key for
/// [`LibraryState::editors`] and the address that preview view closures
/// clone into messages. Replaces the `(library_path, component_id)` shape
/// from the original v0.9 refactor.
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
    // v0.9-refactor-2: DBLib model
    /// Component Preview states currently open. Keyed by
    /// `(library_path, table, row_id)` per the DBLib row identity.
    /// Component Preview tabs are read-only for Symbol+Footprint;
    /// editing happens via standalone `.snxsym` / `.snxfpt` document
    /// tabs (see WS-7).
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
    #[allow(dead_code)]
    pub close_library_confirm: Option<CloseLibraryConfirmState>,
    /// Bundled + per-library parameter templates. Reference-counted
    /// because both the editor and the validator borrow it. WS-F/WS-G
    /// surface this through the editor; WS-E only owns the field.
    #[allow(dead_code)]
    pub template_registry: Arc<TemplateRegistry>,
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
            template_registry: Arc::new(TemplateRegistry::new_with_builtins()),
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
        self.set.mount(Box::new(adapter));
        self.open_libraries.push(OpenLibrary {
            root,
            display_name,
            library_id,
            cached_components: Vec::new(),
        });
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

    /// Refresh the cached component list for a library — walks every
    /// row in every table via `LibraryAdapter::iter_rows` and projects
    /// each into a [`ComponentSummary`] for the panel grid.
    pub fn refresh_components(&mut self, root: &Path) -> Result<(), LibraryError> {
        let library_id = self
            .library_at(root)
            .map(|lib| lib.library_id)
            .ok_or_else(|| LibraryError::NotFound(root.display().to_string()))?;
        let rows = self
            .set
            .get(library_id)
            .ok_or_else(|| LibraryError::NotFound(root.display().to_string()))?
            .iter_rows()?;
        let summaries: Vec<ComponentSummary> = rows
            .into_iter()
            .map(|(_table, row)| ComponentSummary {
                row_id: row.row_id,
                internal_pn: row.internal_pn,
                mpn: row.primary_mpn.mpn,
                state: row.state,
                description: String::new(),
            })
            .collect();
        if let Some(lib) = self.library_at_mut(root) {
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
    /// `refs` — `(row_id, instance_id)` tuples (DBLib model).
    #[allow(dead_code)]
    pub fn ingest_sheet(&mut self, project: &Path, sheet: &Path, refs: &[(Uuid, String)]) {
        // The current `WhereUsedIndex::ingest_sheet` shape is owned by
        // signex-library and re-built when WS-9 ships its row-shaped
        // ingest helper. v0.9-refactor-2 leaves this method as a stub
        // so callers compile; the panel/where-used wiring is rebuilt
        // outside this slice.
        let _ = (project, sheet, refs);
    }

    /// Look up the use-sites for a row.
    pub fn where_used_for(&self, row_id: RowId) -> Vec<UseSite> {
        self.where_used.where_used(row_id)
    }

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
pub struct OpenLibrary {
    pub root: PathBuf,
    pub display_name: String,
    pub library_id: Uuid,
    /// Last-loaded summary list. Refreshed on demand; doubles as the
    /// data source for the panel's component list.
    pub cached_components: Vec<ComponentSummary>,
}

impl std::fmt::Debug for OpenLibrary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenLibrary")
            .field("root", &self.root)
            .field("display_name", &self.display_name)
            .field("library_id", &self.library_id)
            .field("cached_components_len", &self.cached_components.len())
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
/// creates a draft revision and opens the Component Editor.
#[derive(Debug, Clone)]
pub struct NewComponentState {
    /// Live edit buffer for the Internal PN field.
    pub internal_pn: String,
    /// Selected target library — index into `open_libraries`.
    pub library_idx: Option<usize>,
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

// ─────────────────────────────────────────────────────────────────────
// Component Preview (v0.9-refactor-2 — DBLib model)
// ─────────────────────────────────────────────────────────────────────

/// Component Preview tabs in display order.
///
/// Per `v0.9-refactor-2-plan.md` §11, the Component view is preview-only:
/// Symbol and Footprint are read-only renders; editing happens via the
/// standalone `.snxsym` / `.snxfpt` document editors (WS-7).
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
}
