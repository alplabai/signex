//! In-memory state for the Library subsystem.
//!
//! Owned by [`crate::app::Signex::library`]. The main pieces:
//!
//! * `open_libraries` — every `*.snxlib/` directory the user has opened
//!   in this session. Keyed by absolute path so tabs / editors can
//!   round-trip via path without re-scanning the disk.
//! * `open_editors` — one entry per Component Editor window.
//!   `iced::window::Id` is the routing key — non-main windows live in
//!   `Signex::ui_state.windows` as `WindowKind::ComponentEditor`.
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

use signex_library::{
    Component, ComponentId, ComponentSummary, DistributorSource, Footprint, LibraryAdapter,
    LibraryError, LibraryQuery, LocalGitAdapter, Revision, Symbol, UseSite, Version, WhereUsedIndex,
};
use uuid::Uuid;

/// Top-level Library subsystem state. Stored on
/// [`crate::app::Signex`] as a single field so the dispatcher can
/// borrow it independently of the rest of `DocumentState`.
pub struct LibraryState {
    /// Open `*.snxlib/` directories — keyed by absolute path. Each
    /// entry owns a `LocalGitAdapter` (other adapters land in v0.9.x).
    pub open_libraries: Vec<OpenLibrary>,
    /// Component Editor windows currently open. Multi-window-aware:
    /// each entry maps an `iced::window::Id` to the live editor
    /// state. The non-main window itself is registered in
    /// `Signex::ui_state.windows` so `view(id)` knows what to render.
    pub open_editors: HashMap<iced::window::Id, ComponentEditorState>,
    /// Reverse "where-used" index. Single-thread per the L3 invariant
    /// in `signex-library/src/where_used.rs`. Populated incrementally
    /// via [`LibraryState::ingest_sheet`] whenever a sheet opens or
    /// saves; the editor's Where-Used tab reads it via `where_used`.
    /// TODO(v0.9-phase-3): wire signex-engine sheet-load events into
    /// `ingest_sheet` so the index updates without explicit calls.
    pub where_used: WhereUsedIndex,
    /// Picker modal state — `None` while the modal is closed.
    pub picker: Option<PickerState>,
    /// Distributor APIs settings panel state.
    pub settings: DistributorSettings,
    /// True while the Library left-dock panel's expanded library node
    /// at index `i` is open. Independent of `open_libraries.len()`.
    pub expanded: Vec<bool>,
    /// Library left-dock search box buffer.
    pub panel_search: String,
    /// "New Component" modal state — `None` while closed.
    #[allow(dead_code)]
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
        // `<config_dir>/signex/distributors.toml`. The persistence
        // layer falls back to the LIBRARY_PLAN default when the file
        // is absent / corrupt, so this is always safe.
        settings.preferred_order =
            super::settings::persistence::load_preferred_order();
        Self {
            open_libraries: Vec::new(),
            open_editors: HashMap::new(),
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
    /// Look up an open library by its on-disk root path. Linear scan
    /// because `open_libraries` is small (single-digit count in
    /// practice). Returns `None` if `path` isn't currently open.
    pub fn library_at(&self, path: &Path) -> Option<&OpenLibrary> {
        self.open_libraries.iter().find(|lib| lib.root == path)
    }

    pub fn library_at_mut(&mut self, path: &Path) -> Option<&mut OpenLibrary> {
        self.open_libraries.iter_mut().find(|lib| lib.root == path)
    }

    /// Open the `*.snxlib/` at `root`, registering it in
    /// `open_libraries`. Idempotent — re-opening an already-open
    /// library returns Ok(()) without re-creating the adapter.
    pub fn open_library(&mut self, root: PathBuf) -> Result<(), LibraryError> {
        if self.library_at(&root).is_some() {
            return Ok(());
        }
        let adapter = LocalGitAdapter::open(&root)?;
        let display_name = adapter.manifest().library.name.clone();
        self.open_libraries.push(OpenLibrary {
            root,
            display_name,
            adapter: Box::new(adapter),
            cached_components: Vec::new(),
        });
        self.expanded.push(true);
        // Component summaries are loaded on demand to stay snappy on
        // first-open; the panel's `refresh_components` populates
        // `cached_components` once the user expands the node.
        Ok(())
    }

    /// Drop the library backing `root`. Closes every editor window
    /// that pointed at it (Phase 1: the editor surface only stores
    /// the path; Phase 2 surfaces an unsaved-edits prompt).
    pub fn close_library(&mut self, root: &Path) {
        if let Some(idx) = self.open_libraries.iter().position(|lib| lib.root == root) {
            self.open_libraries.remove(idx);
            if idx < self.expanded.len() {
                self.expanded.remove(idx);
            }
            // TODO(v0.9-phase-2): prompt before dropping editors with dirty drafts.
            self.open_editors.retain(|_, st| st.library_root != root);
        }
    }

    /// Refresh the cached component list for a library — runs the
    /// adapter's `search` with an empty query.
    pub fn refresh_components(&mut self, root: &Path) -> Result<(), LibraryError> {
        let lib = self
            .library_at_mut(root)
            .ok_or_else(|| LibraryError::NotFound(root.display().to_string()))?;
        lib.cached_components = lib.adapter.search(&LibraryQuery::default())?;
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
    ///
    /// Thin pass-through to [`WhereUsedIndex::ingest_sheet`]. Phase 1
    /// callers are tests + the future sheet-load flow; Phase 3 wires
    /// signex-engine open/save events directly so the index is live.
    ///
    /// TODO(v0.9-phase-3): wire signex-engine sheet-load events into
    /// `ingest_sheet` so callers don't have to invoke this manually.
    #[allow(dead_code)]
    pub fn ingest_sheet(&mut self, project: &Path, sheet: &Path, refs: &[(Uuid, String, Version)]) {
        self.where_used.ingest_sheet(project, sheet, refs);
    }

    /// Look up the use-sites for a component. Thin pass-through to
    /// [`WhereUsedIndex::where_used`] — kept here so the editor view
    /// only depends on `LibraryState` (not on `signex_library`'s
    /// `WhereUsedIndex` directly).
    pub fn where_used_for(&self, uuid: ComponentId, version: Option<Version>) -> Vec<UseSite> {
        self.where_used.where_used(uuid, version)
    }

    /// Editor windows currently pointing at `root` that have unsaved
    /// edits. Used by the close-library dirty prompt.
    #[allow(dead_code)]
    pub fn dirty_editors_for_library(&self, root: &Path) -> Vec<iced::window::Id> {
        let mut ids: Vec<iced::window::Id> = self
            .open_editors
            .iter()
            .filter(|(_, st)| st.library_root == root && st.dirty)
            .map(|(id, _)| *id)
            .collect();
        ids.sort();
        ids
    }

    /// Existing editor window for `(library_root, component_id)`, if
    /// any. Caller can `gain_focus(id)` instead of opening a duplicate.
    #[allow(dead_code)]
    pub fn editor_for(
        &self,
        library_root: &Path,
        component_id: ComponentId,
    ) -> Option<iced::window::Id> {
        self.open_editors.iter().find_map(|(id, st)| {
            (st.library_root == library_root && st.component_id == component_id).then_some(*id)
        })
    }
}

/// One open `*.snxlib/` directory.
pub struct OpenLibrary {
    pub root: PathBuf,
    pub display_name: String,
    /// The adapter is `Box<dyn LibraryAdapter>` so we can swap in
    /// `DatabaseAdapter` (v0.9.1) without changing the field type.
    pub adapter: Box<dyn LibraryAdapter>,
    /// Last-loaded summary list. Refreshed on demand; doubles as the
    /// data source for the panel's component list.
    pub cached_components: Vec<ComponentSummary>,
}

impl std::fmt::Debug for OpenLibrary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenLibrary")
            .field("root", &self.root)
            .field("display_name", &self.display_name)
            .field("cached_components_len", &self.cached_components.len())
            .finish()
    }
}

/// Picker modal state — Phase 1 only the shape needed to filter and
/// place. Lifecycle-state filter / category facets land in Phase 2.
#[derive(Debug, Clone, Default)]
pub struct PickerState {
    pub filter: String,
    /// Currently-selected component (path + summary). `None` until
    /// the user clicks a row.
    pub selected: Option<(PathBuf, ComponentSummary)>,
}

/// "New Component" modal state — collected before the dispatcher
/// creates a draft revision and opens the Component Editor.
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct NewComponentState {
    /// Live edit buffer for the Internal PN field.
    pub internal_pn: String,
    /// Selected target library — index into `open_libraries`.
    pub library_idx: Option<usize>,
    /// Latest validation error.
    pub error: Option<String>,
}

/// "Close Library — Unsaved Drafts" confirmation modal state.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct CloseLibraryConfirmState {
    pub library_path: PathBuf,
    pub library_name: String,
    pub dirty_editors: Vec<iced::window::Id>,
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
    /// Mutable working draft. Save Draft writes this to
    /// `parts/.draft/<uuid>.snxpart`; Commit auto-bumps the version
    /// and runs `save_revision`.
    pub draft: Revision,
    /// Whole-component view (head + every revision). Refreshed on
    /// open and after every successful Commit. Used by the History
    /// tab and the version dropdown.
    pub component: Component,
    /// Selected revision in the History tab — drives the diff
    /// preview card. Defaults to `component.head`.
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
    pub footprint_canvas_cache: std::sync::OnceLock<iced::widget::canvas::Cache>,
    /// True while the SubmitForReview modal is up. Switched on by
    /// the footer button and off by Cancel / successful submit.
    pub review_dialog_open: bool,
    /// Free-form reviewer-notes buffer; used as the commit message
    /// when the user clicks Submit. Persists across re-renders for
    /// the lifetime of the editor.
    pub review_notes_buf: String,
    /// Status-line text shown in the modal footer. Used by the
    /// dispatcher to surface async failures back to the UI.
    pub review_status: Option<String>,
    /// True while the SubmitForReview save_revision call is in
    /// flight. Disables the Submit button to avoid double-submits.
    pub review_in_flight: bool,
    /// True if any inline form edit has been applied since the last
    /// Save Draft / Commit. Drives the close_library dirty prompt.
    #[allow(dead_code)]
    pub dirty: bool,
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
    ThreeD,
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
        EditorTab::ThreeD,
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
            EditorTab::ThreeD => "3D",
            EditorTab::Params => "Params",
            EditorTab::Supply => "Supply",
            EditorTab::Sim => "Sim",
            EditorTab::History => "History",
            EditorTab::WhereUsed => "Where-Used",
        }
    }
}

/// Distributor APIs Settings panel state.
///
/// UI-WS7: persisted across sessions via
/// `<config_dir>/signex/distributors.toml` for the order-of-preference
/// list. The DigiKey refresh-token + Mouser API key live in the OS
/// keyring (handled by `signex-library`), not on this struct.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct DistributorSettings {
    /// Connected DigiKey OAuth account label — `None` until the
    /// OAuth handshake succeeds. The label is best-effort (DigiKey's
    /// token endpoint doesn't return identity claims) so it usually
    /// reads "DigiKey" rather than an email.
    pub digikey_account_email: Option<String>,
    /// User-visible status string. Drives the OAuth status line:
    /// "Not connected" → "Waiting for browser…" → "Connected as <x>"
    /// or "Failed: <reason>".
    pub digikey_status: Option<String>,
    /// True while the OAuth flow is mid-handshake. Disables the
    /// Connect button + reveals the Cancel button.
    pub digikey_in_flight: bool,
    /// Cancel handle for the in-flight OAuth flow. Held here so the
    /// Cancel button can dispatch a cancel from the UI thread.
    pub digikey_cancel:
        Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    /// Live edit buffer for the Mouser API key text input. Masked at
    /// render time. Empty string = "no key in keyring".
    pub mouser_api_key_buf: String,
    /// User-visible status from the most recent "Test" press.
    pub mouser_status: Option<String>,
    /// True while the Mouser Test request is in flight.
    pub mouser_in_flight: bool,
    /// Active order-of-preference list. The first matching adapter
    /// is queried first when a user pastes a URL into the Supply
    /// tab. Defaults to the LIBRARY_PLAN matrix; loaded from
    /// `distributors.toml` on startup.
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
    /// WS-F: resolves `symbol_ref` / `footprint_ref` against
    /// `library_set`. Falls back to empty primitives when the resolver
    /// has no entry yet — matches the New Component flow's "draft a
    /// fresh part" path until WS-C/E land the full create wiring.
    pub fn from_head(
        library_root: PathBuf,
        component: Component,
        review_required: bool,
        library_set: &LibrarySet,
    ) -> Self {
        let head = component
            .head_revision()
            .cloned()
            .unwrap_or_else(|| draft_starter(component.head));
        let internal_pn = component.internal_pn.as_str().to_string();
        let displayed_version = component.head;

        // Resolve primitives. Empty fallbacks let a brand-new part open
        // before WS-C's adapter primitive CRUD ships.
        let symbol = library_set
            .resolve_symbol(head.symbol_ref.uuid)
            .unwrap_or_else(|| Symbol::empty(internal_pn.clone()));
        let footprint = head
            .footprint_ref
            .as_ref()
            .and_then(|r| library_set.resolve_footprint(r.uuid));

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
            symbol,
            footprint,
            symbol_tool: super::editor::symbol::canvas::SymbolTool::Select,
            symbol_selected: None,
            symbol_ai_preview: None,
            footprint_state: None,
            footprint_canvas_cache: std::sync::OnceLock::new(),
            review_dialog_open: false,
            review_notes_buf: String::new(),
            review_status: None,
            review_in_flight: false,
            dirty: false,
        }
    }

    /// Mark the editor as having unsaved changes. Called from any
    /// inline form edit. `Save Draft` / `Commit` clear the flag.
    #[allow(dead_code)]
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Clear the dirty flag — called from `save_draft` / `commit`.
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
/// version. Used as a fallback when a component has no head revision
/// to clone from (which the Phase 1 flow shouldn't hit, but defends
/// the unwrap above).
fn draft_starter(version: Version) -> Revision {
    use signex_library::{
        DatasheetRef, LifecycleState, ManufacturerPart, ParamMap, PinPadOverride, PlmReserved,
        PrimitiveRef,
    };
    Revision {
        version,
        state: LifecycleState::Draft,
        created: chrono::Utc::now(),
        author: String::new(),
        message: String::new(),
        symbol_ref: PrimitiveRef::new(Uuid::nil(), Uuid::nil()),
        footprint_ref: None,
        sim_ref: None,
        pin_map_overrides: Vec::<PinPadOverride>::new(),
        primary_mpn: ManufacturerPart::draft("", ""),
        alternates: Vec::new(),
        supply: Vec::new(),
        datasheet: DatasheetRef::default(),
        parameters: ParamMap::new(),
        plm: PlmReserved::default(),
        content_hash: [0u8; 32],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_library::{LibraryMeta, Manifest};

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
    fn open_library_smoke_then_close() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("MyLib.snxlib");
        let manifest = Manifest {
            library: LibraryMeta {
                name: "MyLib".into(),
                library_id: uuid::Uuid::now_v7(),
                description: None,
            },
            mode: signex_library::LibraryMode::default(),
            workflow: signex_library::WorkflowConfig::default(),
            users: signex_library::UsersConfig::default(),
        };
        // Init the library on disk.
        let _ = LocalGitAdapter::init(&root, manifest).expect("init snxlib");

        let mut state = LibraryState::default();
        state.open_library(root.clone()).expect("open");
        assert_eq!(state.open_libraries.len(), 1);
        // Empty library — search returns 0 components.
        state.refresh_components(&root).expect("refresh");
        assert_eq!(state.open_libraries[0].cached_components.len(), 0);
        // Closing drops the entry.
        state.close_library(&root);
        assert!(state.open_libraries.is_empty());
        assert!(state.expanded.is_empty());
    }

    #[test]
    fn editor_tab_order_starts_with_overview() {
        assert_eq!(EditorTab::ORDER[0], EditorTab::Overview);
        assert_eq!(EditorTab::ORDER.last(), Some(&EditorTab::WhereUsed));
        assert_eq!(EditorTab::ORDER.len(), 9);
    }

    #[test]
    fn editor_tab_labels_are_short_and_distinct() {
        let labels: std::collections::HashSet<&str> =
            EditorTab::ORDER.iter().map(|t| t.label()).collect();
        assert_eq!(labels.len(), EditorTab::ORDER.len());
    }

    #[test]
    fn ingest_sheet_round_trips_through_state_to_where_used_index() {
        let mut state = LibraryState::default();
        let project = PathBuf::from("/tmp/sample.snxprj");
        let sheet = PathBuf::from("/tmp/sample.snxprj/main.snxsch");
        let uuid = Uuid::now_v7();
        let v = Version::new(1, 2);

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
}

/// Avoid an unused-import warning when the `local-git` feature is off
/// (the adapter import is gated, but `Component` / `Revision` are
/// always pulled in by the editor types). Phase 2 may flip this when
/// the trait widens.
#[allow(dead_code)]
fn _types_used(_c: &Component, _r: &Revision) {}
