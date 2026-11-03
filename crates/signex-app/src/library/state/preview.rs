//! Component-preview, picker, and misc library-modal state types.

use super::*;

/// Picker modal state.
#[derive(Debug, Clone, Default)]
pub struct PickerState {
    pub filter: String,
    /// Range filters on parameter columns of the list (#1572).
    pub range_filters: crate::library::range_filter::RangeFilters,
    pub selected: Option<(PathBuf, ComponentSummary)>,
    /// The list the picker draws; kept current by `sync_view_caches`.
    pub rows: PickerRows,
    /// What the modal is for; see [`PickerMode`].
    pub mode: super::placed_part::PickerMode,
}

/// Inline form state for "+ New Table…" on the Library Browser
/// tab strip — collects the name (and validation error if any) before the
/// dispatcher calls `create_empty_table` on the active library.
#[derive(Debug, Clone, Default)]
pub struct NewTableDraft {
    pub name: String,
    pub error: Option<String>,
    /// The table is being created (#893); a second confirm waits.
    pub saving: bool,
}

/// "Close Library — Unsaved Drafts" confirmation modal state.
#[derive(Debug, Clone)]
pub struct CloseLibraryConfirmState {
    pub library_path: PathBuf,
    pub library_name: String,
    pub dirty_editors: Vec<EditorAddress>,
    /// Symbol / footprint drafts of this library with unsaved edits.
    pub dirty_paths: Vec<PathBuf>,
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

    /// The SI prefix picked next to a measurement's number buffer, by
    /// parameter name. Absent = the prefix the stored value displays.
    pub params_prefix: HashMap<String, signex_types::quantity::SiPrefix>,

    /// The class-template slot behind a measurement's pending edit, by
    /// parameter name, so a commit (Enter, a prefix pick, Save) enforces
    /// the slot's unit and sign rules.
    pub params_slot: HashMap<String, signex_library::ParamSlot>,

    /// Why a measurement's pending edit was refused, by parameter name.
    /// Shown next to the field; Save is blocked while one is pending.
    pub params_error: HashMap<String, String>,

    /// A neutral one-line notice beside a measurement (not a refusal), by
    /// parameter name: a unit change reinterprets the number.
    pub params_note: HashMap<String, String>,

    /// Name typed into the "Add custom parameter" input, consumed by the
    /// kind pills.
    pub new_param_name: String,

    /// Inline pin-map editor state for the Preview tab's pin-map
    /// subsection. Holds expanded_row + override_buf only; the
    /// canonical pin/pad bindings live on `row.pin_map_overrides`.
    pub pin_map_state: PinMapInlineState,

    /// Why the last alternate symbol was refused (pin mismatch, repeat,
    /// unresolved symbol); shown under the alternate list, cleared by the
    /// next successful add or remove.
    pub alternate_error: Option<String>,

    /// True if any inline form edit has been applied since the last
    /// save. Drives the close-tab dirty prompt.
    pub dirty: bool,
    /// The last save was refused because someone else saved the row
    /// first: the working copy's base revision is behind the server's.
    /// Surfaces the "Reload saved version" action.
    pub stale_base: bool,
    /// "Reload saved version" was pressed and waits for the user to confirm
    /// discarding the working copy.
    pub reload_confirming: bool,
    /// PDFs picked for a pinned datasheet and not yet saved (#1332): the
    /// bytes wait here, keyed by hash, until the row's Save writes them
    /// to the library and commits them with the row. Nothing touches the
    /// disk at pick time.
    pub staged_datasheets: HashMap<String, signex_library::PdfAsset>,
    /// Why the last pick for a document (by its index) was refused.
    /// Cleared when the pick is cancelled, the mode changes, a pick
    /// succeeds, or the document list is reshaped.
    pub datasheet_errors: HashMap<usize, String>,
    /// The row as it was when Save was pressed, while that save's PDFs are
    /// still being written. Set means a save is running: a second Save and
    /// a reload are refused, and the save commits this copy, so edits made
    /// meanwhile stay unsaved and the editor stays dirty.
    pub saving_row: Option<ComponentRow>,
    /// Stamp of the save `saving_row` belongs to (`DocumentSaves::mint_generation`),
    /// so the completion of an older save (the editor was closed and
    /// reopened meanwhile) cannot act on a newer one. `0` until a save starts.
    pub saving_generation: u64,
    /// The Save All (its batch id) the running save belongs to: its outcome
    /// is recorded for that flow's settle step
    /// (`LibraryState::batch_row_failures`) instead of raising its own card.
    pub saving_batch: Option<u64>,
    /// A primitive bind (#893) is being written on a worker. It moves the
    /// row's revision, so a Save or a reload started meanwhile is refused
    /// until the result is in.
    pub binding_primitive: bool,
    /// Bumped by [`Self::mark_dirty`] and [`Self::note_edit`], so a read that
    /// was planned before an edit can tell the edit happened, whichever
    /// field it touched, and from wherever it came. View-only events never
    /// bump it.
    pub edit_epoch: u64,
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
            params_prefix: HashMap::new(),
            params_slot: HashMap::new(),
            params_error: HashMap::new(),
            params_note: HashMap::new(),
            new_param_name: String::new(),
            pin_map_state: PinMapInlineState::default(),
            alternate_error: None,
            dirty: false,
            stale_base: false,
            reload_confirming: false,
            staged_datasheets: HashMap::new(),
            datasheet_errors: HashMap::new(),
            saving_row: None,
            saving_generation: 0,
            saving_batch: None,
            binding_primitive: false,
            edit_epoch: 0,
        }
    }

    /// The prefix the picker beside a measurement's number shows: the one
    /// picked, else the one the stored value displays with, else none.
    pub fn measurement_prefix(&self, name: &str) -> signex_types::quantity::SiPrefix {
        self.params_prefix
            .get(name)
            .copied()
            .or_else(|| {
                self.row
                    .parameters
                    .get(name)
                    .and_then(|v| v.measurement_parts())
                    .map(|p| p.prefix)
            })
            .unwrap_or(signex_types::quantity::SiPrefix::One)
    }

    /// Unsaved work: the dirty flag, or a half-typed numeric value still in
    /// an edit buffer (those do not set `dirty` until they parse).
    pub fn has_unsaved_edits(&self) -> bool {
        self.dirty || !self.params_edit_buf.is_empty()
    }

    /// The staged PDFs the row still pins, for the save to write. A PDF
    /// that was picked and then replaced or removed is not written.
    pub fn staged_assets_in_use(&self) -> Vec<signex_library::PdfAsset> {
        self.staged_assets_pinned_by(&self.row)
    }

    /// The staged PDFs `row` pins.
    pub fn staged_assets_pinned_by(&self, row: &ComponentRow) -> Vec<signex_library::PdfAsset> {
        let mut used: Vec<signex_library::PdfAsset> = row
            .documents()
            .iter()
            .filter_map(|doc| match &doc.source {
                signex_library::DatasheetRef::HashPinned { hash, .. } => {
                    self.staged_datasheets.get(hash).cloned()
                }
                signex_library::DatasheetRef::Url { .. } => None,
            })
            .collect();
        used.sort_by(|a, b| a.hash.cmp(&b.hash));
        used.dedup_by(|a, b| a.hash == b.hash);
        used
    }

    /// Drop staged PDFs nothing needs any more. A PDF stays staged while
    /// the live row pins it or the copy being saved (`saving_row`) does,
    /// so a pick made during a save cannot strip the saved copy's bytes.
    pub fn prune_staged_datasheets(&mut self) {
        let mut keep = self.staged_assets_in_use();
        if let Some(saving) = &self.saving_row {
            keep.extend(self.staged_assets_pinned_by(saving));
        }
        self.staged_datasheets
            .retain(|hash, _| keep.iter().any(|a| &a.hash == hash));
    }

    /// The preview's content was edited: it has unsaved work, and a read
    /// planned before this (a reload) no longer describes it.
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
        self.note_edit();
    }

    /// Record an edit that is not (yet) unsaved work, such as a bind that is
    /// already being written, or half-typed text.
    pub fn note_edit(&mut self) {
        self.edit_epoch += 1;
    }

    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }
}

/// Distributor APIs Settings panel state.
#[derive(Debug, Clone)]
pub struct DistributorSettings {
    pub digikey_account_email: Option<String>,
    pub digikey_status: Option<String>,
    pub digikey_in_flight: bool,
    /// A DigiKey worker (or the keyring delete of a token a cancelled one
    /// stored) is still outstanding. Unlike `digikey_in_flight` it survives
    /// Cancel: the keyring has one slot, so a new Connect is refused until
    /// the previous worker's result - and any stale-token delete it
    /// triggered - has been processed. That keeps at most one writer
    /// alive, which is what makes the stale cleanup race-free.
    pub digikey_worker_busy: bool,
    pub digikey_cancel: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    /// Monotonic counter bumped on every `DigiKeyConnect`. The OAuth
    /// worker carries its generation through the `DigiKeyOAuthResult`
    /// message so a stale result from a cancelled flow can't clobber
    /// the state of a freshly-started one. Without this, Cancel +
    /// reconnect could let the first worker's eventual outcome
    /// overwrite the second flow's credentials.
    pub digikey_flow_generation: u64,
    /// Deletes the stored DigiKey refresh token. A field, not a hard-wired
    /// call, so a test can observe the request without touching the OS
    /// credential store.
    pub digikey_token_remover: fn() -> Result<(), String>,
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
            digikey_worker_busy: false,
            digikey_cancel: None,
            digikey_flow_generation: 0,
            digikey_token_remover: crate::library::settings::digikey_oauth::remove_stored_token,
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
