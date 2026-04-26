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
#[derive(Debug)]
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
    /// UI sidecar for the most recently uploaded 3D model — filename,
    /// hash, byte size. The canonical [`signex_library::ModelRef`] on
    /// `draft.pcb.model_3d` only carries (path, offset, rotation), so
    /// this struct keeps the human metadata for the placeholder card
    /// without forcing a re-read off disk on every draw. Cleared when
    /// the user removes the model.
    pub three_d_upload_info: Option<crate::library::editor::three_d::Model3dUploadInfo>,
    /// Sim tab editor state — owns the multi-line `text_editor::Content`
    /// for the SPICE body plus the cached pin-number list. Stays in
    /// sync with `draft.shared.simulation` via the `EditorMsg::Sim*`
    /// dispatcher arms.
    pub sim: super::editor::sim::SimTabState,
    /// Footprint tab live editor state — lazily parsed from
    /// `draft.pcb.footprint.sexpr` on first switch to the Footprint
    /// tab. `None` until then so the parse cost stays out of the
    /// editor-open critical path.
    pub footprint_state:
        Option<crate::library::editor::footprint::state::FootprintEditorState>,
    /// Cache reused across redraws so `iced::widget::Canvas`'s draw
    /// path only retessellates when the model actually changes.
    /// Held in `OnceLock` because the canvas program needs a
    /// borrowed reference; the lock is initialised once on the
    /// first render and cleared on every model mutation.
    #[allow(dead_code)]
    pub footprint_canvas_cache: std::sync::OnceLock<iced::widget::canvas::Cache>,
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
        let sim = super::editor::sim::SimTabState::from_model(
            head.shared.simulation.as_ref(),
            &head.schematic.symbol.sexpr,
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
            three_d_upload_info: None,
            sim,
            footprint_state: None,
            footprint_canvas_cache: std::sync::OnceLock::new(),
        }
    }

    /// Lazily initialise the Footprint tab's in-memory state from
    /// `draft.pcb.footprint.sexpr`. Idempotent — subsequent calls are
    /// no-ops. The dispatcher calls this before any Footprint*
    /// message handler runs so the rest of the dispatch logic can
    /// assume `footprint_state` is `Some`.
    pub fn ensure_footprint_state(&mut self) {
        if self.footprint_state.is_none() {
            let parsed = crate::library::editor::footprint::state::FootprintEditorState::from_sexpr(
                &self.draft.pcb.footprint.sexpr,
            );
            self.footprint_state = Some(parsed);
        }
    }

    /// Re-emit the in-memory footprint state into `draft.pcb.footprint.sexpr`.
    /// Called after every Footprint mutation so Save Draft / Commit
    /// pick up the latest pad layout. The render cache is cleared so
    /// the next draw rebuilds the geometry against the fresh model.
    pub fn flush_footprint_to_draft(&mut self) {
        if let Some(fp) = &self.footprint_state {
            self.draft.pcb.footprint.sexpr = fp.to_sexpr();
            // Cache invalidates on mutation by replacing the OnceLock —
            // cheaper than reaching for interior mutability through
            // the lock.
            self.footprint_canvas_cache = std::sync::OnceLock::new();
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

    /// Set the datasheet to a hash-pinned PDF. Used by the WS3 upload
    /// flow in `dispatch::library`.
    pub fn set_datasheet_pinned(&mut self, hash: String, filename: String) {
        self.draft.shared.datasheet = Some(DatasheetRef::HashPinned { hash, filename });
    }

    /// Switch the datasheet "mode" — preserves nothing across the
    /// switch (the previous variant's payload is dropped). Phase 2 may
    /// add per-mode buffers if reviewers ask.
    pub fn set_datasheet_mode(&mut self, mode: crate::library::editor::datasheet_picker::DatasheetMode) {
        use crate::library::editor::datasheet_picker::DatasheetMode;
        match mode {
            DatasheetMode::Url => match self.draft.shared.datasheet.as_ref() {
                Some(DatasheetRef::Url { .. }) => { /* no-op */ }
                _ => {
                    // Drop the pinned variant; user must paste a URL.
                    self.draft.shared.datasheet = None;
                }
            },
            DatasheetMode::PinnedPdf => match self.draft.shared.datasheet.as_ref() {
                Some(DatasheetRef::HashPinned { .. }) => { /* no-op */ }
                _ => {
                    // Drop the URL; user must upload a PDF.
                    self.draft.shared.datasheet = None;
                }
            },
        }
    }

    /// Set or clear the 3D model alongside its UI sidecar info. Pass
    /// `None` to remove the model.
    pub fn set_model_3d(
        &mut self,
        model: Option<(
            signex_library::ModelRef,
            crate::library::editor::three_d::Model3dUploadInfo,
        )>,
    ) {
        match model {
            Some((m, info)) => {
                self.draft.pcb.model_3d = Some(m);
                self.three_d_upload_info = Some(info);
            }
            None => {
                self.draft.pcb.model_3d = None;
                self.three_d_upload_info = None;
            }
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

    /// Construct a minimal editor state for the WS3 mutation tests.
    fn fresh_editor_state() -> ComponentEditorState {
        use signex_library::{
            Component, InternalPn, PcbSide, SchematicSide, SharedSide, Version,
        };
        let head = Revision {
            version: Version::new(0, 1),
            state: LifecycleState::Draft,
            created: chrono::Utc::now(),
            author: String::new(),
            message: String::new(),
            schematic: SchematicSide::default(),
            pcb: PcbSide::default(),
            shared: SharedSide::default(),
            content_hash: [0u8; 32],
        };
        let component = Component {
            uuid: uuid::Uuid::now_v7(),
            internal_pn: InternalPn::new("TEST_PN"),
            head: head.version,
            revisions: vec![head.clone()],
        };
        ComponentEditorState::from_head(PathBuf::from("MyLib.snxlib"), component, false)
    }

    #[test]
    fn set_datasheet_pinned_replaces_url_variant() {
        let mut state = fresh_editor_state();
        state.set_datasheet_url("https://example.com/d.pdf".into());
        state.set_datasheet_pinned("deadbeef".into(), "TLP281.pdf".into());
        match state.draft.shared.datasheet.as_ref() {
            Some(DatasheetRef::HashPinned { hash, filename }) => {
                assert_eq!(hash, "deadbeef");
                assert_eq!(filename, "TLP281.pdf");
            }
            other => panic!("expected HashPinned, got {other:?}"),
        }
    }

    #[test]
    fn set_datasheet_mode_url_drops_pinned() {
        use crate::library::editor::datasheet_picker::DatasheetMode;
        let mut state = fresh_editor_state();
        state.set_datasheet_pinned("abc".into(), "x.pdf".into());
        state.set_datasheet_mode(DatasheetMode::Url);
        // Switching to URL with no URL-buffer drops the prior datasheet
        // entirely — user must paste a fresh URL.
        assert!(state.draft.shared.datasheet.is_none());
    }

    #[test]
    fn set_datasheet_mode_pinned_drops_url() {
        use crate::library::editor::datasheet_picker::DatasheetMode;
        let mut state = fresh_editor_state();
        state.set_datasheet_url("https://x.test/d.pdf".into());
        state.set_datasheet_mode(DatasheetMode::PinnedPdf);
        assert!(state.draft.shared.datasheet.is_none());
    }

    #[test]
    fn set_datasheet_mode_idempotent_when_variant_matches() {
        use crate::library::editor::datasheet_picker::DatasheetMode;
        let mut state = fresh_editor_state();
        state.set_datasheet_url("https://x.test/d.pdf".into());
        state.set_datasheet_mode(DatasheetMode::Url);
        // No-op — URL preserved.
        match state.draft.shared.datasheet.as_ref() {
            Some(DatasheetRef::Url { url }) => assert_eq!(url, "https://x.test/d.pdf"),
            other => panic!("expected Url, got {other:?}"),
        }
    }

    #[test]
    fn set_model_3d_round_trips_path_and_info() {
        use crate::library::editor::three_d::Model3dUploadInfo;
        use signex_library::ModelRef;
        let mut state = fresh_editor_state();
        let info = Model3dUploadInfo {
            filename: "fpv-cam.step".into(),
            hash_hex: "ab".repeat(32),
            size_bytes: 12_345,
            extension: "step".into(),
        };
        let model = ModelRef {
            path: info.storage_path(),
            offset: [1.0, 2.0, 3.0],
            rotation: [10.0, 20.0, 30.0],
        };
        state.set_model_3d(Some((model.clone(), info.clone())));

        assert_eq!(state.draft.pcb.model_3d.as_ref(), Some(&model));
        assert_eq!(state.three_d_upload_info.as_ref(), Some(&info));

        // JSON serde round-trip on PcbSide preserves the ModelRef.
        let json = serde_json::to_string(&state.draft.pcb).unwrap();
        let back: signex_library::PcbSide = serde_json::from_str(&json).unwrap();
        assert_eq!(back.model_3d.as_ref(), Some(&model));
    }

    #[test]
    fn set_model_3d_none_clears_both_sides() {
        use crate::library::editor::three_d::Model3dUploadInfo;
        use signex_library::ModelRef;
        let mut state = fresh_editor_state();
        let info = Model3dUploadInfo {
            filename: "m.step".into(),
            hash_hex: "0".repeat(64),
            size_bytes: 1,
            extension: "step".into(),
        };
        let model = ModelRef {
            path: "shared/3d-models/dummy.step".into(),
            offset: [0.0; 3],
            rotation: [0.0; 3],
        };
        state.set_model_3d(Some((model, info)));
        assert!(state.draft.pcb.model_3d.is_some());
        assert!(state.three_d_upload_info.is_some());
        state.set_model_3d(None);
        assert!(state.draft.pcb.model_3d.is_none());
        assert!(state.three_d_upload_info.is_none());
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

    #[test]
    fn where_used_for_filters_by_pinned_version_when_requested() {
        let mut state = LibraryState::default();
        let project = PathBuf::from("/tmp/p.snxprj");
        let sheet = PathBuf::from("/tmp/p/main.snxsch");
        let uuid = Uuid::now_v7();
        let v1 = Version::new(1, 0);
        let v2 = Version::new(1, 1);

        state.ingest_sheet(
            &project,
            &sheet,
            &[(uuid, "R1".into(), v1), (uuid, "R2".into(), v2)],
        );

        // Unfiltered → both instances.
        assert_eq!(state.where_used_for(uuid, None).len(), 2);
        // Filtered to v1 → just R1.
        let v1_sites = state.where_used_for(uuid, Some(v1));
        assert_eq!(v1_sites.len(), 1);
        assert_eq!(v1_sites[0].instance_id, "R1");
    }
}

/// Avoid an unused-import warning when the `local-git` feature is off
/// (the adapter import is gated, but `Component` / `Revision` are
/// always pulled in by the editor types). Phase 2 may flip this when
/// the trait widens.
#[allow(dead_code)]
fn _types_used(_c: &Component, _r: &Revision) {}
