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

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use signex_library::{
    Component, ComponentId, ComponentSummary, DatasheetRef, DistributorSource, LibraryAdapter,
    LibraryError, LibraryQuery, LifecycleState, LocalGitAdapter, ParamMap, Revision, SupplierLink,
    UseSite, Version, WhereUsedIndex,
};

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
    /// in `signex-library/src/where_used.rs`. Refresh on project
    /// open/save (Phase 2 wires the ingest path).
    #[allow(dead_code)]
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
}

impl Default for LibraryState {
    fn default() -> Self {
        Self {
            open_libraries: Vec::new(),
            open_editors: HashMap::new(),
            where_used: WhereUsedIndex::new(),
            picker: None,
            settings: DistributorSettings::default(),
            expanded: Vec::new(),
            panel_search: String::new(),
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

/// Component Editor window state — one per editor window.
#[derive(Debug, Clone)]
pub struct ComponentEditorState {
    pub library_root: PathBuf,
    pub component_id: ComponentId,
    /// Internal PN at the time the editor opened. Mirrored to
    /// `draft.shared.mpn`, etc., for inline rename.
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
    /// Editable symbol document — parsed lazily from
    /// `draft.schematic.symbol.sexpr` on editor open. Edits are
    /// serialised back via the `SymbolEdited` message.
    pub symbol_doc: super::editor::symbol::state::SymbolDoc,
    /// Active tool on the Symbol-tab canvas.
    pub symbol_tool: super::editor::symbol::canvas::SymbolTool,
    /// AI-stub PDF preview — populated after a successful PDF pick,
    /// dismissed on Apply / Cancel.
    pub symbol_ai_preview: Option<super::editor::symbol::ai_stub::AiPinoutPreview>,
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
/// Phase 1: held in memory only; Phase 2 persists to disk alongside
/// the rest of `~/.config/signex/prefs.json`.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct DistributorSettings {
    /// Connected DigiKey OAuth account email — `None` when no
    /// keyring entry exists.
    pub digikey_account_email: Option<String>,
    /// Live edit buffer for the Mouser API key text input. Masked at
    /// render time. Empty string = "no key in keyring".
    pub mouser_api_key_buf: String,
    /// User-visible status from the most recent "Test" press.
    pub mouser_status: Option<String>,
    /// Active order-of-preference list. The first matching adapter
    /// is queried first when a user pastes a URL into the Supply
    /// tab. Defaults to the LIBRARY_PLAN matrix.
    pub preferred_order: Vec<DistributorSource>,
}

impl Default for DistributorSettings {
    fn default() -> Self {
        Self {
            digikey_account_email: None,
            mouser_api_key_buf: String::new(),
            mouser_status: None,
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
            .unwrap_or_else(|| draft_starter(component.head));
        let internal_pn = component.internal_pn.as_str().to_string();
        let displayed_version = component.head;
        let symbol_doc = super::editor::symbol::state::SymbolDoc::parse(
            &head.schematic.symbol.sexpr,
            internal_pn.as_str(),
        );
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
            symbol_doc,
            symbol_tool: super::editor::symbol::canvas::SymbolTool::Select,
            symbol_ai_preview: None,
        }
    }

    /// Mutate the draft's `SupplierLink` list — one of the only
    /// list-typed fields the Phase 1 form exposes.
    pub fn supplier_links_mut(&mut self) -> &mut Vec<SupplierLink> {
        &mut self.draft.shared.suppliers
    }

    pub fn parameters_mut(&mut self) -> &mut ParamMap {
        &mut self.draft.shared.parameters
    }

    /// Apply an Overview-tab datasheet edit. Phase 1 only writes
    /// `DatasheetRef::Url` — the hash-pinned variant lands with the
    /// PDF upload flow in Phase 2.
    pub fn set_datasheet_url(&mut self, raw: String) {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            self.draft.shared.datasheet = None;
        } else {
            self.draft.shared.datasheet = Some(DatasheetRef::url(trimmed));
        }
    }
}

/// Internal helper — produce a fresh draft starting at the supplied
/// version. Used as a fallback when a component has no head revision
/// to clone from (which the Phase 1 flow shouldn't hit, but defends
/// the unwrap above).
fn draft_starter(version: Version) -> Revision {
    use signex_library::{PcbSide, SchematicSide, SharedSide};
    Revision {
        version,
        state: LifecycleState::Draft,
        created: chrono::Utc::now(),
        author: String::new(),
        message: String::new(),
        schematic: SchematicSide::default(),
        pcb: PcbSide::default(),
        shared: SharedSide::default(),
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
}

/// Avoid an unused-import warning when the `local-git` feature is off
/// (the adapter import is gated, but `Component` / `Revision` are
/// always pulled in by the editor types). Phase 2 may flip this when
/// the trait widens.
#[allow(dead_code)]
fn _types_used(_c: &Component, _r: &Revision, _u: &UseSite) {}
