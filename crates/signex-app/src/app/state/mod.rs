use std::path::PathBuf;

use signex_types::project::ProjectData;

use crate::dock::DockArea;

use super::TabInfo;

mod agent_edits;
mod board_3d;
mod board_net_classes;
mod board_project;
mod board_sketch;
mod board_sketch_dxf;
mod bom_preview;
#[cfg(test)]
mod case_fold_tests;
pub(crate) mod check_runs;
pub(crate) mod cross_select;
pub(crate) mod dir_case;
mod dirty;
#[cfg(test)]
mod dirty_tests;
mod document;
mod document_load;
#[cfg(test)]
mod draft_ownership_tests;
mod error_notice;
#[cfg(test)]
mod error_notice_tests;
mod fast_grids;
mod interaction;
mod keymap_recorder;
mod keymap_status;
pub(crate) mod library_entry;
mod link_policy;
pub(crate) mod navigator_job;
pub(crate) mod net_class_styles;
pub(crate) mod net_nav;
mod omitted_page;
pub(crate) mod ownership_index;
mod parameter_table;
mod path_key;
mod pcb_board;
mod pcb_drc;
pub(crate) mod pcb_edit;
pub(crate) mod pcb_query;
mod pcb_ratsnest;
mod pcb_revision;
mod pdf_dialog;
mod pdf_page_setup;
mod pdf_preview_raster;
mod placement;
mod preview;
mod profile_update;
pub(crate) mod project_case;
pub(crate) mod project_fs;
pub(crate) mod project_hierarchy;
mod reference_policy;
pub(crate) mod rule_profile_ui;
pub(crate) mod rules_history;
pub(crate) mod scope;
pub(crate) mod sheet_change;
mod sheet_read;
pub(crate) mod sheet_refs;
mod tab_placements;
mod ui;
pub(crate) mod volume_case;
pub(crate) mod window_hierarchies;
mod window_scales;
mod window_sizes;
mod window_tab;

pub use crate::app::core::case_fold::CaseFold;
pub use crate::app::core::document_times::DocumentTimes;
pub use agent_edits::AgentEdits;
pub use board_3d::{Board3dSession, BuiltScene, CaptureResult, MatteBackdrop};
pub use board_net_classes::BoardNetClassPanelState;
pub use board_sketch::BoardSketchSession;
pub use board_sketch_dxf::{
    DXF_LAYER_TARGETS, DxfImportMode, DxfLayerTarget, SketchDxfDialog, SketchDxfMsg,
    SketchDxfProblem, SketchDxfResult, SketchDxfStep,
};
pub use bom_preview::{BomIssueLine, BomPreviewState, BomSidebarTab, ColumnResizeState};
pub(crate) use document_load::SheetInstall;
pub use error_notice::{
    ErrorNotice, NoticeAction, NoticeSeverity, disk_changed_choices, untranslated, verbatim,
};
pub use interaction::{ClickedPin, InteractionState};
pub use keymap_recorder::KeymapRecorderState;
pub use keymap_status::KeymapStatus;
pub use omitted_page::{OmittedPage, OmittedReason};
pub use parameter_table::{ClosedSheetCopies, ParameterCell, ParameterTableState};
pub(crate) use pcb_drc::DrcInput;
pub(crate) use pcb_ratsnest::{PcbRatsnest, RatsnestKey, RouteRatsKey};
pub use pdf_dialog::{ANGLE_PRESETS, PdfDialogState, PdfSection, WatermarkPreset};
pub use pdf_page_setup::PageSetupInputs;
pub use pdf_preview_raster::{
    PageSetupMemory, RasterOutcome, RasterQueue, RasterSource, RememberedPageSetup,
    forget_page_setup, rekey_page_setup,
};
pub use placement::{
    LastPlacement, PasteCounters, PlacementKind, PlacementSession, SheetPlacementPause,
};
pub use preview::{PdfQuality, PreviewState};
pub use profile_update::PendingProfileUpdate;
pub(crate) use sheet_read::SheetUnreadable;
pub use sheet_read::UnreadableCause;
pub use tab_placements::{TabPlacements, default_placement};
pub use ui::{DEFAULT_BOTTOM_PANEL_HEIGHT, UiState};

pub struct Signex {
    pub ui_state: UiState,
    pub document_state: DocumentState,
    pub interaction_state: InteractionState,
    /// v0.9 Library subsystem state. Borrowed independently of
    /// `document_state` so the library dispatcher can mutate it
    /// without colliding with schematic / PCB engine borrows.
    pub library: crate::library::LibraryState,
    /// The macOS system menu bar built from the menu model; inert elsewhere
    /// (docs/MENU_BAR.md, "Native macOS menu").
    pub(crate) native_menu: crate::menu_bar::native::NativeMenuState,
    /// The local agent listener's runtime state (#1924); inert while the
    /// "Allow agent connections" preference is off.
    pub(crate) agent: crate::app::agent::AgentState,
    /// The pan/zoom frame bench (#2472); `None` unless the process was
    /// started with `SIGNEX_BENCH_PAN_ZOOM` set.
    pub(crate) pan_zoom_bench: Option<crate::pan_zoom_bench::PanZoomBench>,
    /// The extensions the binary installed (#2495); none in Community.
    pub(crate) extensions: crate::extension::ExtensionHost,
}

/// Role of a non-main window opened by Signex. Phase 2 adds detached
/// modals; Phase 3 adds `UndockedTab(tab_index)` so a schematic sheet
/// can live in its own OS window.
#[derive(Debug, Clone)]
pub enum WindowKind {
    DetachedModal(ModalId),
    /// Undocked document tab. Stores the tab's file path (unique per
    /// open tab in Signex) so the mapping survives tab reordering or
    /// unrelated tabs closing. The `title` copy is used as the OS
    /// window title without re-reading tabs.
    UndockedTab {
        path: std::path::PathBuf,
        title: String,
    },
    /// Detached dock panel. Opened automatically when the user drags a
    /// floating panel past the main window edge. Closing the OS window
    /// reattaches the panel to its last dock region.
    DetachedPanel(crate::panels::PanelKind),
    /// v0.9-refactor-2 Component Preview — one window per open row.
    /// The preview's full state lives in `Signex::library.editors`
    /// keyed by `EditorAddress(library_path, table, row_id)`.
    ComponentEditor {
        library_path: std::path::PathBuf,
        table: String,
        row_id: signex_library::RowId,
    },
}

/// Kind of z-order picker currently armed. Drives the first-click
/// resolve in `handle_canvas_left_click`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReorderPicker {
    /// Move selection to render just above the clicked reference.
    Above,
    /// Move selection to render just below the clicked reference.
    Below,
}

/// Custom net-colour picker state (Active Bar → Net Color → Custom, or a
/// palette row's Custom…).
#[derive(Debug, Clone)]
pub struct NetColorCustomState {
    pub show: bool,
    pub draft: iced::Color,
    /// The draft when the picker opened: Cancel puts it back (#2770).
    pub draft_at_open: iced::Color,
    /// The net the picker colours when it was opened from a palette row;
    /// `None` arms the net-colour pen, as the Active Bar entry does.
    pub net: Option<String>,
    /// The sheet `net` was read on. Use Color refuses when another sheet
    /// is active, so a same-named net elsewhere is never coloured.
    pub sheet: Option<PathBuf>,
    /// The Hex field while the user types in it; `None` shows the draft.
    /// Text that does not parse stays here and the field shows an error.
    pub hex: Option<String>,
}

impl Default for NetColorCustomState {
    fn default() -> Self {
        Self {
            show: false,
            draft: iced::Color::from_rgb(0.40, 0.40, 0.93),
            draft_at_open: iced::Color::from_rgb(0.40, 0.40, 0.93),
            net: None,
            sheet: None,
            hex: None,
        }
    }
}

/// F5 net colour palette state (#2528): the filter text and the row whose
/// chooser is open.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NetColorPaletteState {
    pub filter: String,
    /// The net label text whose chooser is open beside its row.
    pub chooser: Option<String>,
    /// The toast after a Clear, while it shows (mockup row 71).
    pub toast: Option<NetColorToast>,
}

/// The net colour palette's toast after a Clear, with its Undo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetColorToast {
    pub cleared: NetColorsCleared,
    /// The sheet the clear was made on.
    pub sheet: std::path::PathBuf,
    /// The sheet's engine revision right after the clear. Unique per edit,
    /// so it also tells this toast's timeout from an older one's.
    pub revision: u64,
}

/// What a palette toast says was cleared.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetColorsCleared {
    /// Clear all colours took the colour off this many listed nets (named by
    /// a label or a power port).
    Nets(usize),
    /// Clear all colours took colours off nets that no label or power port
    /// names.
    Unlabelled,
    /// A row chooser's Clear took the colour off this net.
    Net(String),
}

/// Transient state for the Move Selection dialog.
/// Deltas are stored as strings so mid-edit partial values (`-`, `2.`)
/// don't panic through number parsing; the Apply handler parses them.
#[derive(Debug, Clone, Default)]
pub struct MoveSelectionState {
    pub open: bool,
    pub dx: String,
    pub dy: String,
    /// The sheet the dialog was opened on: Apply moves that sheet's
    /// selection, not whichever sheet the main window shows (#1910).
    pub sheet: Option<std::path::PathBuf>,
}

pub(crate) use crate::app::core::drawing_field::LiveDrawingEdit;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModalId {
    AnnotateDialog,
    AnnotateResetConfirm,
    /// Re-prefix to standard (#1315): the list of changes, to confirm.
    ReprefixConfirm,
    ErcDialog,
    /// Altium-style Move-Selection dialog — numeric ΔX / ΔY inputs.
    MoveSelection,
    /// F5 net-color palette.
    NetColorPalette,
    /// Parameter manager — bulk parameter editor.
    ParameterManager,
    /// Import Parameters (#1295): the changes a CSV file makes, to apply.
    /// Like the Parameter Manager it only ever opens as its own window.
    ParameterImport,
    // Reserved for future draggable modals — wired in when each dialog's
    // header gets a drag hook.
    Preferences,
    FindReplace,
    /// Rename-sheet dialog (Projects-panel leaf → Rename...).
    RenameDialog,
    /// Remove-from-project dialog (Projects-panel leaf → Remove from Project).
    RemoveDialog,
    /// Print Preview / Export PDF unified modal (File → Print Preview, File → Export PDF).
    PrintPreview,
    /// BOM Export preview modal (File → Export → Bill of Materials…).
    BomPreview,
    /// Enable Version Control confirm modal (Projects-panel root →
    /// Enable Version Control...).
    EnableVersionControl,
    /// v0.18.11 — Cartesian Grid Editor modal (Ctrl+G in a
    /// footprint editor).
    GridProperties,
    /// v0.18.14.1 — Custom Selection Filter modal (8-row checkbox
    /// table; opens from the Properties `Custom…` button).
    SelectionFilterCustom,
    /// File ▸ New Project: the setup dialog (#2277).
    NewProject,
    /// Help ▸ About Signex.
    About,
    /// The keyboard-shortcuts reference sheet (F1).
    KeyboardShortcuts,
    /// Tools ▸ Passive Network Calculator.
    PassiveCalculator,
    /// The first-run onboarding tour.
    FirstRunTour,
    /// "Create Sheet": where to write the file of a sheet symbol (confirm
    /// class).
    CreateChildSheet,
    /// The error card (confirm class): one at a time.
    ErrorNotice,
    /// The netlist-incomplete "export anyway?" prompt (confirm class).
    NetlistIncompletePrompt,
    /// The autosave recovery prompt (confirm class).
    AutosaveRecovery,
    /// "Close Project - Unsaved Edits" (confirm class, on the data-loss path).
    ProjectCloseConfirm,
    /// "Exit Signex - Unsaved Edits" (confirm class, on the data-loss path).
    AppQuitConfirm,
    // Library dialogs. Edit row and delete confirm are bound to ONE library
    // browser per window (`UiState::library_modal_owners`); the rest are
    // single-instance.
    LibraryPicker,
    LibraryEditRow,
    LibraryPrimitivePicker,
    LibraryDocumentOptions,
    LibraryCreateOptions,
    /// Which library receives a copy of a stock primitive (#813).
    LibraryStockCopy,
    LibraryUpdates,
    /// Close library with unsaved drafts (confirm class).
    LibraryCloseConfirm,
    /// Delete the selected library row (confirm class).
    LibraryDeleteConfirm,
    /// Library recovery (confirm class); never auto-decided.
    LibraryRecovery,
    /// The custom net colour picker.
    NetColorCustom,
}

pub use signex_types::schematic::AnnotateOrder;

pub use crate::app::core::project_id::ProjectId;

/// One loaded project in the multi-project workspace. `path` is the
/// canonical identity (`.snxprj` location on disk); `data`
/// is the parsed project contents. Multiple projects with different
/// `path`s coexist in `DocumentState.projects`; two identical `path`s
/// at once is a loader bug (existing `open_project_file` de-dupes).
#[derive(Debug, Clone)]
pub struct LoadedProject {
    pub id: ProjectId,
    pub path: PathBuf,
    pub data: ProjectData,
    /// Libraries the user has authored via the New Library flow but not
    /// yet committed to disk. The New Library dialog's Create button
    /// only registers an entry here + flips the project dirty bit;
    /// `commands::create_pending_library` runs at project-save time
    /// to actually write the `.snxlib`. Closes
    /// `feedback_no_disk_writes_without_user_save.md`'s "wait for
    /// explicit user save" invariant. Keyed by a temporary handle that
    /// becomes the eventual `library_id` once materialised.
    pub pending_libraries:
        std::collections::HashMap<uuid::Uuid, crate::library::commands::PendingLibrarySpec>,
    /// Path-key rules for this project's volume, probed once off-thread
    /// after load (#694); the host default until the answer arrives.
    pub case_fold: scope::CaseFold,
    /// The volume probe has reported; the first answer wins.
    pub case_probed: bool,
    /// The probe DECIDED (not undecided, failed or a stat error): only then is
    /// `case_fold` a fact about the volume rather than the host default, and
    /// only then does document identity trust it (#1447).
    pub case_decided: bool,
}

impl LoadedProject {
    /// The project's directory — the *one* convention for "where this
    /// project's relative sheet filenames resolve from".
    ///
    /// Two conventions used to coexist: the parent of the `.snxprj` on disk,
    /// and the persisted `data.dir` string. The loader patches `data.dir` to
    /// the opened path's parent, so they agree for a freshly-loaded project —
    /// but a `LoadedProject` assembled any other way (project creation, a
    /// `.snxprj` that recorded an absolute `dir` that no longer matches) can
    /// have them disagree, and then ownership resolution succeeds while the
    /// sheet paths it produces do not exist. The export skips those pages
    /// silently. `path.parent()` is the only one that cannot be stale: it is
    /// derived from the file actually opened.
    pub fn dir(&self) -> &std::path::Path {
        self.path
            .parent()
            .unwrap_or_else(|| std::path::Path::new(""))
    }
}

/// #431 — pending "Export anyway (incomplete)?" prompt for netlist export.
///
/// Raised (instead of a dead-end error) when `Tools ▸ Reports ▸ Netlist` derives a
/// netlist that does not cover the whole project. Holds the picked `save_path`
/// and the omitted-page `messages` so the two prompt actions can act without
/// re-deriving: "Export anyway" writes the partial `.net` (a clean file —
/// `.NET` has no comment syntax) and logs these messages as a warning;
/// "Cancel" writes nothing. The
/// refusal stays the default — nothing reaches disk until the user acts.
#[derive(Debug, Clone)]
pub struct NetlistIncompletePrompt {
    /// Where the file dialog said to write the `.net`.
    pub save_path: PathBuf,
    /// One user-facing line per omitted / uncovered page — the same
    /// `ExportIssues::messages()` logged as a warning when the user exports
    /// anyway.
    pub messages: Vec<String>,
    /// The export scope snapshotted when the prompt was raised. "Export anyway"
    /// writes from THIS, never a fresh re-derivation — so the bytes on disk and
    /// the warning (drawn from `messages`, the same derivation) always
    /// describe the same project state, even if the document changes while the
    /// modal is up (#431 review).
    pub ctx: signex_output::ExportContext,
    /// The prompt's list: one row per page left out of the netlist, from the
    /// same derivation as `messages` (`ExportIssues::omitted_pages`).
    pub omitted: Vec<OmittedPage>,
}

/// One printed page copy in the export file picker: the sheet file plus the
/// sheet placement it is a copy of (root for a page printed as itself). A
/// child placed twice is two picker rows with one path, so the path alone
/// cannot key a tick.
pub type PageKey = (PathBuf, signex_types::schematic::InstancePath);

/// What the preview modal chose for one PDF export, for the document it was
/// chosen on. Carried inside `ExportMsg::PdfFinished`, so each Export click
/// owns its own copy and overlapping exports cannot clobber each other (#852).
#[derive(Debug, Clone)]
pub struct PdfExportRequest {
    /// The document the export was started from.
    pub source: PathBuf,
    pub options: signex_output::PdfOptions,
    /// Sheet paths to include, from the preview's file picker. An empty set
    /// (after a Clear) means "no files chosen" and the export is rejected.
    pub files: std::collections::HashSet<PageKey>,
    /// The sheet placement `source` was descended into when the export was
    /// requested (#1133), read then because the dialog can outlive the tab.
    pub descended_placement: Option<signex_types::schematic::InstancePath>,
}

pub struct DocumentState {
    /// Document ids a document cannot say itself: project files not loaded,
    /// and the order documents were first seen in (#2172).
    pub document_ids: crate::app::document_ids::DocumentIds,
    /// Counts the changes that can alter an ERC finding: an edit to a sheet
    /// (through the engine, or Annotate, Replace, library update and the like),
    /// a project-file change, and a sheet written behind the engines. Each calls
    /// [`DocumentState::note_sheet_edit`], mostly via `mark_document_dirty`.
    /// ERC compares it with the count its results were read at (#1768).
    pub sheet_edit_epoch: u64,
    /// Symbol editors opened so far; see [`DocumentState::next_symbol_history_base`].
    pub symbol_editors_opened: u64,
    pub dock: DockArea,
    /// Change detection + debounce for persisting `dock` (#692). The
    /// write itself runs as a `Task`; see `handlers/dock/layout_persist.rs`.
    pub dock_layout_sync: crate::dock::DockLayoutSync,
    /// The Components filter + Library Browser queries and the debounce
    /// that persists them (#859); see `handlers/dock/search_prefs_persist.rs`.
    pub search_prefs: crate::persist::SearchPrefsSync,
    /// Off-thread document writes: per-path ordering, in-flight tracking and
    /// the quit barrier (#859); see `handlers/document_files/save_async.rs`.
    pub document_saves: crate::persist::DocumentSaves,
    /// A Save All's settle that ran while rows of its batch were still
    /// writing, by batch id; resumed by the last of those writes (#1728).
    pub parked_save_all_settles:
        std::collections::HashMap<u64, crate::app::contracts::SaveAllSettle>,
    /// Opt-in recovery sidecars of dirty schematics (#104); see `crate::autosave`.
    pub autosave: crate::autosave::AutosaveState,
    /// Disk facts behind the project tree's `(missing)` marks (#111).
    pub project_fs: project_fs::ProjectFsCache,
    /// Idle check of open documents against the disk (#1877).
    pub disk_poll: crate::app::handlers::document_files::DiskPoll,
    /// Off-thread Navigator hierarchy assembly (#1470).
    pub navigator_job: navigator_job::NavigatorJob,
    /// Hierarchies of the other projects undocked windows show (#2021).
    pub window_hierarchies: window_hierarchies::WindowHierarchies,
    /// Cached sheet-path -> owning-project index, rebuilt when its inputs
    /// change (#1470); see `state/ownership_index.rs`.
    pub(crate) ownership: std::cell::RefCell<Option<ownership_index::OwnershipIndex>>,
    /// Projects already told (once per open) that an ERC run fell back to this
    /// machine's preferences because they store no rule set (#1085).
    pub erc_fallback_noted: std::collections::HashSet<ProjectId>,
    /// Projects already told (once per open) that the profile their rules
    /// were seeded from has changed locally (#1085).
    pub profile_drift_noted: std::collections::HashSet<ProjectId>,
    /// Undo / redo of each project's stored rule set edits (#1085).
    pub rules_history: std::collections::HashMap<ProjectId, rules_history::RulesHistory>,
    /// Undo / redo of each project's General settings (engine-owned).
    pub general_history: std::collections::HashMap<ProjectId, signex_engine::ProjectGeneralHistory>,
    /// How many undo steps every history keeps (`undo_depth`); see
    /// [`Signex::set_undo_depth`](crate::app::Signex::set_undo_depth).
    pub undo_depth: usize,
    /// Which font families the Messages panel has already said an open
    /// document names but this machine lacks (#510), so each is reported
    /// once per document.
    pub missing_fonts: crate::fonts::document_report::MissingFontReporter,
    /// Engines (`Engine::instance_id`) already told, once per open, that an
    /// edit was refused because the sheet is read-only (#1402).
    pub read_only_refusal_noted: std::collections::HashSet<u64>,
    pub tabs: Vec<TabInfo>,
    pub active_tab: usize,
    /// All live schematic engines keyed by their on-disk path. Every
    /// open schematic tab — whether it's the active one in the main
    /// window or parked in an undocked-tab window — has a live entry
    /// here. `active_path` names which entry `active_engine()` resolves
    /// to; undocked windows look up their own entry via
    /// `engine_for_window`. Save-as rekeys an entry via
    /// `rekey_engine(old, new)`.
    pub engines: std::collections::HashMap<PathBuf, signex_engine::Engine>,
    /// The clock and the file times the computed special strings show.
    pub times: DocumentTimes,
    /// Per-tab state for open `.snxsym` document tabs. Keyed by the
    /// file path stored on `TabInfo.path` for matching
    /// `TabKind::SymbolEditor(path)` tabs. Insert on
    /// `Signex::open_primitive`; drop in
    /// `close_tab_at_index` alongside the engine cleanup.
    pub symbol_editors: std::collections::HashMap<PathBuf, super::SymbolDocument>,
    /// Per-tab state for open `.snxfpt` document tabs. Keyed the same
    /// way as `symbol_editors`.
    pub footprint_editors: std::collections::HashMap<PathBuf, super::FootprintDocument>,
    /// v0.26-E — process-local clipboard for footprint-editor pad
    /// Cut / Copy / Paste. Survives tab switches so a pad copied from
    /// one footprint can be pasted into another. `None` until the
    /// first Cut / Copy. Replaced wholesale on every copy. Keyed by
    /// no path on purpose — Altium parity is single-slot, last-write-
    /// wins.
    pub pad_clipboard: Option<crate::library::editor::footprint::state::EditorPad>,
    /// The path of the schematic the main window is currently editing.
    /// `active_engine()` reads `engines.get(active_path)`. `None` means
    /// no schematic tab is active (e.g. a PCB tab is active, or nothing
    /// is open).
    pub active_path: Option<PathBuf>,
    /// Every loaded project in the workspace. Order = load order. First
    /// project becomes active on load; subsequent opens append.
    pub projects: Vec<LoadedProject>,
    /// Which project is "active" for handlers that operate on the workspace
    /// at large (save-all, the Projects panel). Currently tracks the
    /// most-recently-loaded project plus whichever project contains the
    /// active tab; single source of truth for "where am I focused".
    ///
    /// Sticky by design: focusing a tab with no project of its own leaves it
    /// pointing at the last-loaded project. Anything scoped to the *document
    /// set the user is working in* — export, ERC, annotate, child-sheet
    /// resolution — must use [`DocumentState::active_document_project`]
    /// instead, or it silently operates on another project's sheets (#406).
    pub active_project: Option<ProjectId>,
    /// Project paths (or loose root sheets) whose per-placement-designator
    /// load note was already started, so it appears once per load, not once
    /// per opened sheet.
    pub instance_note_scopes: std::collections::HashSet<std::path::PathBuf>,
    /// A schematic tab just opened: the next `finish_update` starts the
    /// per-placement designator note (a no-op once its scope was noted).
    pub instance_note_due: bool,
    /// Schematic tabs opened since the last `update` finished: each needs
    /// its earlier-run recovery copy checked (#1208). `open_schematic_tab`
    /// queues the path, `update` starts the check.
    pub recovery_check_due: Vec<std::path::PathBuf>,
    /// Dirty documents that no editor draft holds; see `dirty.rs`, the only
    /// reader and writer. Ask `is_document_dirty`, never this field.
    pub(crate) dirty_docs: dirty::DirtyDocuments,
    /// Undo/redo of each project's net-class table (#1042). The table is
    /// project data, so it has its own history beside the per-sheet engines.
    pub net_class_history: std::collections::HashMap<ProjectId, signex_engine::NetClassHistory>,
    /// Project tab state per loaded project (#1202): selected section and
    /// the General history. Outlives the tab, dropped with the project.
    pub project_tabs: std::collections::HashMap<ProjectId, crate::app::ProjectTabState>,
    /// Monotonic counter used to mint `ProjectId` on load. Never reused —
    /// closing a project does not free its id (tabs may hold stale refs,
    /// which we detect by resolving through `projects` and treating None
    /// as "no longer loaded").
    pub next_project_id: u32,
    /// Monotonic counter minting the raw value of [`signex_engine::UndoGroupId`]s for
    /// project-wide undo steps (an annotate run under "One step for the whole
    /// project"). Never reused.
    pub next_undo_group: u64,
    /// Bumped whenever a document is opened, saved or closed, when a project
    /// closes, and when a rename, delete or project-folder move starts and
    /// finishes. A background job that read sheets from disk records it first
    /// and applies its result only if it is unchanged, so it never installs
    /// bytes a later open, save, close, move or delete has already superseded.
    pub documents_epoch: u64,
    /// Designators used on the sheets of the project a part is placed in,
    /// scanned off-thread; see `designator_scan.rs`.
    pub(crate) designator_cache: crate::app::designator_scan::DesignatorCache,
    /// Highest board revision any engine reached; see `state/pcb_revision.rs`.
    pub next_pcb_revision: u64,
    /// Board revision per PCB path (mirror of its engine); a save marks clean only if it matches.
    pub pcb_revisions: std::collections::HashMap<PathBuf, u64>,
    /// The board engine of each open PCB tab, by path, like `engines`; see
    /// `state/pcb_board.rs`, the only reader and writer.
    pub(crate) boards: std::collections::HashMap<PathBuf, signex_engine::pcb::BoardEngine>,
    /// The engines of PCB tabs closed while dirty; see `state/pcb_board.rs`.
    pub(crate) parked_pcb_boards: pcb_board::ParkedBoards,
    /// The sheets each undo group touched. Undo/redo of a
    /// grouped entry consults this to find the sheets that must move with it.
    pub undo_groups:
        std::collections::HashMap<signex_engine::UndoGroupId, crate::app::UndoGroupSheets>,
    /// Power rail renames waiting on their project's sheets (mockup row 45).
    pub(crate) rail_rename: crate::app::core::rail_rename::RailRenames,
    /// An agent's edit list in progress, and the labels of its undo steps (#2178).
    pub agent_edits: AgentEdits,
    /// The last runs of `check_erc`, for a later run's baseline (#2199).
    pub(crate) check_runs: check_runs::CheckRuns,
    /// Each open project's design reuse blocks, read or saved (#2410).
    pub(crate) reuse_blocks: crate::app::board_reuse::ReuseBlockStore,
    /// The one store of half-typed title-block and paper-edge text (#2027);
    /// every panel context holds a handle to it.
    pub drafts: crate::panels::PanelDrafts,
    /// The drawing field being typed, if any (#2768).
    pub(crate) live_drawing_edit: Option<LiveDrawingEdit>,
    /// The selection the Line Width box's text was typed under (#2767).
    pub(crate) line_width_draft_selection: Option<uuid::Uuid>,
    pub panel_ctx: crate::panels::PanelContext,
    /// The panel context of each undocked document window, so its dock
    /// panels show and edit that window's sheet (#1966).
    pub window_panel_ctx: std::collections::HashMap<iced::window::Id, crate::panels::PanelContext>,
    /// What each window context was last built from; see `window_ctx_key`.
    pub(crate) window_panel_ctx_keys:
        std::collections::HashMap<iced::window::Id, crate::app::runtime::WindowCtxKey>,
    /// Print-preview overlay state. `Some` while the preview dialog is
    /// open. Doubles as the unified PDF Export modal — `File → Export
    /// PDF` and `File → Print Preview` both populate this field.
    pub preview: Option<PreviewState>,
    /// Page setup (size, orientation, margins, scale) each document last used
    /// in the PDF export modal, restored when it reopens for the same
    /// document. Session-only; never written to disk.
    pub page_setup_memory: PageSetupMemory,
    /// The sheet symbol each child tab was descended through (#1133).
    pub tab_placements: TabPlacements,
    /// Sheet history, net highlight, hover insight and the project net index
    /// they read (#1270, #1271, #1272).
    pub net_nav: net_nav::NetNavState,
    pub compiled_design: crate::app::core::compiled_design::CompiledDesigns,
    /// Generation counter and in-flight job of the preview renderer.
    pub raster_queue: RasterQueue,
    /// The app's one user-visible error card. `Some` while it is shown;
    /// cleared by `OverlayMsg::DismissErrorNotice`.
    ///
    /// Not export-only, despite the heading it used to hardcode: the
    /// project-close path already raised it for "could not save N
    /// file(s)", so a failed close announced itself to the user as
    /// "Export Failed" (#532). The title now travels with the message.
    pub error_notice: Option<ErrorNotice>,
    /// #431 — pending "Export anyway (incomplete)?" prompt. `Some` while the
    /// netlist-incomplete modal is up, waiting on the user's choice. Set by
    /// `handle_export_netlist_finished` when the derived netlist has a hole;
    /// cleared by both prompt actions (Export anyway / Cancel).
    pub netlist_incomplete_prompt: Option<NetlistIncompletePrompt>,
    /// BOM preview state. `Some` while the BOM Export modal is open.
    /// Mirrors the Print Preview pattern: the user adjusts grouping /
    /// include flags / format / variant in the modal and clicks
    /// Export to drive the file picker with the chosen options.
    pub bom_preview: Option<BomPreviewState>,
    /// Right-dock History panel state — current generation counter,
    /// last loaded entries, the path the load was issued for. Driven
    /// by `Message::HistoryLoaded` and re-targeted on tab switch.
    pub history: crate::panels::history::HistoryPanelState,
    /// The History panel state of each undocked document window, swapped in
    /// with its panel context so the window shows and restores its own file
    /// (#2067).
    pub(crate) window_history:
        std::collections::HashMap<iced::window::Id, crate::panels::history::HistoryPanelState>,
    /// v0.23 — Async git commit work queue. Save handlers push tuples
    /// here; `finish_update` drains them into `Task::perform` calls
    /// that run the actual commit on a tokio `spawn_blocking`. Each
    /// completion routes through `Message::Project(ProjectMsg::GitCommitDone)`.
    pub pending_git_commits: Vec<PendingGitCommit>,
    /// v0.23 — Set of `(project_root, rel_path)` pairs whose commits
    /// are currently queued or in flight. Drives the status bar's
    /// "Saving…" pill — when non-empty the user sees an indicator.
    /// An entry lands here as soon as
    /// [`Signex::commit_save_to_project_git`] enqueues the work and
    /// clears on `Message::Project(ProjectMsg::GitCommitDone)`.
    pub inflight_git_commits: std::collections::HashSet<(PathBuf, PathBuf)>,
    /// The subset of `inflight_git_commits` whose commit reads the working
    /// file when its worker starts instead of holding the bytes it records
    /// (primitive-editor saves, project-file saves, renames). A History
    /// restore of such a file waits, or the commit would record the restored
    /// bytes under its own label; commits that carry their bytes never block it.
    pub unbound_git_commits: std::collections::HashSet<(PathBuf, PathBuf)>,
    /// The newest save sequence queued for a commit, per `(project_root,
    /// rel_path)`; a commit of an older write is dropped.
    pub git_commit_sequences: std::collections::HashMap<(PathBuf, PathBuf), u64>,
    /// #833 — repo roots with a commit currently running. At most one
    /// commit per repo is in flight; the rest wait in
    /// `pending_git_commits` so concurrent tasks never race on the
    /// repo's index lock. Cleared per repo by `GitCommitDone`.
    pub active_git_repos: std::collections::HashSet<PathBuf>,
    /// #1205 — project folders whose auto-commit the guard refused to
    /// initialise; each is announced once, not on every save.
    pub paused_git_projects: std::collections::HashSet<PathBuf>,
    /// #478 review — paths whose async schematic/PCB read+parse
    /// (`open_schematic_file` / `open_pcb_file`) has been spawned but
    /// hasn't completed yet. Without this, firing the same open twice
    /// before the first `Task::perform` resolves races two tabs into
    /// existence aliasing one `engines` entry; closing either then
    /// orphans the other. Inserted right before the `Task::perform` is
    /// spawned; cleared in both the `Ok` and `Err` arms of
    /// `FileMsg::SchematicOpenFinished` / `FileMsg::PcbOpenFinished`.
    pub pending_opens: crate::app::document_read::InFlightReads,
    /// What the directory case probes of loose documents reported, and the
    /// opens and saves waiting on one (#1447).
    pub dir_case: dir_case::DirCases,
    /// #859 — History restores running on a worker, keyed by the document
    /// path. A second restore click for the same document is ignored until
    /// the first completes; closing the project or all documents cancels it.
    pub pending_restores: crate::app::document_read::InFlightReads,
    /// The repository a History branch switch is checking out. Saves and
    /// restores of its files are refused until the switch reports back.
    pub branch_switch: Option<PathBuf>,
    /// #859 — renames and deletes whose disk work runs on a worker. Saves,
    /// opens, restores and second moves of an overlapping path are refused
    /// until the completion message drops the entry.
    pub pending_path_ops: crate::app::path_ops::PendingPathOps,
    /// New Projects whose folder and files a worker is still writing, by the
    /// chosen `.snxprj`. A quit is held until they land (#2280).
    pub new_project_writes: std::collections::HashSet<PathBuf>,
    /// The Altium import still running, if any. A second import is
    /// refused while one runs: two converters writing the same output
    /// folder would interleave.
    pub altium_import_running: Option<AltiumImportRun>,
    /// Projects whose folder is being renamed on the blocking pool (#1221).
    /// A second rename of the same project is refused until the first
    /// reports back.
    pub project_folder_renames: std::collections::HashSet<ProjectId>,
    /// Where each write running on the blocking pool lands (an export, Add
    /// New Schematic, the target folder of an Add Existing copy, a new
    /// library file), one entry per write. A folder rename is refused while
    /// one lies under the folder, or the late write would land in the old
    /// folder.
    pub project_writes: Vec<PathBuf>,
    /// Project folders a rename moved away this session. A write a dialog
    /// aimed there before the move is refused, or it would recreate the old
    /// folder as an orphan.
    pub moved_project_dirs: Vec<MovedProjectDir>,
    /// Commits asked for while their project folder moved, run once the move
    /// has finished.
    pub deferred_git_commits: Vec<DeferredGitCommit>,
    /// The Gerber Viewer tab (mockup rows 193 and 193f): the package shown
    /// and the app-wide recent packages. Its slice owns it.
    pub gerber_viewer: crate::app::gerber_viewer::GerberViewerState,
}

/// A project folder a rename moved from `old` to `new`.
#[derive(Debug, Clone)]
pub struct MovedProjectDir {
    pub old: PathBuf,
    pub new: PathBuf,
    pub fold: CaseFold,
}

/// A project commit asked for while the project folder moved; the paths are
/// as they were named then.
#[derive(Debug, Clone)]
pub struct DeferredGitCommit {
    pub project: ProjectId,
    pub file_path: PathBuf,
    pub message: String,
    pub replace_message: bool,
    pub removed_path: Option<PathBuf>,
    /// Whether the commit may carry the agent edit summary (#2253); a restore
    /// never does.
    pub with_summary: bool,
}

/// An Altium import on the blocking pool (#1123).
#[derive(Debug, Clone)]
pub struct AltiumImportRun {
    /// The picked Altium file.
    pub input: PathBuf,
    /// Set by Cancel; the converter loop stops the child when it sees it.
    pub cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl PartialEq for AltiumImportRun {
    fn eq(&self, other: &Self) -> bool {
        self.input == other.input && std::sync::Arc::ptr_eq(&self.cancel, &other.cancel)
    }
}

/// v0.23 — One queued commit for the async git pipeline. Stays
/// resident in `DocumentState.pending_git_commits` until
/// `finish_update` drains it into a `Task::perform`.
#[derive(Debug, Clone)]
pub struct PendingGitCommit {
    pub project_root: PathBuf,
    pub rel_path: PathBuf,
    /// A project-relative path the same commit removes, so a move reads as
    /// a rename in the history (#1219).
    pub removed_rel_path: Option<PathBuf>,
    pub message: String,
    /// The exact bytes to commit for `rel_path`: what a save wrote or a
    /// restore read back. `None` commits the working file as it is when the
    /// worker runs (callers that hold no copy of what they wrote).
    pub content: Option<crate::persist::SharedBytes>,
    /// The project's "Summarise agent changes in the commit message" as it was
    /// when the commit was queued (#2253).
    pub summarise_agent_edits: bool,
}

impl PendingGitCommit {
    /// Whether the worker reads the working file when it runs: it holds no
    /// bytes, or it is a rename commit, which stages the file from disk and
    /// ignores any bytes.
    pub fn reads_working_file(&self) -> bool {
        self.content.is_none() || self.removed_rel_path.is_some()
    }
}

/// The bytes a commit records and the order of the write they came from.
/// Saves of one file can report completion out of order; a commit of an
/// older write must never replace, or queue behind, a newer one's.
#[derive(Debug, Clone)]
pub struct CommitBytes {
    pub bytes: crate::persist::SharedBytes,
    /// The write's position in its document-save ledger; `0` is exempt from
    /// ordering (a restore).
    pub sequence: u64,
}
