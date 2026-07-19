//! Component-preview, picker, and misc library-modal state types.

use super::*;

/// Picker modal state.
#[derive(Debug, Clone, Default)]
pub struct PickerState {
    pub filter: String,
    pub selected: Option<(PathBuf, ComponentSummary)>,
}

// ─────────────────────────────────────────────────────────────────────
// New Component flow
// ─────────────────────────────────────────────────────────────────────

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
    /// In-flight "+ New Table…" sub-form. `None` when the user is on
    /// the regular Table picker; `Some(NewTableDraft)` while they're
    /// typing a fresh table name. Confirm dispatches a
    /// `create_empty_table` against the active library and switches
    /// the picker to the new table; Cancel just clears this field.
    pub creating_table: Option<NewTableDraft>,
    /// "Advanced ▾" disclosure flag — hides the Table picker by
    /// default so first-time users only see Library + Class + PN +
    /// Pick Symbol/Footprint. The Table picker auto-resolves to
    /// `manifest.table_for_class(class)` at submit time when the
    /// disclosure stays closed; opening it lets power users pick a
    /// custom routing or mint a new table inline.
    pub advanced_open: bool,
}

/// Inline form state for "+ New Table…" inside the New Component
/// modal — collects the name (and validation error if any) before the
/// dispatcher calls `create_empty_table` on the active library.
#[derive(Debug, Clone, Default)]
pub struct NewTableDraft {
    pub name: String,
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
            creating_table: None,
            advanced_open: false,
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
    /// Monotonic counter bumped on every `DigiKeyConnect`. The OAuth
    /// worker carries its generation through the `DigiKeyOAuthResult`
    /// message so a stale result from a cancelled flow can't clobber
    /// the state of a freshly-started one. Without this, Cancel +
    /// reconnect could let the first worker's eventual outcome
    /// overwrite the second flow's credentials.
    pub digikey_flow_generation: u64,
    pub mouser_api_key_buf: String,
    pub mouser_status: Option<String>,
    pub mouser_in_flight: bool,
    pub preferred_order: Vec<DistributorSource>,
    /// Inline error from the last preferred-order save, `None` when the
    /// last save succeeded. Without this a failed write was invisible
    /// until the user found the order reverted on next launch.
    pub preferred_order_error: Option<String>,
}

impl Default for DistributorSettings {
    fn default() -> Self {
        Self {
            digikey_account_email: None,
            digikey_status: None,
            digikey_in_flight: false,
            digikey_cancel: None,
            digikey_flow_generation: 0,
            mouser_api_key_buf: String::new(),
            mouser_status: None,
            mouser_in_flight: false,
            preferred_order: vec![
                DistributorSource::DigiKey,
                DistributorSource::Mouser,
                DistributorSource::Lcsc,
                DistributorSource::Jlcpcb,
            ],
            preferred_order_error: None,
        }
    }
}

// `ComponentPreviewState::from_row` is the canonical builder; the legacy
// `from_head` helper that constructed an editor from a `Component` chain
// is gone with the v0.9-refactor-2 DBLib model.

