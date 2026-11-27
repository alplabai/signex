//! In-memory state for the Library subsystem (DBLib model).
//!
//! Owned by [`crate::app::Signex::library`]. Components are rows inside per-category TSV
//! tables under `<lib>/tables/<category>.tsv`, addressed by
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
//! * `template_registry` — bundled + per-library parameter templates,
//!   resolved at component-class lookup time.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use signex_library::{
    ComponentRow, ComponentSummary, DistributorSource, Footprint, LibraryAdapter, LibraryError,
    LibrarySet, LocalGitAdapter, PrimitiveKind, PrimitiveSummary, RowId, SimModel, Symbol,
    TemplateRegistry, UseSite, WhereUsedIndex,
};
use signex_types::coord::Unit;

use crate::panels::SheetColor;
use uuid::Uuid;

pub use crate::app::core::editor_address::EditorAddress;

/// Lifecycle visibility mode for the Library Browser grid.
///
/// Mirrors plan §6 "Lifecycle, tagging, distributors": rows tagged
/// `Released`/`InReview`/`Draft` count as "active" for filtering, while
/// `Deprecated` and `Obsolete` rows are both hidden by default
/// (`Deprecated` rows are tinted yellow once `IncludeDeprecated` shows
/// them). Stage 18 surfaces these as a single dropdown pill in the
/// browser header so users can pivot the visible row set without
/// touching every row's lifecycle field.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LifecycleFilter {
    /// Default — show `Released` / `InReview` / `Draft`. Hides
    /// `Deprecated` and `Obsolete`.
    #[default]
    ActiveAndPreferred,
    /// Show only `Released` (the "preferred for new designs" subset
    /// once admins promote rows out of `Draft`).
    PreferredOnly,
    /// Surface `Deprecated` rows alongside the default set — for
    /// continuity / repair workflows.
    IncludeDeprecated,
    /// Show every row including `Obsolete`. The library admin's audit
    /// view.
    All,
}

impl LifecycleFilter {
    /// Stable display label for the dropdown.
    pub fn label(self) -> &'static str {
        match self {
            Self::ActiveAndPreferred => "Active + Preferred",
            Self::PreferredOnly => "Preferred Only",
            Self::IncludeDeprecated => "Incl. Deprecated",
            Self::All => "All (incl. Obsolete)",
        }
    }

    /// The catalog message of the filter pill.
    pub fn message(self) -> signex_i18n::MessageId {
        use signex_i18n::ids;
        match self {
            Self::ActiveAndPreferred => ids::LIBRARY_BROWSER_FILTER_ACTIVE,
            Self::PreferredOnly => ids::LIBRARY_BROWSER_FILTER_PREFERRED,
            Self::IncludeDeprecated => ids::LIBRARY_BROWSER_FILTER_DEPRECATED,
            Self::All => ids::LIBRARY_BROWSER_FILTER_ALL,
        }
    }

    /// Every option in stable display order — drives the
    /// `pick_list`'s value list.
    pub const ALL: &'static [LifecycleFilter] = &[
        LifecycleFilter::ActiveAndPreferred,
        LifecycleFilter::PreferredOnly,
        LifecycleFilter::IncludeDeprecated,
        LifecycleFilter::All,
    ];

    /// Whether a row in `state` should render under this filter.
    /// Deprecated rows are hidden by default and render tinted when
    /// `IncludeDeprecated` shows them.
    /// `LifecycleState` is `#[non_exhaustive]`, so any future variant
    /// falls through to hidden.
    pub fn allows(self, state: signex_library::LifecycleState) -> bool {
        use signex_library::LifecycleState as L;
        match (self, state) {
            (Self::All, _) => true,
            (Self::PreferredOnly, L::Released) => true,
            (Self::PreferredOnly, _) => false,
            (
                Self::ActiveAndPreferred | Self::IncludeDeprecated,
                L::Released | L::InReview | L::Draft,
            ) => true,
            (Self::IncludeDeprecated, L::Deprecated) => true,
            (Self::ActiveAndPreferred, L::Deprecated) => false,
            _ => false,
        }
    }
}

impl std::fmt::Display for LifecycleFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
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
    /// Lifecycle visibility filter (Stage 18). Defaults to
    /// `ActiveAndPreferred` — hides obsolete rows, tints deprecated.
    pub lifecycle_filter: LifecycleFilter,
    /// When `Some`, the right-side component grid only shows rows
    /// whose `class` field matches this exact key. Toggled by
    /// clicking a class row in the left sidebar — clicking the
    /// same row twice clears the filter.
    pub class_filter: Option<String>,
    /// Range filters on this tab's measurement columns, by parameter key
    /// (#1572). Cleared when the active table changes: the columns are
    /// the table's.
    pub range_filters: crate::library::range_filter::RangeFilters,
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
    /// Active sort column + direction. `None` = canonical insertion
    /// order (the file's row_id-sorted order from Stage 12a). When
    /// set, [`view_grid`](super::browser) sorts visible rows with
    /// auto-detected numeric comparison: if both cells parse as
    /// `f64` the sort is numeric, otherwise lexical
    /// (case-insensitive). Repeated clicks on the same column header
    /// toggle ascending/descending; clicking a different column
    /// resets to ascending.
    ///
    /// Stage 8 of `v0.9-snxlib-as-file-plan.md` — closes the Altium
    /// "lexical sort on numeric columns" pain by detecting numeric
    /// columns at compare time without needing a typed schema lookup.
    /// The `[tables.<name>.column_types]` sidecar (Stage 12a) drives
    /// stricter validation when needed; the auto-detect path keeps
    /// untyped legacy tables sorting sanely.
    pub sort_by: Option<BrowserSort>,
    /// In-flight `+ Add Table` inline form. `None` while the regular
    /// tab strip is showing; `Some(NewTableDraft)` while the user is
    /// typing a fresh table name. Confirm dispatches
    /// `BrowserConfirmAddTable` which calls `create_empty_table` on
    /// the adapter and switches `active_table` to the new row.
    /// Cancel just clears the field without writing.
    pub adding_table: Option<NewTableDraft>,
    /// Error from the most recent `× Delete Table` click. Adapter
    /// refuses non-empty deletes with `Conflict`; we render the
    /// message inline alongside the strip until the user dismisses
    /// it or clicks anywhere else.
    pub delete_error: Option<String>,
    /// In-flight rename — `(original_name, edit_buffer)`. While
    /// `Some`, the matching sidebar row renders a text input
    /// instead of the static label. Confirm dispatches a
    /// `rename_table` against the adapter; Cancel clears the field
    /// without writing.
    pub renaming_table: Option<(String, String)>,
    /// Most recent rename error (e.g. duplicate target name).
    /// Surfaces inline next to the rename input.
    pub rename_error: Option<String>,
    /// The rename in `renaming_table` is being written (#893); a second
    /// confirm waits for the result.
    pub rename_saving: bool,
    /// Filtered, sorted rows and derived columns of the active table (#1374),
    /// kept current by `LibraryState::sync_view_caches_in`; `view` reads it
    /// instead of recomputing every frame.
    pub view_cache: Option<super::browser::BrowserViewCache>,
    /// Where the grid is scrolled to, as reported by its scrollable. The
    /// grid builds only the rows in this window (#1355).
    pub grid_scroll: super::browser::GridScroll,
    /// The selected row's symbol and footprint drawings for the preview
    /// pane (mockup row 118), read off-thread with the Components panel's
    /// loader.
    pub preview: crate::panels::components_panel::details::DetailsState,
    /// Why the last preview read of the selected row gave no drawing;
    /// cleared when a new read starts.
    pub preview_error: Option<BrowserPreviewError>,
    /// The preview pane's ⋯ menu (Delete Selected) is open.
    pub more_menu_open: bool,
    /// The column chooser at the right of the count line is open (mockup
    /// row 118).
    pub columns_menu_open: bool,
    /// Columns turned on or off in the chooser, by sort key; any other
    /// column keeps its default, see [`COLUMNS_HIDDEN_BY_DEFAULT`].
    pub column_choice: std::collections::BTreeMap<String, bool>,
}

/// Sort keys of the grid columns the chooser starts with off: Rev and Tags
/// (decided 2026-11-23, mockup row 118).
pub const COLUMNS_HIDDEN_BY_DEFAULT: [&str; 2] = ["version", "parameters.tags"];

/// Active sort key + direction for the Library Browser grid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserSort {
    /// Sort key matching the column's `ColumnKind::sort_key()`.
    pub key: String,
    pub descending: bool,
}

impl LibraryBrowserState {
    pub fn new(library_path: PathBuf) -> Self {
        Self {
            library_path,
            active_table: None,
            selected_row: None,
            search: String::new(),
            lifecycle_filter: LifecycleFilter::default(),
            class_filter: None,
            range_filters: Default::default(),
            edit_modal: None,
            cell_edit: HashMap::new(),
            delete_confirm: None,
            sort_by: None,
            adding_table: None,
            delete_error: None,
            renaming_table: None,
            rename_error: None,
            rename_saving: false,
            view_cache: None,
            grid_scroll: super::browser::GridScroll::default(),
            preview: Default::default(),
            preview_error: None,
            more_menu_open: false,
            columns_menu_open: false,
            column_choice: std::collections::BTreeMap::new(),
        }
    }

    /// Whether the grid shows the column with sort key `key`.
    pub fn column_shown(&self, key: &str) -> bool {
        self.column_choice
            .get(key)
            .copied()
            .unwrap_or(!COLUMNS_HIDDEN_BY_DEFAULT.contains(&key))
    }

    /// Turn the column with sort key `key` on when off, off when on.
    pub fn toggle_column(&mut self, key: String) {
        let shown = self.column_shown(&key);
        self.column_choice.insert(key, !shown);
    }

    /// Toggle the sort: if `key` matches the current sort column, flip
    /// direction; otherwise set ascending sort on `key`.
    pub fn toggle_sort(&mut self, key: String) {
        match &mut self.sort_by {
            Some(s) if s.key == key => s.descending = !s.descending,
            _ => {
                self.sort_by = Some(BrowserSort {
                    key,
                    descending: false,
                });
            }
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
    /// The SI prefix shown next to a measurement's number, by parameter
    /// name. Seeded with the prefix the stored value displays; a
    /// parameter without an entry (or with a unit that takes no prefix)
    /// has none.
    pub param_prefix: HashMap<String, signex_types::quantity::SiPrefix>,
    /// The parameter whose unit menu is open, if any.
    pub unit_menu_open: Option<String>,
    /// Live edit buffer for the comma-separated tags row (Stage 18).
    /// Persisted to `parameters["tags"]` as `ParamValue::Text` on Save.
    /// Seeded from the existing `parameters["tags"]` value at modal open.
    pub tags_buf: String,
    /// Inline error text — surfaced if the save fails.
    pub error: Option<String>,
    /// A neutral one-line notice (not a refusal), e.g. that a unit change
    /// reinterprets the value. Cleared by the next commit.
    pub note: Option<String>,
    /// The last save was refused because the row changed on the server.
    pub stale_base: bool,
    /// "Reload saved version" awaits the user's confirmation to discard the
    /// draft.
    pub reload_confirming: bool,
    /// The primary datasheet and its label as the modal opened. The modal
    /// shows no label, so the label is kept only while the primary still
    /// points where it did (#1231).
    pub opened_primary: (signex_library::DatasheetRef, String),
    /// The copy being written by a Save that has not finished (#893): the
    /// modal stays open meanwhile, and the result closes it only if the
    /// draft still equals this.
    pub saving: Option<SavingSnapshot>,
    /// Template slots added through their "+ name" offer in this edit, each
    /// with whether an edit message has touched it since (#2648): while one
    /// is untouched its row keeps a ✕.
    pub added_slots: HashMap<String, bool>,
}

/// What the Edit Component Details modal held when Save was pressed.
#[derive(Debug, Clone)]
pub struct SavingSnapshot {
    pub draft: ComponentRow,
    pub tags_buf: String,
    pub param_buf: HashMap<String, (String, String)>,
}

impl EditRowModalState {
    /// The prefix a stored measurement opens with in the picker; `None`
    /// for anything that is not a measurement in a known unit.
    pub fn param_prefix_seed(
        v: &signex_library::ParamValue,
    ) -> Option<signex_types::quantity::SiPrefix> {
        v.measurement_parts().map(|p| p.prefix)
    }

    /// The `(value, unit)` edit-buffer text a stored parameter opens with.
    /// A measurement in a known unit opens as the number the picker's
    /// prefix applies to and the canonical unit token (`4.7`, `ohm` for a
    /// legacy `{4.7, "kΩ"}`); an unknown unit keeps its raw text.
    pub fn param_buf_entry(v: &signex_library::ParamValue) -> (String, String) {
        if let Some(parts) = v.measurement_parts() {
            return (parts.mantissa, parts.unit.token().to_string());
        }
        match v {
            signex_library::ParamValue::Text(s) => (s.clone(), String::new()),
            signex_library::ParamValue::Number(n) => (n.to_string(), String::new()),
            signex_library::ParamValue::Bool(b) => (b.to_string(), String::new()),
            signex_library::ParamValue::Measurement { value, unit } => {
                (value.to_string(), unit.clone())
            }
        }
    }

    pub fn new(address: EditorAddress, draft: ComponentRow) -> Self {
        let param_buf: HashMap<String, (String, String)> = draft
            .parameters
            .iter()
            .map(|(k, v)| (k.clone(), Self::param_buf_entry(v)))
            .collect();
        let param_prefix = draft
            .parameters
            .iter()
            .filter_map(|(k, v)| Some((k.clone(), Self::param_prefix_seed(v)?)))
            .collect();
        // Seed the tags buffer from `parameters["tags"]` if present;
        // tags live as a free-form `ParamValue::Text` keyed by "tags"
        // (plan §6).
        let tags_buf = match draft.parameters.get("tags") {
            Some(signex_library::ParamValue::Text(s)) => s.clone(),
            Some(other) => other.display(),
            None => String::new(),
        };
        let opened_primary = (draft.datasheet.clone(), draft.datasheet_label.clone());
        Self {
            address,
            draft,
            param_buf,
            param_prefix,
            unit_menu_open: None,
            tags_buf,
            error: None,
            note: None,
            stale_base: false,
            reload_confirming: false,
            opened_primary,
            saving: None,
            added_slots: HashMap::new(),
        }
    }

    /// Whether the row `key` is a template slot added in this edit and still
    /// at its default, whatever its kind, so it can be taken back (#2648).
    pub fn is_untouched_added_slot(&self, key: &str) -> bool {
        self.added_slots.get(key) == Some(&false)
    }

    /// An edit message reached the row `key`: an added slot stops being
    /// untouched, even when the value it sets is the default again.
    pub fn touch_slot(&mut self, key: &str) {
        if let Some(touched) = self.added_slots.get_mut(key) {
            *touched = true;
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
    /// Scroll offset of the list, in pixels, as last reported by its
    /// scrollable; drives which rows get built.
    pub scroll_y: f32,
    /// Symbols of `.snxsym` files outside every library, offered for a copy
    /// into the target row's library (#1841).
    pub loose: Vec<super::loose_symbols::LooseSymbol>,
    /// The loose symbol the user chose, waiting for "Copy it into the
    /// library?" to be answered.
    pub copy_confirm: Option<super::loose_symbols::LooseSymbol>,
    /// A copy is running on a worker.
    pub copying: bool,
    /// The unmounted `.snxlib` the picker asked to open and is waiting on.
    pub opening_library: Option<std::path::PathBuf>,
    /// The highlighted row and its preview (mockup row 131).
    pub selection: PickerSelection,
}

/// The pick a copy of a loose symbol was made for (#1841).
#[derive(Debug, Clone)]
pub struct CopyTarget {
    pub kind: PrimitiveKind,
    pub target: PrimitivePickerTarget,
}

impl CopyTarget {
    /// Whether `picker` is still the pick this copy was made for.
    pub fn is_open_in(&self, picker: &PrimitivePickerState) -> bool {
        picker.kind == self.kind
            && std::mem::discriminant(&picker.target) == std::mem::discriminant(&self.target)
            && picker.target.rows() == self.target.rows()
    }
}

/// Where the picker should write the picked `PrimitiveRef`.
#[derive(Debug, Clone)]
pub enum PrimitivePickerTarget {
    /// The user picks for an open Component Preview tab — apply +
    /// save the row.
    PreviewRow(EditorAddress),
    /// The user picks while editing a row in the Library Browser
    /// grid's Edit Component Details modal (Deliverable B).
    EditRowModal(EditorAddress),
    /// The user picks for the row currently selected in the Library
    /// Browser tab — apply directly through the adapter and refresh.
    /// F15 (2026-05-03): row binding lives next to the row, not in a
    /// separate Component Preview tab or modal.
    BrowserRow(EditorAddress),
    /// The in-use symbol Delete's Re-bind (#2572): the picked symbol takes
    /// the place of `replacing` in every row listed, as default and as
    /// alternate, each written as a [`Self::BrowserRow`] pick is.
    BrowserRows {
        rows: Vec<EditorAddress>,
        replacing: signex_library::PrimitiveRef,
    },
    /// The user picks a symbol to add as an alternate of the row open in a
    /// Component Preview tab (#1230).
    PreviewAlternate(EditorAddress),
    /// Update PCB's Choose footprint (#2371): the footprint of a part of the
    /// design, by its sheet placement and anchor unit, set on the schematic.
    DesignPart {
        sheet: signex_types::schematic::InstancePath,
        part: Uuid,
    },
    /// A board footprint swap (#2366): the footprints of `board` to replace,
    /// and the footprints their library parts offer, listed first.
    BoardFootprint {
        board: std::path::PathBuf,
        footprints: Vec<Uuid>,
        suggested: Vec<signex_library::PrimitiveRef>,
    },
    /// Import from another footprint (mockup row 189): the sketch editor at
    /// `editor` lists the picked footprint's parameters. Reads only.
    SketchParameters { editor: std::path::PathBuf },
}

impl PrimitivePickerTarget {
    /// The one row the pick is for; `None` for a pick that writes no row
    /// or several.
    pub fn address(&self) -> Option<&EditorAddress> {
        match self {
            Self::PreviewRow(a)
            | Self::EditRowModal(a)
            | Self::BrowserRow(a)
            | Self::PreviewAlternate(a) => Some(a),
            Self::BrowserRows { .. }
            | Self::DesignPart { .. }
            | Self::BoardFootprint { .. }
            | Self::SketchParameters { .. } => None,
        }
    }

    /// Every row the pick writes.
    pub fn rows(&self) -> &[EditorAddress] {
        match self {
            Self::PreviewRow(a)
            | Self::EditRowModal(a)
            | Self::BrowserRow(a)
            | Self::PreviewAlternate(a) => std::slice::from_ref(a),
            Self::BrowserRows { rows, .. } => rows,
            Self::DesignPart { .. }
            | Self::BoardFootprint { .. }
            | Self::SketchParameters { .. } => &[],
        }
    }

    /// Directory of the library whose rows this pick writes; `None` when
    /// it writes no row, or rows of several libraries.
    pub fn library_path(&self) -> Option<&std::path::Path> {
        let (first, rest) = self.rows().split_first()?;
        rest.iter()
            .all(|a| a.library_path == first.library_path)
            .then_some(first.library_path.as_path())
    }

    /// Every library this pick writes a row of, each once.
    pub fn library_paths(&self) -> Vec<&std::path::Path> {
        let mut paths: Vec<&std::path::Path> = self
            .rows()
            .iter()
            .map(|a| a.library_path.as_path())
            .collect();
        paths.sort();
        paths.dedup();
        paths
    }
}

/// Top-level Library subsystem state. Stored on
/// [`crate::app::Signex`] as a single field so the dispatcher can
/// borrow it independently of the rest of `DocumentState`.
pub struct LibraryState {
    /// Shows a datasheet in the platform's viewer. A field so a test can
    /// record what would open instead of launching a browser (#1301).
    pub datasheet_opener: crate::library::placed_part::OpenDatasheet,
    /// Embedded-vs-library symbol drift per open sheet (#912).
    pub drift: super::drift::DriftState,
    /// A symbol Delete waiting for the user's answer (#1845).
    pub pending_symbol_delete: Option<super::symbol_delete::PendingSymbolDelete>,
    /// Last serial handed to a symbol-Delete warning (#1903).
    pub symbol_delete_serial: u64,
    /// Latest switch request per placed symbol (#1230).
    pub symbol_choice_requests: SymbolChoiceRequests,
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
    /// Library left-dock search box buffer.
    pub panel_search: String,
    /// Library panel rows the user expanded (#1895), by `.snxlib` path.
    pub panel_expanded: std::collections::HashSet<PathBuf>,
    /// The component a single click highlighted in the Library panel.
    pub panel_selected: Option<ComponentsPanelRowKey>,
    /// The Library panel's last component click, for the double-click
    /// window that opens the component.
    pub panel_last_click: crate::double_click::DoubleClick<ComponentsPanelRowKey>,
    /// The Library panel's kind pick and filtered Parts rows (mockup row
    /// 119).
    pub library_panel: LibraryPanelState,
    /// "Close Library — Unsaved Drafts" modal state — `None` while closed.
    pub close_library_confirm: Option<CloseLibraryConfirmState>,
    /// `(label, reason)` of each Save-All row whose off-thread datasheet
    /// store or commit failed, keyed by `(batch id, row)`, until that Save
    /// All settles and reports it. Another Save All never touches them.
    pub batch_row_failures: HashMap<(u64, EditorAddress), (String, String)>,
    /// Save-All rows whose staged-PDF store or row write is still running,
    /// keyed by the save's generation: `(batch id, row)`. A settle step
    /// waits until its batch has none (#1728).
    pub batch_rows_in_flight: HashMap<u64, (u64, EditorAddress)>,
    /// Bundled + per-library parameter templates. Reference-counted
    /// because both the editor and the validator borrow it. The
    /// Component Preview tab + the validator both read through this
    /// registry; this struct owns the field.
    pub template_registry: Arc<TemplateRegistry>,
    /// Per-browser-tab state, keyed by `.snxlib` root path. One entry
    /// per `TabKind::LibraryBrowser(path)` tab in the main window's
    /// tab bar; insert on tab open, drop on tab close.
    pub library_browsers: HashMap<PathBuf, LibraryBrowserState>,
    /// The library whose browser tab was showing at the last update, so a
    /// switch to another tab can be told apart from a repeat of the same one.
    pub shown_browser: Option<PathBuf>,
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
    /// Library Missing dialogs queued behind `recovery` (#1897).
    pub missing: missing_queue::MissingLibraries,
    /// New Library dialog state — `None` while closed. Opens
    /// after the user picks a `.snxlib` save target in the New Library
    /// Save-As dialog (Stage 11 of `v0.9-snxlib-as-file-plan.md`). The
    /// modal lets the user opt into Git LFS for binary 3D models
    /// (`*.step` / `*.stp` / `*.wrl` / `*.iges`) before the adapter
    /// runs `git init` + initial commit.
    pub create_options: Option<LibraryCreateOptionsState>,
    /// "Library Updates Available" modal state — Stage 16 of
    /// `v0.9-snxlib-as-file-plan.md` §3.5. Populated by the
    /// schematic-open scan when the source library's mode is
    /// [`signex_library::WorkflowMode::Team`] and at least one placed
    /// Symbol's pinned version drifts from the row's current version.
    /// Personal-mode schematic opens auto-apply silently and never
    /// build this state.
    pub library_updates: Option<super::updates_dialog::LibraryUpdatesState>,
    /// Updates modals for other sheets, waiting for the open one to close
    /// (one modal at a time). Promoted by `show_next_queued_updates`.
    pub queued_library_updates: Vec<super::updates_dialog::LibraryUpdatesState>,
    /// Set of schematic paths the user explicitly skipped updates on.
    /// Nothing renders this set yet (no status-bar indicator). Cleared
    /// on apply / close-tab, and on a re-scan with no drift and no
    /// `skipped_versions` entry still pending.
    pub skipped_updates_for: std::collections::HashSet<PathBuf>,
    /// The versions Skip All dismissed, per sheet; the scan leaves them out.
    pub skipped_versions: super::updates_dialog::SkippedVersions,
    /// Session-scoped "Installed" libraries — opened via the
    /// Components Panel's "+ Add Library…" button (Stage 9 of
    /// `v0.9-snxlib-as-file-plan.md` §3 mount-source 2). Wiped on app
    /// close; the user can promote an entry to Global to make it
    /// stick.
    pub installed_libraries: Vec<PathBuf>,
    /// Signex-wide "Global" libraries — persisted to
    /// `<config_dir>/signex/global_libraries.toml` and re-mounted on
    /// every app start (Stage 9 mount-source 3). The on-disk schema
    /// lives in `panels::components_panel::global_prefs`.
    pub global_libraries: Vec<crate::panels::components_panel::global_prefs::GlobalLibraryEntry>,
    /// Mounted path of the bundled stock library; `None` when it was not
    /// found or failed to mount. Set once at bootstrap.
    pub stock_library: Option<PathBuf>,
    /// Where the bundled stock library was resolved, mounted or not. A
    /// stock library that fails to open is never mounted, so only this
    /// field lets the open-failure recovery flows recognise it (#814).
    pub stock_library_root: Option<PathBuf>,
    /// The stock library mounts writable so a maintainer edits what ships
    /// (#1892). Only a source checkout's `assets/stock-library` with
    /// `SIGNEX_EDIT_STOCK=1`; see `components_panel::stock`. Set once at
    /// bootstrap.
    pub stock_editable: bool,
    /// Stock copies refused while the bundled stock was still mounting:
    /// mounted after all if that mount fails (#1894).
    pub stock_copies_refused: Vec<PathBuf>,
    /// Libraries refused or closed because another shares their
    /// `library_id` (#1908); the Components panel lists them.
    pub id_clash_notices: Vec<IdClashNotice>,
    /// Components Panel UI state — collapsed sections + substring
    /// filter. Stage 9 ships the simple substring matcher; the full
    /// search syntax (plan §5) is polish work.
    pub components_panel: ComponentsPanelState,
    /// Footprints read for the Properties FOOTPRINT card, by footprint and
    /// `data_version`.
    pub properties_footprints: crate::panels::FootprintCards,
    /// `.snxlib` mounts currently being prepared off the UI thread —
    /// issue #99 part 2c. Keyed by the `.snxlib` file path, valued by
    /// why the mount was asked for.
    ///
    /// Three jobs in one map, which is why it is an intent map and not
    /// the `HashSet<PathBuf>` that `DocumentState.pending_opens` uses
    /// for schematic/PCB opens:
    ///
    /// - **In-flight marker.** A path present here must not be spawned
    ///   again, or two adapters race to mount it.
    /// - **Intent, re-read at completion.** A project auto-mount and a
    ///   user double-click can request the same path concurrently; the
    ///   intent is upgraded in place (`Silent` → `OpenBrowserTab`) and
    ///   the completion handler reads it from *here*, never from a value
    ///   captured at spawn time.
    /// - **Cancellation tombstone.** `close_library` removes the entry,
    ///   so a completion whose path is gone discards its payload instead
    ///   of resurrecting a library the user just closed.
    ///
    /// Path-keyed rather than generation-counted on purpose: two
    /// concurrent mounts of *different* libraries are both legitimate,
    /// so a single counter would cancel the wrong one.
    pub pending_mounts: HashMap<PathBuf, super::mount::MountIntent>,
    /// Libraries mounted whose `requires` were not checked yet; checked
    /// once no mount is pending (`library::requires`, #3112).
    pub requires_unchecked: Vec<PathBuf>,
    /// Libraries holding saved changes that no git commit has yet (#858).
    pub uncommitted: uncommitted::UncommittedState,
    /// An external-edit `stat` pass is running off-thread (#531); a tick
    /// that lands meanwhile is dropped rather than stacked behind it.
    pub external_poll_in_flight: bool,
    /// Set while a poll's blocking `stat` pass runs, including one that
    /// outlived [`crate::library::external_poll::POLL_TIMEOUT`]. No new pass
    /// starts behind it, so a hung network mount cannot pile up threads.
    ///
    /// The one piece of Model state written off the UI thread: after a
    /// timeout no message is pending to clear it, and a hung `stat` may never
    /// return to send one, so `update` reads the blocking thread's own
    /// drop-guarded flag instead (#869 review).
    pub external_poll_stat_running: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// A stall was reported and the poll has not completed since, so the
    /// warning is not repeated every tick (#827).
    pub external_poll_stall_reported: bool,
    /// A failed pass was reported and the poll has not completed since, so
    /// a pass that fails every tick logs once, not every 2 s (#869 review).
    pub external_poll_failure_reported: bool,
    /// The Refresh All Pricing walk in progress, if any. One at a time: a
    /// second request is refused with a Messages-panel line.
    pub pricing_refresh: Option<super::pricing::PricingRefreshRun>,
    /// The write order of each library (#893): library mutations run on
    /// workers, one library at a time, in the order the user made them.
    pub writes: super::writes::LibraryWrites,
    /// Row updates a primitive file's save still owes, outliving its editor
    /// (#2004).
    pub owed_cascades: owed_cascades::OwedCascades,
    /// Libraries whose close waits for their writes to finish.
    pub closing: std::collections::HashSet<PathBuf>,
    /// Libraries mounted read-only because their Git history is missing
    /// (Skip for now on Git Repository Missing), until it is restored.
    pub history_missing: std::collections::HashSet<PathBuf>,
    /// Libraries unmounted since the last `finish_update`, which drains
    /// this to close their Library Browser and Component Preview tabs.
    pub unmounted: Vec<PathBuf>,
    /// "Now" as `view` may use it, for the age of cached prices. `view` is
    /// pure and cannot ask the clock, so `update` sets this on the poll tick
    /// and whenever a library's caches are refreshed.
    pub clock: chrono::DateTime<chrono::Utc>,
    /// Latest refresh generation per library root (#1374). An off-thread
    /// refresh result applies only while its generation is still the one
    /// recorded here; a newer request, a synchronous refresh or closing
    /// the library supersedes it.
    pub refresh_generations: HashMap<PathBuf, u64>,
    /// Source of refresh generations — never reused, so a result from a
    /// closed library cannot match a later mount of the same path.
    pub refresh_generation_counter: u64,
    /// Latest preview re-resolution plan per library (`library_id`). A
    /// readout applies only while its generation is still the one here.
    pub preview_generations: HashMap<Uuid, u64>,
    /// Latest row reload planned per Component Preview. A reload applies
    /// only while its generation is still the one here.
    pub editor_reload_generations: HashMap<EditorAddress, u64>,
    /// Latest alternate-symbol lookup planned per Component Preview. A
    /// lookup applies only while its generation is still the one here.
    pub alternate_generations: HashMap<EditorAddress, u64>,
    /// Latest first-visit tab lookup planned per Component Preview. A
    /// lookup applies only while its generation is still the one here.
    pub tab_resolve_generations: HashMap<EditorAddress, u64>,
    /// The Properties panel's summaries of the selected row's bindings,
    /// read off the UI thread.
    pub row_summary_job: super::row_summaries::RowSummaryJob,
    /// Off-thread reads started and not yet reported: generation -> the
    /// library root it reads. Keyed by generation (unique, never reused) so
    /// a late report removes only its own read, whatever happened to the
    /// path since (relocated, or closed and mounted again).
    pub refresh_in_flight: HashMap<u64, PathBuf>,
    /// Follow-ups of refreshes that have not landed yet, per library root.
    pub pending_follow_ups: HashMap<PathBuf, Vec<super::refresh::RefreshFollowUp>>,
    /// Bumped whenever an open library's rows, prices or the set of open
    /// libraries changes. Caches derived from them (the picker list, the
    /// browser grid) rebuild when it moves — see `touch_data`.
    pub data_version: u64,
    /// Preference-backed defaults for a library opened or created from now
    /// on; `Save Preferences` updates it, so no reader goes back to disk.
    pub new_library_defaults: NewLibraryDefaults,
    /// The open Copy to Library dialog (#813): a stock symbol or footprint
    /// was asked for and no usable target library is remembered.
    pub stock_copy: Option<super::stock_copy::StockCopyState>,
    /// The library the user chose to receive stock copies, remembered in
    /// `prefs.json`. May name a library that is no longer mounted; it is
    /// checked with [`LibraryState::usable_stock_copy_target`] on use.
    pub stock_copy_target: Option<PathBuf>,
    /// Stock files whose copy is being written, so a second open of the
    /// same file does not start a second copy.
    pub stock_copies_in_flight: std::collections::HashSet<PathBuf>,
    /// The library import running, and the last one finished (#3114).
    pub import: crate::library_import::run_state::ImportState,
    /// The Properties panel state of each symbol editor (mockup rows 122
    /// and 123): its tab, Pins filter, part chip and illustration hover, by
    /// `.snxsym` path. Session only; an entry outlives its tab so a
    /// reopened file shows the same tab.
    pub symbol_props: HashMap<PathBuf, crate::library::editor::symbol::state::SymbolPropsUi>,
    /// The open Mirror card of each sketch editor (mockup row 191), by the
    /// editor's path: the `.snxfpt` of a footprint, the board of a board
    /// sketch. Session only, never saved or undone.
    pub mirror_cards:
        HashMap<PathBuf, crate::library::editor::footprint::sketch_mode::mirror_card::MirrorCard>,
}

/// Three mount sources surfaced as collapsible sections inside the
/// Components Panel (Stage 9 of `v0.9-snxlib-as-file-plan.md` §3).
/// Drives section header rendering + the section-add button
/// dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentsMountSource {
    /// Auto-mounted from `Project.libraries` for any loaded project.
    Project,
    /// Session-scoped — opened via the Components Panel's
    /// "+ Add Library…" button. Wiped on app close.
    Installed,
    /// Persisted across app launches via
    /// `<config_dir>/signex/global_libraries.toml`.
    Global,
    /// The library bundled with the install; resolved and mounted at
    /// bootstrap, never recorded in `global_libraries.toml`.
    Stock,
}

impl ComponentsMountSource {
    pub fn label(self) -> &'static str {
        match self {
            Self::Project => "Project",
            Self::Installed => "Installed",
            Self::Global => "Global",
            Self::Stock => "Stock",
        }
    }

    /// The catalog message of [`Self::label`] (source chips, the Add menu).
    pub fn message(self) -> signex_i18n::MessageId {
        use signex_i18n::ids;
        match self {
            Self::Project => ids::PANELS_COMPONENTS_SOURCE_PROJECT,
            Self::Installed => ids::PANELS_COMPONENTS_SOURCE_INSTALLED,
            Self::Global => ids::PANELS_COMPONENTS_SOURCE_GLOBAL,
            Self::Stock => ids::PANELS_COMPONENTS_SOURCE_STOCK,
        }
    }

    /// The source badge of a library row, written in capitals.
    pub fn badge(self) -> signex_i18n::MessageId {
        use signex_i18n::ids;
        match self {
            Self::Project => ids::PANELS_COMPONENTS_BADGE_PROJECT,
            Self::Installed => ids::PANELS_COMPONENTS_BADGE_INSTALLED,
            Self::Global => ids::PANELS_COMPONENTS_BADGE_GLOBAL,
            Self::Stock => ids::PANELS_COMPONENTS_BADGE_STOCK,
        }
    }

    /// Lower-case identifier of the source.
    pub fn key(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Installed => "installed",
            Self::Global => "global",
            Self::Stock => "stock",
        }
    }

    /// Every source in list order: Stock first, then Project, Installed
    /// and Global.
    pub const ORDER: &'static [ComponentsMountSource] = &[
        ComponentsMountSource::Stock,
        ComponentsMountSource::Project,
        ComponentsMountSource::Installed,
        ComponentsMountSource::Global,
    ];

    /// The sources "+ Add" can mount a library into, in menu order.
    pub const ADDABLE: &'static [ComponentsMountSource] = &[
        ComponentsMountSource::Project,
        ComponentsMountSource::Installed,
        ComponentsMountSource::Global,
    ];
}

/// How the Components panel draws its part rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ComponentsRowStyle {
    /// One line, the part name only; the description is a tooltip.
    #[default]
    Compact,
    /// Taller rows with a symbol thumbnail and a one-line description.
    Previews,
}

impl ComponentsRowStyle {
    /// The style a saved "Previews" switch stands for.
    pub fn from_previews(previews: bool) -> Self {
        if previews {
            Self::Previews
        } else {
            Self::Compact
        }
    }

    /// Whether the "Previews" switch is on.
    pub fn shows_previews(self) -> bool {
        self == Self::Previews
    }
}

/// Source filter, collapse and filter state for the Components Panel.
/// Session-scoped except [`Self::row_style`], which `prefs.json` keeps.
#[derive(Debug, Clone)]
pub struct ComponentsPanelState {
    /// The source chip selected under the search box; `None` is "All".
    pub source_filter: Option<ComponentsMountSource>,
    /// Libraries the user collapsed (by root); every other library is open.
    pub collapsed_libraries: std::collections::HashSet<PathBuf>,
    /// The header "+ Add" menu is open.
    pub add_menu_open: bool,
    /// Substring filter applied to mpn / manufacturer / internal_pn
    /// / library name. Empty = show everything. Stage 9 uses a
    /// case-insensitive `contains` match across all three fields;
    /// the rich `mpn:LM317 lifecycle:preferred` syntax (plan §5) is
    /// a follow-up.
    pub filter: String,
    /// Searchable row list the panel draws; kept current by
    /// `LibraryState::sync_view_caches_in` (#1353).
    pub rows: PanelRows,
    /// The row a click highlighted; the details section shows it.
    pub selected: Option<ComponentsPanelRowKey>,
    /// Height of the list above the details section; the divider drags it.
    pub list_height: f32,
    /// The pointer is over the divider.
    pub split_hovered: bool,
    /// The divider is being dragged.
    pub split_dragging: bool,
    /// Zoom and pan of the card's Symbol and Footprint previews.
    pub preview_views: crate::panels::components_panel::details::preview_view::PreviewViews,
    /// The Mouse wheel preference, mirrored by `sync_zoom_step`.
    pub wheel_mode: crate::canvas::WheelMode,
    /// The selected row's details, or the load that is filling them.
    pub details: crate::panels::components_panel::details::DetailsState,
    /// The footprint picked in the details view, with the row it was picked
    /// for (#1234). Absent means the row's default; Place and the preview
    /// both follow it. A pick for another row than the selected one is void.
    pub footprint_pick: Option<(
        ComponentsPanelRowKey,
        signex_library::primitive::PrimitiveRef,
    )>,
    /// A row press that may become a drag onto the schematic.
    pub drag: Option<ComponentsRowDrag>,
    /// The row context menu, while open.
    pub row_menu: Option<ComponentsRowMenu>,
    /// The row under the pointer, for its hover tint.
    pub hovered: Option<ComponentsPanelRowKey>,
    /// A row dropped on the canvas: the placement epoch its read runs under
    /// and the world point (mm) to commit the part at once it arrives.
    pub pending_drop: Option<(u64, (f64, f64))>,
    /// Compact or Previews rows; the "Previews" switch next to Add.
    pub row_style: ComponentsRowStyle,
    /// Parts placed from this panel this session, most recent first.
    pub recent: crate::panels::components_panel::recent::RecentlyPlaced,
    /// Row symbol thumbnails read so far, by symbol reference.
    pub thumbs: crate::panels::components_panel::thumbs::SymbolThumbs,
}

impl Default for ComponentsPanelState {
    fn default() -> Self {
        Self {
            source_filter: None,
            collapsed_libraries: Default::default(),
            add_menu_open: false,
            filter: String::new(),
            rows: PanelRows::default(),
            selected: None,
            list_height: crate::panels::components_panel::split::DEFAULT_LIST_HEIGHT,
            split_hovered: false,
            split_dragging: false,
            preview_views: Default::default(),
            wheel_mode: Default::default(),
            details: Default::default(),
            footprint_pick: None,
            drag: None,
            row_menu: None,
            hovered: None,
            pending_drop: None,
            row_style: ComponentsRowStyle::default(),
            recent: Default::default(),
            thumbs: Default::default(),
        }
    }
}

/// One Components panel row: its library, table and row id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentsPanelRowKey {
    pub library_path: PathBuf,
    pub table: String,
    pub row_id: signex_library::RowId,
}

/// A pressed row: it becomes a drag once the pointer moves past the
/// threshold from `start`; released before that, it was a plain click.
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentsRowDrag {
    pub key: ComponentsPanelRowKey,
    pub start: (f32, f32),
    pub started: bool,
    /// The undocked window the press happened in (`None`: the main window):
    /// the drag label paints there and the drop lands on its sheet.
    pub window: Option<iced::window::Id>,
    /// The part was read for the cursor ghost: the pointer reached the
    /// schematic canvas while dragging, so the part is (or is being) attached.
    pub armed: bool,
}

/// What a row menu item does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentsRowAction {
    /// Attach the part to the cursor, like a drag onto the canvas.
    Place,
    /// Open the row's Component Preview tab for editing.
    Open,
    /// Copy the row's internal part number to the clipboard.
    CopyPartNumber,
}

/// The row context menu: the row it acts on and where it opened.
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentsRowMenu {
    pub key: ComponentsPanelRowKey,
    pub x: f32,
    pub y: f32,
    /// The undocked window that opened the menu (`None`: the main window).
    pub window: Option<iced::window::Id>,
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

/// State for the New Library dialog that pops up between the
/// Save-As dialog (where the user picked the `.snxlib` filename) and
/// the actual `LocalGitAdapter::init` call. Carries the project the
/// library should attach to + the chosen `.snxlib` path so the
/// dispatcher can finalise `commands::create_library_at` once the user
/// confirms.
///
/// Two user-facing toggles: `enable_git` (opt-in to version control
/// — default off, fresh libraries land as plain files) and `use_lfs`
/// (only meaningful when version control is on).
#[derive(Debug, Clone)]
pub struct LibraryCreateOptionsState {
    /// Project the library will attach to — re-resolved at confirm
    /// time in case the project unloads between modal spawn and
    /// confirmation.
    pub project_path: PathBuf,
    /// `.snxlib` file path the user picked in the Save-As dialog.
    pub lib_path: PathBuf,
    /// "Enable version control" — when on, adapter runs `git init`
    /// at the parent dir and stages the initial commit. Defaults to
    /// off so fresh libraries don't surprise users with a hidden
    /// `.git/`. Users can opt in later via Enable Version Control
    /// (project-level command, future work).
    pub enable_git: bool,
    /// "Use Git LFS for binary 3D models" — defaults to `false`.
    /// Only meaningful when `enable_git` is also true; the modal
    /// greys it out otherwise.
    pub use_lfs: bool,
    /// Why the last Create press was refused: a library is already at the
    /// path, the file name is not usable, the path is already waiting to be
    /// created, or its folder is the root of a repository with history. The
    /// modal stays open and shows it; toggling version control clears only
    /// the repository refusal.
    pub refusal: Option<String>,
    /// The refusal is a library already at the path: it survives the
    /// version-control toggle and the banner offers to open that library.
    pub existing_library: bool,
    /// The refusal is the path itself (an unusable name, or already waiting
    /// to be created): it survives the version-control toggle too.
    pub path_refused: bool,
}

mod browser_preview;
mod closing;
mod id_precedence;
mod methods;
mod missing_queue;
mod new_library_defaults;
pub mod owed_cascades;
mod panel_kind;
mod panel_rows;
#[cfg(test)]
mod panel_rows_tests;
mod picker_cache;
#[cfg(test)]
mod picker_cache_tests;
mod picker_preview;
mod placed_part;
mod preview;
mod preview_history;
mod refresh;
#[cfg(test)]
mod refresh_tests;
mod relocate;
#[cfg(test)]
mod scale_bench_tests;
mod stock_copy;
mod symbol_choice;
mod table_scan;
#[cfg(test)]
mod tests;
mod uncommitted;
mod under_folder;

pub use crate::app::core::new_library_defaults::NewLibraryDefaults;
pub use browser_preview::BrowserPreviewError;
pub use id_precedence::IdClashNotice;
pub use methods::*;
pub use panel_kind::{
    LibraryPanelKind, LibraryPanelPick, LibraryPanelState, LibraryPanelTab, LibraryRowStyle,
};
pub use panel_rows::PanelRows;
pub use picker_cache::{PickerEntry, PickerRows};
pub use picker_preview::{PickerSelection, PrimitiveChoice, PrimitiveDrawing};
pub use placed_part::{
    ChangeFilter, ChangeFilters, ChangePreview, ChangeScope, ChangeSession, CompareField,
    CompareKind, CompareRow, DatasheetChoice, PickerMode, PinMatch, PlacedNow,
};
pub use preview::*;
pub use preview_history::{EditScope, RowHistory};
pub use refresh::{LibrarySnapshot, RefreshFinish, RefreshStart};
pub use relocate::rebase_path;
pub use stock_copy::STOCK_LIBRARY_ID;
pub use symbol_choice::SymbolChoiceRequests;
pub use uncommitted::{CommitNowReport, UncommittedState};
