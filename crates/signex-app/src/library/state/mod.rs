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

/// Lifecycle visibility mode for the Library Browser grid.
///
/// Mirrors plan §6 "Lifecycle, tagging, distributors": rows tagged
/// `Released`/`InReview`/`Draft` count as "active" for filtering, while
/// `Deprecated` rows are tinted yellow when shown and `Obsolete` rows
/// are hidden by default. Stage 18 surfaces these as a single dropdown
/// pill in the browser header so users can pivot the visible row set
/// without touching every row's lifecycle field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LifecycleFilter {
    /// Default — show `Released` / `InReview` / `Draft` and tint
    /// `Deprecated`. Hides `Obsolete`.
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

    /// Every option in stable display order — drives the
    /// `pick_list`'s value list.
    pub const ALL: &'static [LifecycleFilter] = &[
        LifecycleFilter::ActiveAndPreferred,
        LifecycleFilter::PreferredOnly,
        LifecycleFilter::IncludeDeprecated,
        LifecycleFilter::All,
    ];

    /// Whether a row in `state` should render under this filter.
    /// Deprecated rows render in the default filter but render tinted
    /// (see `lifecycle_dot_color`). `LifecycleState` is
    /// `#[non_exhaustive]` so the match falls through to the default
    /// (active-but-not-preferred) bucket for any future variant.
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
            (_, L::Obsolete) => false,
            _ => false,
        }
    }
}

impl Default for LifecycleFilter {
    fn default() -> Self {
        Self::ActiveAndPreferred
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
    /// In-flight `+ Class` form — `Some` while the user is typing
    /// a new class key + label. Confirm dispatches
    /// `update_library_classes` with the appended row.
    pub adding_class: Option<NewClassDraft>,
    /// In-flight class rename — `(original_key, key_buffer, label_buffer)`.
    pub renaming_class: Option<(String, String, String)>,
    /// Most recent class-edit error (validation, duplicate key, etc).
    pub class_error: Option<String>,
}

/// `+ Class` inline form state — separate key + label inputs since
/// the canonical class identifier (`key`) is distinct from the
/// human-readable display label.
#[derive(Debug, Clone, Default)]
pub struct NewClassDraft {
    pub key: String,
    pub label: String,
    pub error: Option<String>,
}

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
            edit_modal: None,
            cell_edit: HashMap::new(),
            delete_confirm: None,
            sort_by: None,
            adding_table: None,
            delete_error: None,
            renaming_table: None,
            rename_error: None,
            adding_class: None,
            renaming_class: None,
            class_error: None,
        }
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
    /// Live edit buffer for the comma-separated tags row (Stage 18).
    /// Persisted to `parameters["tags"]` as `ParamValue::Text` on Save.
    /// Seeded from the existing `parameters["tags"]` value at modal open.
    pub tags_buf: String,
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
        // Seed the tags buffer from `parameters["tags"]` if present;
        // tags live as a free-form `ParamValue::Text` keyed by "tags"
        // (plan §6).
        let tags_buf = match draft.parameters.get("tags") {
            Some(signex_library::ParamValue::Text(s)) => s.clone(),
            Some(other) => other.display(),
            None => String::new(),
        };
        Self {
            address,
            draft,
            param_buf,
            tags_buf,
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
    /// The user picks for the row currently selected in the Library
    /// Browser tab — apply directly through the adapter and refresh.
    /// F15 (2026-05-03): row binding lives next to the row, not in a
    /// separate Component Preview tab or modal.
    BrowserRow(EditorAddress),
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
    /// "Library Options" modal state — `None` while closed. Opens
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
    /// Set of schematic paths the user explicitly skipped updates on
    /// — drives the persistent "Library Updates" status-bar indicator.
    /// Cleared on apply / re-scan / close-tab.
    #[allow(dead_code)]
    pub skipped_updates_for: std::collections::HashSet<PathBuf>,
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
    /// Components Panel UI state — collapsed sections + substring
    /// filter. Stage 9 ships the simple substring matcher; the full
    /// search syntax (plan §5) is polish work.
    pub components_panel: ComponentsPanelState,
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
}

impl ComponentsMountSource {
    pub fn label(self) -> &'static str {
        match self {
            Self::Project => "Project",
            Self::Installed => "Installed",
            Self::Global => "Global",
        }
    }

    /// Section key — lower-case identifier used for the
    /// `ComponentsPanelToggleSection` message + panel-state field
    /// dispatch.
    pub fn key(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Installed => "installed",
            Self::Global => "global",
        }
    }

    /// Every section in stable display order — Project on top,
    /// Installed in the middle, Global last.
    pub const ORDER: &'static [ComponentsMountSource] = &[
        ComponentsMountSource::Project,
        ComponentsMountSource::Installed,
        ComponentsMountSource::Global,
    ];
}

/// Per-source collapse + filter state for the Components Panel.
/// Persists across panel re-renders but not across app restarts —
/// cheap session-scoped UI flags.
#[derive(Debug, Clone, Default)]
pub struct ComponentsPanelState {
    /// `true` while the named section is collapsed. Sections are
    /// `"project"`, `"installed"`, `"global"`.
    pub collapsed_project: bool,
    pub collapsed_installed: bool,
    pub collapsed_global: bool,
    /// Substring filter applied to mpn / manufacturer / internal_pn
    /// / library name. Empty = show everything. Stage 9 uses a
    /// case-insensitive `contains` match across all three fields;
    /// the rich `mpn:LM317 lifecycle:preferred` syntax (plan §5) is
    /// a follow-up.
    pub filter: String,
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

/// State for the "Library Options" modal that pops up between the
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
}

mod methods;
mod preview;
#[cfg(test)]
mod tests;

pub use methods::*;
pub use preview::*;
