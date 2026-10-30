//! Dialog + export + context-menu message enums.

use super::*;

/// Per-net colour override message family (ADR-0001 D3). Namespaced
/// under `Message::NetColor` and routed to
/// `dispatch_net_color_message`.
#[derive(Debug, Clone)]
pub enum NetColorMsg {
    /// Open the F5 Net Color palette (detached modal).
    Open,
    /// Close the Net Color palette.
    Close,
    /// Assign a color to a net label text, or clear the override.
    Set {
        net: String,
        color: Option<signex_types::theme::Color>,
    },
    /// Show / hide the custom net-color picker modal.
    CustomShow(bool),
    /// Live-update the draft colour as the user drags the picker.
    CustomDraft(iced::Color),
    /// Commit the draft colour and arm net-colour flood mode.
    CustomSubmit(iced::Color),
    /// Edit one R/G/B channel of the custom-picker draft via text
    /// input. Parsed as 0-255; invalid values ignored.
    CustomChannel(Channel, String),
}

/// Parameter Manager dialog message family (ADR-0001 D3). Namespaced
/// under `Message::ParameterManager` and routed to
/// `dispatch_parameter_manager_message`.
#[derive(Debug, Clone)]
pub enum ParameterManagerMsg {
    /// Open the Parameter Manager dialog (bulk parameter editor).
    Open,
    /// Close the dialog.
    Close,
    /// Edit a single parameter on a single symbol via the manager.
    Edit {
        symbol_uuid: uuid::Uuid,
        key: String,
        value: String,
    },
}

/// Annotate dialog message family (ADR-0001 D3). Namespaced under
/// `Message::Annotate` and routed to `dispatch_annotate_message`.
#[derive(Debug, Clone)]
pub enum AnnotateMsg {
    /// Auto-annotate every unannotated symbol (reference ends in `?`).
    /// Three modes: incremental, reset+renumber, reset-only.
    Run(signex_engine::AnnotateMode),
    /// Show the Annotate Schematics modal with preview of proposed changes.
    OpenDialog,
    /// Dismiss the Annotate dialog without applying.
    CloseDialog,
    /// Change the Annotate dialog's order-of-processing choice.
    OrderChanged(crate::app::state::AnnotateOrder),
    /// Show the Reset-Annotations confirm modal.
    OpenResetConfirm,
    /// Dismiss the Reset-Annotations confirm modal.
    CloseResetConfirm,
    /// Toggle the "locked against reannotation" flag on a symbol from
    /// inside the Annotate dialog. Locked symbols keep their current
    /// designator even under Reset & Renumber.
    ToggleLock(uuid::Uuid),
    /// Lock (`true`, "All Off") or unlock (`false`, "All On") every symbol
    /// the dialog lists.
    SetAllLocked(bool),
    /// The background load of the preview's sheets finished. `generation`
    /// is the load's token; stale results are dropped.
    PreviewLoaded {
        generation: u64,
        result: std::sync::Arc<Result<crate::app::handlers::erc::AnnotatePreviewLoad, String>>,
    },
    /// "Re-prefix to standard" was pressed (#1315): ask the mounted libraries
    /// for the class and prefix override of the project's placed parts.
    ReprefixPressed,
    /// The libraries answered the Re-prefix lookup. `generation` is the
    /// request's token; a result a newer request replaced is dropped.
    ReprefixRowsResolved {
        generation: u64,
        result: std::sync::Arc<Result<crate::library::reprefix_lookup::ReprefixLookup, String>>,
    },
    /// Confirm on the Re-prefix list: re-plan against the state of now and
    /// apply it as one undo step.
    ReprefixConfirmed,
    /// Cancel (or Esc) on the Re-prefix list: nothing changes.
    ReprefixCancelled,
    /// The worker read the project's sheets for Annotate or Reset Duplicate
    /// Designators (#859). `generation` names the run; a result a newer run
    /// replaced is dropped.
    PassSheetsRead {
        generation: u64,
        result: std::sync::Arc<Result<crate::app::handlers::erc::SheetsRead, String>>,
    },
    /// The worker wrote the unopened sheets the run renumbered. `hold` keeps
    /// quit waiting until the completion has registered what it owes.
    PassSheetsWritten {
        generation: u64,
        result: std::sync::Arc<Result<Vec<crate::app::handlers::erc::SheetWrite>, String>>,
        hold: crate::autosave::SaveHold,
    },
}

/// ERC dialog message family (ADR-0001 D3). Namespaced under
/// `Message::Erc` and routed to `dispatch_erc_message`.
#[derive(Debug, Clone)]
pub enum ErcMsg {
    /// Run the ERC engine against the active schematic snapshot and populate
    /// `ui_state.erc_violations`. Bound to F8.
    Run,
    /// Show the ERC modal (severity matrix + pin-compatibility matrix).
    OpenDialog,
    /// Dismiss the ERC dialog.
    CloseDialog,
    /// Override the severity for a single rule from within the ERC dialog.
    SeverityChanged(signex_erc::RuleKind, signex_erc::Severity),
    /// Click on a pin-connection matrix cell: cycle its severity
    /// Error → Warning → Info → Off → (back to baseline default).
    PinMatrixCellCycled { row: u8, col: u8 },
    /// Copy this machine's ERC preferences into the active project's
    /// `.snxprj` rule set (written on the next explicit save, #1085).
    StoreRulesInProject,
    /// "Save as profile": open the dialog that asks for a name and version
    /// (#1608). Writes nothing.
    ProfileSaveDialogOpened,
    /// The Save-as dialog's name field changed.
    ProfileSaveNameChanged(String),
    /// The Save-as dialog's version field changed.
    ProfileSaveVersionChanged(String),
    /// Save in the Save-as dialog: validate, then write unless the name
    /// exists (then ask first).
    ProfileSaveSubmitted,
    /// The user agreed to replace the existing profile of that name.
    ProfileSaveOverwriteConfirmed,
    /// Go back from the overwrite question, or close the Save-as dialog.
    ProfileSaveCancelled,
    /// The profile file was written, found taken, or failed. On success the
    /// project records the written profile as its provenance.
    ProfileSaved {
        project: crate::app::state::ProjectId,
        result: std::sync::Arc<Result<crate::rule_profiles::SaveOutcome, String>>,
    },
    /// The `User Default` profile (the machine's rules) was read at launch.
    /// `Ok(None)`: the machine has none yet.
    UserDefaultLoaded(std::sync::Arc<Result<Option<signex_types::project::RuleProfile>, String>>),
    /// A queued profile file write finished; the next one may start. Carries
    /// the profile the file now holds, for an edit of a library profile.
    ProfileWriteFinished(std::sync::Arc<Result<crate::app::rule_profile_ui::WriteOutcome, String>>),
    /// The quit path ran the writes still queued.
    ProfileWritesFlushed(std::sync::Arc<Result<(), String>>),
    /// "Update from profile": look up the profile the project was seeded
    /// from and show what applying it would change.
    ProfileUpdateRequested,
    /// The profile an update asked for was read. `Ok(None)`: no profile of
    /// that name on this machine.
    ProfileLoadedForUpdate {
        project: crate::app::state::ProjectId,
        result: std::sync::Arc<Result<Option<crate::rule_profiles::ResolvedProfile>, String>>,
    },
    /// The user confirmed the shown diff: apply it as one undo step.
    ProfileUpdateConfirmed,
    /// The user dismissed the shown diff; the project is unchanged.
    ProfileUpdateCancelled,
    /// The local profile a project was seeded from was read for the drift
    /// check. Same payload as `ProfileLoadedForUpdate`.
    ProfileDriftChecked {
        project: crate::app::state::ProjectId,
        result: std::sync::Arc<Result<Option<crate::rule_profiles::ResolvedProfile>, String>>,
    },
    /// The first-run move of the preferences' ERC rules into the user's
    /// default profile finished.
    DefaultProfileMigrated(std::sync::Arc<Result<crate::rule_profiles::ProfileMigration, String>>),
    /// Undo the last edit of the project's stored rule set.
    RulesUndone,
    /// Redo the last undone edit of the project's stored rule set.
    RulesRedone,
    /// A background ERC run finished. `generation` is the run's token: a
    /// result is applied only if no newer run has started since.
    RunFinished {
        generation: u64,
        result: std::sync::Arc<Result<crate::app::handlers::erc::ErcRunOutcome, String>>,
    },
}

/// Preferences modal message family (ADR-0001 D3). Namespaced under
/// `Message::Preferences` and routed to `dispatch_preferences_message`.
#[derive(Debug, Clone)]
pub enum PreferencesMsg {
    /// Open the Preferences modal.
    Open,
    /// Close the Preferences modal.
    Close,
    /// Navigate to a Preferences pane.
    Nav(crate::preferences::PrefNav),
    /// Forward an inner Preferences message to the pane handler.
    Inner(crate::preferences::PrefMsg),
}

/// Enable Version Control modal message family (ADR-0001 D3).
/// Namespaced under `Message::EnableVersionControl` and routed to
/// `dispatch_enable_version_control_message`.
#[derive(Debug, Clone)]
pub enum EnableVersionControlMsg {
    /// Toggle the LFS checkbox on the Enable Version Control modal.
    ToggleLfs,
    /// Toggle the per-item "Track" checkbox on the Enable Version
    /// Control modal. Index is into `EnableVersionControlState::items`.
    /// Untracked items are written into a generated `.gitignore` at
    /// confirm time so they sit outside the initial commit.
    ToggleItem(usize),
    /// Confirm — runs `git init` + initial commit at the project
    /// dir, refreshes the panel ctx so any in-tree dirty markers
    /// reflect the new state.
    Confirm,
    /// Dismiss the Enable Version Control modal without writing.
    Close,
    /// The worker that lists a library's top-level entries finished (#859).
    /// Applied only while the dialog still shows that directory.
    ItemsScanned {
        project_dir: std::path::PathBuf,
        result: Result<Vec<crate::app::TrackItem>, String>,
    },
    /// The off-thread run that `Confirm` started has finished (#1122).
    Finished {
        scope: crate::app::VersionControlScope,
        project_dir: std::path::PathBuf,
        outcome: EnableVersionControlOutcome,
    },
}

/// What an Enable Version Control run did on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnableVersionControlOutcome {
    /// `git init` / the initial commit was refused or failed; nothing is
    /// version-controlled. Carries the reason.
    InitFailed(String),
    /// The repository exists. `adapter_error` is why the project-scope
    /// `.gitattributes` sync failed (auto-commit then stays off);
    /// `commit_warning` is a failed commit of that sync, which is only
    /// logged.
    Created {
        adapter_error: Option<String>,
        commit_warning: Option<String>,
    },
}

/// Rename modal message family (ADR-0001 D3). Namespaced under
/// `Message::Rename` and routed to `dispatch_rename_message`.
#[derive(Debug, Clone)]
pub enum RenameMsg {
    /// Text input in the rename modal — updates the live buffer.
    BufferChanged(String),
    /// Commit the rename: fs::rename + update in-memory sheet / tab
    /// state. Errors surface in `RenameDialogState::error`.
    Submit,
    /// Dismiss the rename modal without applying.
    Close,
    /// A sheet, PCB, primitive or project file was moved on disk (or the
    /// move failed): re-key everything that knew the old path (#859).
    FileRenamed(Box<crate::app::path_ops::FileRenameDone>),
    /// A library was moved on disk and its saved project entry patched (or
    /// not): re-point the mount and open documents (#859).
    LibraryRenamed(Box<crate::app::path_ops::LibraryRenameDone>),
    /// The worker that undid a library rename the app could not follow
    /// finished (#859).
    LibraryRenameRolledBack(Box<crate::app::path_ops::LibraryRollbackDone>),
}

/// Remove-from-project modal message family (ADR-0001 D3). Namespaced
/// under `Message::Remove` and routed to `dispatch_remove_message`.
#[derive(Debug, Clone)]
pub enum RemoveMsg {
    /// User picked Delete / Exclude in the Remove modal.
    Confirm(RemoveChoice),
    /// User picked Save / Discard on the unsaved-edits stage (#738).
    ExcludeDirty(ExcludeDirtyChoice),
    /// Dismiss the Remove modal without applying.
    Close,
    /// The delete worker finished: forget the documents of what it removed
    /// (#859).
    Deleted(Box<crate::app::path_ops::DeleteDone>),
}

/// Context-menu subsystem message family (ADR-0001 D3). Namespaced
/// under `Message::ContextMenu` and routed to
/// `dispatch_context_menu_message`. Covers the canvas right-click
/// menu, the Projects-panel tree right-click menu, the document-tab
/// right-click menu, and the shared submenu hover/open state machine.
#[derive(Debug, Clone)]
pub enum ContextMenuMsg {
    Show(f32, f32),
    Close,
    Action(ContextAction),
    /// Right-click landed on a specific tree node — open the per-node
    /// context menu at `last_mouse_pos`. `key = None` → background menu.
    ShowProjectTree(Option<signex_widgets::tree_view::TreeKey>),
    /// Dismiss the Projects-panel tree context menu.
    CloseProjectTree,
    /// Menu item picked — route the action.
    ProjectTreeAction(ProjectTreeAction),
    /// Right-click on a document tab — open the per-tab context menu
    /// at `last_mouse_pos`. Carries the clicked tab's index.
    ShowTab(usize),
    /// Dismiss the document-tab right-click menu.
    CloseTab,
    /// Menu item picked — route the action.
    TabAction(TabContextAction),
    /// Expand a click-to-open submenu inside the right-click context
    /// menu (Place or Align). Toggles off when the same kind is fired
    /// twice, otherwise replaces the current submenu.
    SubmenuOpen(ContextSubmenu),
    /// Hover entered a submenu launcher row — start the 200 ms
    /// hover-open timer for that submenu (and cancel any pending
    /// close).
    SubmenuHover(ContextSubmenu),
    /// Hover left the submenu launcher row — cancels any pending
    /// open and starts the 150 ms close timer if a submenu is open.
    SubmenuLeave,
    /// Hover entered the open submenu panel — cancels the close timer
    /// so the panel stays visible while the cursor traverses it.
    SubmenuEnterPanel,
    /// Hover left the open submenu panel — starts the close timer.
    SubmenuLeavePanel,
    /// Tick (50 ms) fired by the subscription while the context menu is
    /// open; promotes a `pending_submenu` that has hovered long enough into
    /// an actual open, and closes a submenu once `submenu_unhovered_since`
    /// has aged.
    SubmenuTickHover,
}

/// Export subsystem message family (ADR-0001 D3). Namespaced under
/// `Message::Export` and routed to `dispatch_export_message`. Covers
/// the PDF / netlist / BOM export lifecycle plus the export-error
/// modal dismiss.
#[derive(Debug, Clone)]
pub enum ExportMsg {
    /// Open the unified PDF Export overlay (File → Export → PDF…). Now
    /// delegates to `handle_print_preview_requested`, which sets up
    /// `document_state.preview` with the rasterized pages plus every
    /// PDF setting in one modal.
    PdfOpenDialog,
    /// Completion of PDF export — carries the request (source document,
    /// options and sheet picks) captured at the Export click, plus the saved
    /// path or a cancel/error. The OS save dialog is non-modal and the preview
    /// can change before it returns, so nothing is read from shared state
    /// (#833, #852).
    PdfFinished {
        // Boxed: the options make this the largest `Message` variant.
        request: Box<crate::app::state::PdfExportRequest>,
        result: Result<std::path::PathBuf, String>,
    },
    /// Completion of netlist export — carries the document the export was
    /// started from (captured when the dialog opened, #852) plus either the
    /// saved path or error.
    NetlistFinished {
        source: std::path::PathBuf,
        result: Result<std::path::PathBuf, String>,
    },
    /// User invoked File → Export → Bill of Materials… — open the
    /// BOM preview modal instead of going straight to the file
    /// dialog. Mirrors Print Preview.
    BomRequested,
    /// Completion of BOM export — carries the document and options captured
    /// at the Export click (#852) plus either the saved path or a cancel/error.
    BomFinished {
        source: std::path::PathBuf,
        options: Box<signex_output::BomOptions>,
        result: Result<std::path::PathBuf, String>,
    },
    /// User picked a path for File → Export → ERC Report — format and write.
    ErcReportPathChosen(Result<std::path::PathBuf, String>),
    /// #431 — user clicked "Export anyway (incomplete)" on the
    /// netlist-incomplete prompt. Writes the partial `.net` (a clean file, no
    /// header) and logs the omitted pages as a warning, then clears the prompt.
    NetlistExportAnyway,
    /// #431 — user clicked "Cancel" on the netlist-incomplete prompt (or
    /// clicked outside it). Writes nothing and clears the prompt.
    NetlistCancelIncomplete,
    /// An export deliverable's off-thread write finished (#859).
    FileWritten(Box<ExportWriteOutcome>),
}

/// What an export's background file write produced, carried back to `update`
/// so the notice, advisory and log lines are applied on the UI thread.
#[derive(Debug, Clone)]
pub struct ExportWriteOutcome {
    /// The file written, released from the rename guard
    /// (`DocumentState::project_writes`) when the write lands.
    pub path: std::path::PathBuf,
    /// `Err` holds the complete user-facing failure text.
    pub result: Result<(), String>,
    /// Logged when the write succeeded.
    pub success_log: String,
    /// Shown after a successful write (BOM validation findings).
    pub advisory: Option<crate::app::state::ErrorNotice>,
    /// Reported to the Messages panel after a successful write.
    pub warnings: Vec<String>,
    /// The outcome is also shown in the Export PDF window's status line.
    pub report_in_preview: bool,
}

/// Custom Selection Filter modal message family (ADR-0001 D3).
/// Namespaced under `Message::SelectionFilter` and routed to
/// `dispatch_selection_filter_message`. Drives the footprint editor's
/// selection-filter customization modal.
#[derive(Debug, Clone)]
pub enum SelectionFilterMsg {
    /// v0.18.14.1 — Custom Selection Filter modal launcher. Opens
    /// the checkbox table of selection kinds over the active footprint editor.
    OpenCustom,
    /// v0.18.14.1 — Custom Selection Filter modal: Cancel / Esc.
    /// Discards the in-flight draft.
    CloseCustom,
    /// v0.18.14.1 — Custom Selection Filter modal: per-row checkbox
    /// toggle.
    ToggleCustomKind(crate::library::editor::footprint::state::SelectionFilterKind),
    /// v0.18.14.1 — Custom Selection Filter modal: Apply button.
    /// Writes the draft into the active footprint editor's
    /// `state.selection_filter` then closes.
    ApplyCustom,
}

/// File / save message family (ADR-0001 D3). Namespaced under
/// `Message::File` and routed to `dispatch_file_message`.
#[derive(Debug, Clone)]
pub enum FileMsg {
    Opened(Option<PathBuf>),
    /// File ▸ New Project — destination picked by the Save-As dialog.
    /// `None` when the user cancelled the picker; on `Some(path)` the
    /// handler creates the project folder `<parent>/<stem>/` and writes a
    /// fresh `<stem>.snxprj` (empty marker file — the parser is
    /// directory-driven) plus a blank `<stem>.snxsch` inside it, then loads
    /// the project + opens the schematic tab (#1205).
    NewProject(Option<PathBuf>),
    /// Completion of the off-thread New Project file writes (#859): the
    /// project is loaded and its schematic opened only after they succeed.
    NewProjectWritten {
        path: PathBuf,
        /// The `.snxprj` actually written (inside the project folder).
        result: Result<PathBuf, String>,
    },
    /// Completion of the off-thread sample-project unpack kicked off by
    /// Help > Open Sample Project. Carries the directory the sample
    /// lives in, or the stringified `anyhow` chain on failure.
    SampleProjectUnpacked(Result<PathBuf, String>),
    /// File ▸ Import ▸ Altium Project — the Altium file picked, or `None`
    /// when the picker was cancelled.
    AltiumImportPicked(Option<PathBuf>),
    /// Completion of the off-thread `signex-altium-import` run.
    AltiumImportFinished(Result<crate::altium_import::ImportOutcome, String>),
    /// Completion of the async schematic read+parse kicked off by
    /// `open_schematic_file` — the `fs::read_to_string` +
    /// `SnxSchematic::parse` run in `spawn_blocking` so `update()`
    /// never blocks on disk IO (mirrors the `HistoryLoaded` pattern).
    /// Carries the original path/title so the handler can open the
    /// tab exactly as the old synchronous path did; the error is the
    /// stringified `anyhow` context chain (`Message` is `Clone`,
    /// `anyhow::Error` is not).
    SchematicOpenFinished {
        path: PathBuf,
        title: String,
        result: Result<Box<SchematicSheet>, String>,
    },
    /// The per-placement-designator load note, computed off the UI thread
    /// after a schematic opened (`Ok(None)`: nothing to say; `Err`: the
    /// worker died, carrying the join error).
    InstanceNoteReady(Result<Option<String>, String>),
    /// Completion of the async PCB read+parse kicked off by
    /// `open_pcb_file`. Same shape as `SchematicOpenFinished`.
    PcbOpenFinished {
        path: PathBuf,
        title: String,
        result: Result<Box<signex_types::pcb::PcbBoard>, String>,
    },
    /// Completion of an off-thread schematic / PCB / primitive read started
    /// from the project tree or a primitive open (#859). Applied only while
    /// `request` is still the read tracked for its path; otherwise dropped.
    DocumentReadFinished {
        request: crate::app::document_read::ReadRequest,
        result: Result<
            Box<crate::app::document_read::LoadedDocument>,
            crate::app::document_read::OpenFailure,
        >,
    },
    /// Completion of an off-thread History restore (#859): the git restore
    /// and the re-read of the restored file. Applied only while `request` is
    /// still the restore tracked for its document.
    HistoryRestoreFinished {
        request: crate::app::document_read::ReadRequest,
        sha: String,
        edit_stamp: crate::app::restore_job::EditStamp,
        result: Result<
            Box<crate::app::restore_job::RestoredFile>,
            crate::app::restore_job::RestoreFailure,
        >,
    },
    Save,
    /// Completion of an off-thread document write (Save / Save As). The
    /// dirty marker clears only if the document has not changed since the
    /// snapshot; see `handlers/document_files/save_async.rs`.
    DocumentSaved(crate::persist::DocumentSaveDone),
    /// Completion of an off-thread symbol / footprint container write
    /// (Save / Save As). The draft stays dirty if it was edited meanwhile;
    /// see `app/dispatch/library/editor/primitive_save.rs`.
    PrimitiveSaved(Box<crate::persist::PrimitiveSaveDone>),
    /// The off-thread volume probes of a Save All finished; its writes
    /// start now (#1216).
    SaveAllProbed(Box<SaveAllPlan>),
    /// The off-thread identity probe of a loose document's directory finished
    /// (#1447): the case rule open and Ctrl+S use to tell whether two
    /// spellings are one file.
    DirCaseProbed {
        dir: PathBuf,
        answer: crate::app::state::project_case::CaseProbeOutcome,
    },
    /// Every write a Save All started has completed; carries what to do
    /// next (close the project, quit) and the failures already known.
    SaveAllSettled(Box<SaveAllSettle>),
    /// Quit could not confirm that in-flight document writes finished.
    DocumentSaveWaitFailed(String),
    /// User picked a destination from the schematic Save As dialog.
    /// `from_path` is the tab the dialog was opened for; the dialog is
    /// not modal, so the active tab may have changed by the time it
    /// returns and the handler refuses rather than write the wrong
    /// document to `to_path`.
    SaveAs {
        from_path: PathBuf,
        to_path: PathBuf,
    },
    /// User picked a destination from the Save-As dialog spawned the
    /// first time a freshly-minted `.snxsym` / `.snxfpt` editor tab is
    /// saved (the in-memory tab opened by `Add New ▸ Symbol` /
    /// `Add New ▸ Footprint`). Re-keys the editor + tab from the
    /// suggested path to the user's choice, then writes the file.
    /// `from_path` is the suggested in-memory path the editor is
    /// currently keyed under; `to_path` is the rfd result.
    SavePrimitiveAs {
        from_path: PathBuf,
        to_path: PathBuf,
    },
}

/// Project lifecycle message family (ADR-0001 D3). Namespaced under
/// `Message::Project` and routed to `dispatch_project_message`.
#[derive(Debug, Clone)]
pub enum ProjectMsg {
    /// User picked an option in the project-close confirmation modal
    /// (Save All / Discard All / Cancel) shown when closing a
    /// project that still has dirty documents.
    CloseConfirm(ProjectCloseChoice),
    /// The off-thread child-sheet resolution that Close Project starts
    /// finished (#741). Carries the project by id, not index, because the
    /// project list can shift while the worker reads; `Err` is a worker that
    /// died, which refuses the close rather than guessing ownership.
    CloseRefsResolved {
        project_id: crate::app::ProjectId,
        result: Result<Box<crate::app::state::sheet_refs::SheetRefs>, String>,
    },
    /// The off-thread probe of a freshly loaded project's volume finished
    /// (#694): the volume ignores case, is case-sensitive, could not be
    /// decided, or the worker failed.
    CaseProbed {
        project_id: crate::app::ProjectId,
        outcome: crate::app::state::project_case::CaseProbeOutcome,
    },
    /// The off-thread filesystem probe behind the project tree's
    /// `(missing)` marks and library listings finished (#111); `Err` is a
    /// worker that died, with the reason (#1091).
    FsFactsProbed(
        u64,
        Result<Box<crate::app::state::project_fs::ProjectFsFacts>, String>,
    ),
    /// The off-thread Navigator hierarchy assembly finished (#1470); `Err`
    /// is a worker that died. Applied only while its generation is the latest.
    NavigatorAssembled(
        u64,
        Result<Box<crate::app::state::navigator_job::NavigatorAssembly>, String>,
    ),
    /// User choice (Save All / Discard All / Cancel) on the app-quit
    /// confirmation modal, shown when the user tries to exit Signex
    /// while any document is dirty. Reuses `ProjectCloseChoice`.
    AppQuitConfirm(ProjectCloseChoice),
    /// A Project tab message (#1202), routed to the `project_tab` slice.
    Tab(crate::app::ProjectTabMsg),
    /// The off-thread `.snxprj` write a Project tab Ctrl+S started finished.
    FileSaved(Box<crate::persist::ProjectSaveDone>),
    /// Result of the `Add Existing to Project…` file picker. Carries
    /// the owning project's id plus the user's picks (`None` on
    /// cancel, otherwise one or more paths from `pick_files`) so the
    /// handler can copy each into the project directory in turn.
    AddExistingFilePicked {
        project_id: crate::app::state::ProjectId,
        paths: Option<Vec<std::path::PathBuf>>,
    },
    /// Result of the `Add New ▸ Schematic` Save-As dialog. `None`
    /// when the user cancelled; on `Some(path)` the handler writes
    /// a blank `.snxsch`, registers it on the project, and marks
    /// the .snxprj dirty.
    AddNewSchematicPicked {
        project_id: crate::app::state::ProjectId,
        path: Option<std::path::PathBuf>,
    },
    /// The off-thread copies behind `AddExistingFilePicked` finished: one
    /// entry per pick, `Ok` the path to register, `Err` a message for the
    /// Messages panel. The outer `Err` is a worker that died.
    AddExistingFilesCopied {
        project_id: crate::app::state::ProjectId,
        /// The folder the copies went into, released from the rename
        /// guard (`DocumentState::project_writes`).
        project_dir: std::path::PathBuf,
        results: Result<Vec<Result<std::path::PathBuf, String>>, String>,
    },
    /// The off-thread blank-schematic write behind `AddNewSchematicPicked`
    /// finished.
    AddNewSchematicWritten {
        project_id: crate::app::state::ProjectId,
        path: std::path::PathBuf,
        result: Result<(), String>,
    },
    /// The Create Sheet dialog's path field was edited.
    CreateChildSheetPathChanged(String),
    /// "Browse..." in the Create Sheet dialog.
    CreateChildSheetBrowse,
    /// The native save dialog of "Browse..." answered (`None`: cancelled).
    CreateChildSheetBrowsed(Option<std::path::PathBuf>),
    /// The Create Sheet dialog was answered: Create, or Later (`create:
    /// false`, which also closes the window).
    CreateChildSheetAnswered { create: bool },
    /// "Open it" in the Create Sheet dialog, for a path that already exists.
    CreateChildSheetOpenExisting,
    /// The off-thread write of a child sheet created from a symbol finished.
    /// `project_id` is the project owning the parent sheet, if any.
    ChildSheetCreated {
        project_id: Option<crate::app::state::ProjectId>,
        parent: std::path::PathBuf,
        symbol: uuid::Uuid,
        path: std::path::PathBuf,
        result: Result<(), crate::app::contracts::ChildSheetWriteError>,
    },
    /// v0.23 — Async project-git commit completed. The dispatcher
    /// removes the `(project_root, rel_path)` entry from
    /// `inflight_git_commits` and logs success/failure. `result.Ok`
    /// carries the formatted commit OID, or `None` when the staged tree
    /// matched HEAD's and no commit was made (#149); `result.Err` carries the
    /// error string. Best-effort — a failure here doesn't roll back
    /// the on-disk save (data is already on disk; this just means git
    /// didn't capture it).
    GitCommitDone {
        project_root: std::path::PathBuf,
        rel_path: std::path::PathBuf,
        result: Result<Option<String>, GitCommitFailure>,
    },
    /// The off-thread project move behind a Rename Project that also
    /// renames the project folder (#1221) finished. The in-memory
    /// project, its open documents, the recent-projects list and the
    /// version-control root follow only when `result` is `Ok`; on `Err`
    /// the disk was restored and the dialog shows the reason.
    ProjectFolderRenamed {
        job: ProjectRenameJob,
        result: Result<(), String>,
    },
    /// The off-thread move of the `.snxprj` alone (the folder keeps its name)
    /// finished (#859). The in-memory project follows only when `result` is
    /// `Ok`; on `Err` the file was not moved and the dialog says why.
    ProjectFileRenamed {
        job: ProjectFileRenameJob,
        result: Result<(), String>,
    },
    /// The worker that lists the `.snxsch` files beside the active sheet
    /// finished (#859): the Place Sheet tool's second click picks its default
    /// name from them. `ticket` names the click; a result for any other
    /// ticket is dropped.
    SheetFilesListed {
        ticket: u64,
        result: Result<Vec<String>, String>,
    },
}

/// One Rename Project that moves only the `.snxprj`: `from` becomes `to`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectFileRenameJob {
    /// The registration in `pending_path_ops` that refuses saves, opens and
    /// other moves of the two paths until the move is reported.
    pub guard: crate::app::path_ops::PathOpId,
    pub project_id: crate::app::state::ProjectId,
    pub from: std::path::PathBuf,
    pub to: std::path::PathBuf,
    pub new_stem: String,
    /// The recent-projects spelling of `from`, read before the move.
    pub recent_key: std::path::PathBuf,
}

/// One Rename Project that moves the `.snxprj` and its folder together
/// (#1221): `<old_dir>/<old_stem>.snxprj` becomes
/// `<new_dir>/<new_stem>.snxprj`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRenameJob {
    pub project_id: crate::app::state::ProjectId,
    pub old_dir: std::path::PathBuf,
    pub new_dir: std::path::PathBuf,
    pub old_stem: String,
    pub new_stem: String,
    /// The recent-projects spelling of the old `.snxprj`, read before the
    /// move (the file no longer exists afterwards to canonicalise).
    pub recent_key: std::path::PathBuf,
}

impl ProjectRenameJob {
    /// The `.snxprj` before the move.
    pub fn old_project_file(&self) -> std::path::PathBuf {
        self.old_dir.join(format!("{}.snxprj", self.old_stem))
    }

    /// The `.snxprj` after the move.
    pub fn new_project_file(&self) -> std::path::PathBuf {
        self.new_dir.join(format!("{}.snxprj", self.new_stem))
    }
}

/// Why a project-git commit did not happen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitCommitFailure {
    /// The version-control guard refused to start a repository in the
    /// project's folder (#1205); auto-commit is paused for it.
    Refused(String),
    /// git itself failed.
    Failed(String),
}

/// What a paste request does once the system clipboard has answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteIntent {
    /// Attach the copy to the cursor.
    Paste,
    /// Place the copy beside the existing content.
    SmartPaste,
    /// Open the Paste Array dialog.
    Array,
    /// Place the copy just made, beside the original.
    Duplicate,
    /// Arm the copy just made as a rubber stamp.
    Stamp,
    /// Read the project's numbers for an open Paste Array dialog whose
    /// designator choice became Next free; nothing follows the read.
    ArraySeed,
}

/// One paste-family request in flight: what it is for, and the tool state it
/// was made in. An answer that arrives after the placement epoch or the
/// active sheet moved on is dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PasteRequest {
    pub intent: PasteIntent,
    pub epoch: u64,
    pub path: Option<PathBuf>,
    /// The Paste Array dialog an `ArraySeed` read is for; 0 otherwise.
    pub dialog: u64,
}

/// Edit-command message family (ADR-0001 D3). Namespaced under
/// `Message::Edit` and routed to `dispatch_edit_message`.
#[derive(Debug, Clone)]
pub enum EditMsg {
    /// Apply an edit to a placed SchDrawing, dispatched from the
    /// post-placement Properties panel. The engine replaces the stored
    /// drawing by uuid with full undo.
    UpdateDrawingField(uuid::Uuid, DrawingFieldEdit),
    /// Delete the current selection. In a footprint editor this routes
    /// to the footprint dispatcher's `DeleteSelected`; otherwise the
    /// schematic engine removes the selected elements.
    DeleteSelected,
    /// Undo the most recent edit.
    Undo,
    /// Redo the most recently undone edit.
    Redo,
    /// Rotate the current selection 90 degrees clockwise on screen (mirrored symbols included).
    RotateSelectedClockwise,
    /// Rotate the current selection 90 degrees counter-clockwise on screen.
    RotateSelectedCounterclockwise,
    /// Mirror the current selection about the X axis.
    MirrorSelectedX,
    /// Mirror the current selection about the Y axis.
    MirrorSelectedY,
    /// Copy the current selection to the clipboard.
    Copy,
    /// Cut the current selection to the clipboard.
    Cut,
    /// Paste the clipboard contents.
    Paste,
    /// Smart-paste the clipboard contents.
    SmartPaste,
    /// Duplicate the current selection in place.
    Duplicate,
    /// Arm the last label or text note again, with its text stepped (#1253).
    RepeatLastItem,
    /// Rubber-stamp copy: copy the selection and keep pasting it on every
    /// click until Esc or a right-click (#1264).
    RubberStampCopy,
    /// Open the Paste Array dialog for the clipboard (#1264).
    PasteArrayRequested,
    /// A Paste Array dialog event (#1264).
    PasteArray(crate::paste_array::PasteArrayMsg),
    /// The system clipboard answered a paste request (#1263): its text if it
    /// holds any. A Signex fragment in it replaces the in-app copy.
    PasteTextRead {
        request: PasteRequest,
        text: Option<String>,
    },
    /// The project's designator numbers were read off the UI thread (#1263);
    /// the paste carries on.
    PasteCountersSeeded {
        request: PasteRequest,
        seed: std::sync::Arc<crate::app::handlers::paste::PasteSeed>,
    },
    /// Reorder the selected drawings within the sheet's drawing list; the
    /// selection's circuit elements are ignored (#1583).
    ArrangeDrawings(signex_engine::DrawingArrange),
}

/// Per-shape edit descriptor. The Properties panel dispatches one of
/// these per numeric text-input edit; the handler looks up the stored
/// drawing, applies the field change, and emits
/// `Command::UpdateSchDrawing` with the patched variant.
#[derive(Debug, Clone, Copy)]
pub enum DrawingFieldEdit {
    Width(f64),
    Fill(signex_types::schematic::FillType),
    LineStartX(f64),
    LineStartY(f64),
    LineEndX(f64),
    LineEndY(f64),
    RectStartX(f64),
    RectStartY(f64),
    RectWidthMm(f64),
    RectHeightMm(f64),
    CircleCenterX(f64),
    CircleCenterY(f64),
    CircleRadius(f64),
    ArcCenterX(f64),
    ArcCenterY(f64),
    ArcRadius(f64),
    ArcStartAngle(f64),
    ArcEndAngle(f64),
    /// Override the stroke colour; `None` restores the theme default.
    StrokeColor(Option<signex_types::schematic::StrokeColor>),
    /// Put the drawing behind or above the circuit (#1583). Not a field of
    /// the drawing itself: it is stored per sheet.
    Placement(signex_types::schematic::DrawingPlacement),
}
