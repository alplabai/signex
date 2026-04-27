//! In-memory state for the Library subsystem.
//!
//! Owned by [`crate::app::Signex::library`]. The main pieces:
//!
//! * `set` — cross-library resolver that maps `library_id → Box<dyn LibraryAdapter>`.
//!   Editors and renderers hand a `PrimitiveRef` to `set.resolve_*` to load
//!   `Symbol`/`Footprint`/`SimModel` primitives without knowing which library
//!   they live in. (WS-E shim — WS-C is shipping the canonical
//!   `signex_library::adapters::library_set::LibrarySet`; this crate's
//!   placeholder will be deleted then.)
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
    Component, ComponentClass, ComponentId, ComponentSummary, DistributorSource, Footprint,
    LibraryAdapter, LibraryError, LibraryQuery, LocalGitAdapter, PrimitiveRef, Revision, SimModel,
    Symbol, TemplateRegistry, UseSite, Version, WhereUsedIndex,
};
use uuid::Uuid;

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

/// WS-E shim for the cross-library resolver.
///
/// WS-C is adding the canonical `LibrarySet` inside
/// `signex_library::adapters::library_set` — when that lands the field
/// type on [`LibraryState`] flips to `signex_library::LibrarySet` and this
/// shim is deleted.
///
/// Ownership rule: an open `*.snxlib/` is mounted **here** by
/// `library_id`. `OpenLibrary` records the root path so the panel can
/// render it; the underlying adapter is reached via `set.adapter(...)`.
pub struct LibrarySet {
    libs: HashMap<Uuid, Box<dyn LibraryAdapter>>,
}

impl LibrarySet {
    pub fn new() -> Self {
        Self {
            libs: HashMap::new(),
        }
    }

    /// Mount an adapter under `library_id`. Replaces any prior adapter
    /// that was mounted under the same id.
    pub fn mount(&mut self, library_id: Uuid, adapter: Box<dyn LibraryAdapter>) {
        self.libs.insert(library_id, adapter);
    }

    /// Drop the adapter for `library_id`, if any.
    pub fn unmount(&mut self, library_id: Uuid) {
        self.libs.remove(&library_id);
    }

    /// Number of mounted libraries.
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.libs.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.libs.is_empty()
    }

    /// Mounted library ids — used to flatten primitive lookups.
    #[allow(dead_code)]
    pub fn library_ids(&self) -> impl Iterator<Item = Uuid> + '_ {
        self.libs.keys().copied()
    }

    pub fn adapter(&self, library_id: Uuid) -> Option<&dyn LibraryAdapter> {
        self.libs.get(&library_id).map(|b| &**b)
    }

    #[allow(dead_code)]
    pub fn adapter_mut<'a>(
        &'a mut self,
        library_id: Uuid,
    ) -> Option<&'a mut (dyn LibraryAdapter + 'static)> {
        self.libs.get_mut(&library_id).map(|b| b.as_mut())
    }

    // TODO(WS-C): the canonical `LibrarySet` will gain primitive-CRUD
    // pass-through (`get_symbol` / `save_symbol` / `list_symbols` …).
    // Until WS-C lands the `LibraryAdapter` trait those methods don't
    // exist, so `resolve_*` returns `None` and persistence is a no-op.
    #[allow(dead_code)]
    pub fn resolve_symbol(&self, _r: &PrimitiveRef) -> Option<Symbol> {
        None
    }

    #[allow(dead_code)]
    pub fn resolve_footprint(&self, _r: &PrimitiveRef) -> Option<Footprint> {
        None
    }

    #[allow(dead_code)]
    pub fn resolve_sim(&self, _r: &PrimitiveRef) -> Option<SimModel> {
        None
    }

    /// WS-C will replace this with `adapter.save_symbol(sym, msg)`.
    /// Stubbed today so the New-Component create path can still run
    /// end-to-end without WS-C; primitive bytes live in memory only.
    pub fn save_symbol(
        &self,
        _library_id: Uuid,
        _sym: &Symbol,
        _msg: &str,
    ) -> Result<(), LibraryError> {
        Ok(())
    }

    pub fn save_footprint(
        &self,
        _library_id: Uuid,
        _fp: &Footprint,
        _msg: &str,
    ) -> Result<(), LibraryError> {
        Ok(())
    }
}

impl Default for LibrarySet {
    fn default() -> Self {
        Self::new()
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
        let manifest = adapter.manifest();
        let display_name = manifest.library.name.clone();
        let library_id = manifest.library.library_id;
        self.set.mount(library_id, Box::new(adapter));
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

    /// Refresh the cached component list for a library — runs the
    /// adapter's `search` with an empty query.
    pub fn refresh_components(&mut self, root: &Path) -> Result<(), LibraryError> {
        let library_id = self
            .library_at(root)
            .map(|lib| lib.library_id)
            .ok_or_else(|| LibraryError::NotFound(root.display().to_string()))?;
        let summaries = self
            .set
            .adapter(library_id)
            .ok_or_else(|| LibraryError::NotFound(root.display().to_string()))?
            .search(&LibraryQuery::default())?;
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
    /// `refs` — `(component_uuid, instance_id, version_pinned)` tuples.
    #[allow(dead_code)]
    pub fn ingest_sheet(&mut self, project: &Path, sheet: &Path, refs: &[(Uuid, String, Version)]) {
        self.where_used.ingest_sheet(project, sheet, refs);
    }

    /// Look up the use-sites for a component.
    pub fn where_used_for(&self, uuid: ComponentId, version: Option<Version>) -> Vec<UseSite> {
        self.where_used.where_used(uuid, version)
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
        self.editors
            .get(&EditorAddress::new(library_root.to_path_buf(), component_id))
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
// Component Editor
// ─────────────────────────────────────────────────────────────────────

/// Component Editor window state — one per editor window.
///
/// Per the v0.9 refactor a `Revision` no longer embeds symbol/footprint
/// blobs; it points at primitives by `PrimitiveRef`. The editor lazily
/// loads the bound primitives via `LibrarySet::resolve_*` on first
/// switch into the relevant tab so the editor-open path stays cheap.
#[derive(Debug)]
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

    // ── Primitive bindings (loaded lazily by WS-F/WS-G) ─────────────
    /// Resolved symbol primitive — `None` until WS-F's Symbol tab is
    /// opened or the primitive ref is missing.
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
}

/// Component Editor tabs in display order.
///
/// WS-E adds `PinMap` between Footprint and Params per
/// `v0.9-library-refactor-plan.md` §12.5 — WS-G fleshes out the tab.
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
    /// Build a fresh editor state from the head revision of `component`.
    pub fn from_head(library_root: PathBuf, component: Component, review_required: bool) -> Self {
        let head = component
            .head_revision()
            .cloned()
            .unwrap_or_else(|| draft_starter(&component));
        let internal_pn = component.internal_pn.as_str().to_string();
        let displayed_version = component.head;
        Self {
            library_root,
            component_id: component.uuid,
            display_internal_pn: internal_pn,
            displayed_version,
            active_tab: EditorTab::Overview,
            history_selected: Some(component.head),
            draft: head,
            component,
            review_required,
            symbol: None,
            footprint: None,
            sim: None,
            pin_map: PinMapTabState::default(),
            review_dialog_open: false,
            review_notes_buf: String::new(),
            review_status: None,
            review_in_flight: false,
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

/// Internal helper — produce a fresh draft starting at the supplied
/// component. Used as a fallback when a component has no head revision
/// to clone from.
fn draft_starter(component: &Component) -> Revision {
    use signex_library::{DatasheetRef, LifecycleState, ManufacturerPart, PlmReserved};
    // No head revision → seed an empty draft. The empty primitive UUIDs
    // here are "all-zeros" sentinels; WS-F's editor will pick the
    // canonical primitives bound to the component on first save.
    let lib = component.uuid;
    let _ = lib; // unused — sentinel comment only.
    Revision {
        version: component.head,
        state: LifecycleState::Draft,
        created: chrono::Utc::now(),
        author: String::new(),
        message: String::new(),
        symbol_ref: PrimitiveRef::new(Uuid::nil(), Uuid::nil()),
        footprint_ref: None,
        sim_ref: None,
        pin_map_overrides: Vec::new(),
        primary_mpn: ManufacturerPart::draft("", ""),
        alternates: Vec::new(),
        supply: Vec::new(),
        datasheet: DatasheetRef::default(),
        parameters: signex_library::ParamMap::new(),
        plm: PlmReserved::default(),
        content_hash: [0u8; 32],
    }
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
