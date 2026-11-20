use std::path::PathBuf;

use signex_types::project::ProjectData;

mod agent_edits;
mod board_3d;
mod board_net_classes;
mod board_project;
mod board_sketch;
#[cfg(test)]
mod case_fold_tests;
pub(crate) mod check_runs;
pub(crate) mod cross_select;
pub(crate) mod dir_case;
mod dirty;
#[cfg(test)]
mod dirty_tests;
mod document;
#[cfg(test)]
mod draft_ownership_tests;
mod error_notice;
#[cfg(test)]
mod error_notice_tests;
mod fast_grids;
mod interaction;
mod keymap_recorder;
pub(crate) mod library_entry;
mod link_policy;
pub(crate) mod navigator_job;
pub(crate) mod net_class_styles;
pub(crate) mod net_nav;
pub(crate) mod ownership_index;
mod parameter_table;
mod path_key;
mod pcb_board;
mod pcb_drc;
pub(crate) mod pcb_edit;
pub(crate) mod pcb_query;
mod pcb_ratsnest;
mod pcb_revision;
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
pub use board_3d::{Board3dSession, BuiltScene};
pub use board_net_classes::BoardNetClassPanelState;
pub use board_sketch::BoardSketchSession;
pub use document::{
    AltiumImportRun, CommitBytes, DeferredGitCommit, DocumentState, MovedProjectDir,
    PendingGitCommit,
};
pub use error_notice::{ErrorNotice, NoticeAction, untranslated, verbatim};
pub use interaction::{ClickedPin, InteractionState};
pub use keymap_recorder::KeymapRecorderState;
pub use parameter_table::{ParameterCell, ParameterTableState};
pub(crate) use pcb_drc::DrcInput;
pub(crate) use pcb_ratsnest::{PcbRatsnest, RatsnestKey};
pub use pdf_page_setup::PageSetupInputs;
pub use pdf_preview_raster::{
    PageSetupMemory, PreviewSlot, RasterOutcome, RasterQueue, RasterSource, RememberedPageSetup,
    forget_page_setup, rekey_page_setup,
};
pub use placement::{
    LastPlacement, PasteCounters, PlacementKind, PlacementPause, PlacementSession,
};
pub use preview::{
    BomPreviewState, BomSidebarTab, ColumnResizeState, PdfPreviewTab, PdfQuality, PreviewState,
};
pub use profile_update::PendingProfileUpdate;
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
    /// yet committed to disk. The Library Options modal's Create button
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
/// Raised (instead of a dead-end error) when `Reports ▸ Netlist` derives a
/// netlist that does not cover the whole project. Holds the picked `save_path`
/// and the omitted-page `messages` so the two prompt actions can act without
/// re-deriving: "Export anyway" writes the partial `.net` (a clean file —
/// Protel has no comment syntax) and logs these messages as a warning;
/// "Cancel" writes nothing. The
/// refusal stays the default — nothing reaches disk until the user acts.
#[derive(Debug, Clone)]
pub struct NetlistIncompletePrompt {
    /// Where the file dialog said to write the `.net`.
    pub save_path: PathBuf,
    /// One user-facing line per omitted / uncovered page — the same
    /// `ExportIssues::messages()` shown in the prompt and logged as a
    /// warning when the user exports anyway.
    pub messages: Vec<String>,
    /// The export scope snapshotted when the prompt was raised. "Export anyway"
    /// writes from THIS, never a fresh re-derivation — so the bytes on disk and
    /// the warning (drawn from `messages`, the same derivation) always
    /// describe the same project state, even if the document changes while the
    /// modal is up (#431 review).
    pub ctx: signex_output::ExportContext,
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
