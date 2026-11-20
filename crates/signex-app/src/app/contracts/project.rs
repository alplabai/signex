//! File and project message enums, and the jobs they carry.

use super::*;

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
        /// What the read learned about the file (#1826, #1827).
        disk: Option<crate::persist::DiskRead>,
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
        /// What the read learned about the file (#1863).
        disk: Option<crate::persist::DiskRead>,
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
    /// The read behind "Reload from disk" finished (#1826): the sheet read
    /// back from the file that changed under an open document.
    DiskReloadFinished {
        path: PathBuf,
        /// The document's edit stamp when Reload was chosen; a later one means
        /// the user edited during the read.
        edit_stamp: crate::app::restore_job::EditStamp,
        result: Result<Box<crate::app::handlers::document_files::ReloadedFile>, String>,
    },
    /// An idle poll re-read the files of open documents (#1877).
    DocumentDiskPolled(Vec<crate::app::handlers::document_files::DiskRead>),
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
    /// The Save As dialog opened from the File Changed on Disk card returned
    /// `picked` (a `SaveAs` or `SavePrimitiveAs`); a pick onto `conflicted`
    /// itself is refused (GG-08).
    DiskConflictSaveAsPicked {
        conflicted: PathBuf,
        picked: Box<FileMsg>,
    },
    /// The off-thread patch of the saved `.snxprj` after a sheet Save As
    /// finished (#1824).
    SaveAsProjectPatched(Box<SaveAsProjectPatched>),
}

/// What a sheet Save As did to the saved project file.
#[derive(Debug, Clone)]
pub struct SaveAsProjectPatched {
    pub project_id: crate::app::state::ProjectId,
    pub from: PathBuf,
    pub to: PathBuf,
    pub result: Result<crate::app::project_entry_disk::EntryPatch, String>,
    /// The parent references planned for the new file, and what the worker
    /// did to the saved parents (#1849).
    pub plan: crate::app::path_ops::ReferencePlan,
    pub parent_disk: crate::app::parent_reference_disk::ParentDiskOutcomes,
}

/// Project lifecycle message family (ADR-0001 D3). Namespaced under
/// `Message::Project` and routed to `dispatch_project_message`.
#[derive(Debug, Clone)]
pub enum ProjectMsg {
    /// The New Project setup dialog (#2232); routed to
    /// `dispatch_new_project_message`.
    Setup(crate::app::NewProjectMsg),
    /// The New PCB or New Schematic setup dialog (#2232); routed to
    /// `dispatch_document_setup_message`.
    DocumentSetup(crate::app::DocumentSetupMsg),
    /// A Save as profile or Remove was merged into `prefs.json` (or not):
    /// `note` is said in Messages on success (#2232).
    SetupProfilesWritten {
        note: String,
        /// How to take the in-memory change back if the write failed.
        undo: Box<crate::app::ProfileUndo>,
        /// The id the profile is stored under (`None` for a removal).
        result: Result<Option<String>, String>,
    },
    /// Start screen project rows: pin, unpin, open a pinned project, the
    /// row menu (#2305); routed to `dispatch_start_screen_message`.
    StartScreen(crate::start_screen::StartScreenMsg),
    /// The Number Schematic Sheets dialog (#2247); routed to
    /// `dispatch_sheet_numbering_message`.
    SheetNumbering(crate::app::SheetNumberingMsg),
    /// User picked an option in the project-close confirmation modal
    /// (Save All / Discard All / Cancel) shown when closing a
    /// project that still has dirty documents.
    CloseConfirm(ProjectCloseChoice),
    /// The Tools ▸ Assign Footprints dialog (#2248); routed to
    /// `dispatch_assign_footprints_message`.
    AssignFootprints(crate::app::AssignFootprintsMsg),
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
    /// The off-thread read of the ids at the head of the files a project
    /// references finished (#2172). Dropped when the project closed meanwhile.
    DocumentHeadersRead {
        project_id: crate::app::ProjectId,
        reads: Vec<crate::app::document_ids::HeaderRead>,
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
    /// A descent through a sheet symbol that an undocked window started: the
    /// window cannot switch sheets inside its swap, so the open runs here, on
    /// the main window, for the child the window's sheet references (#1966).
    /// It names the window and the sheet the descent started on; it is
    /// refused when it lands if either no longer holds (#2037).
    DescendRequested {
        window: iced::window::Id,
        from: std::path::PathBuf,
        symbol: uuid::Uuid,
        path: std::path::PathBuf,
        placement: Option<signex_types::schematic::InstancePath>,
    },
    /// Edit Symbol from an undocked window: the window cannot open or switch
    /// tabs inside its swap, so the symbol file it resolved is opened here, on
    /// the main window (#1966).
    PrimitiveOpenRequested(std::path::PathBuf),
    /// A Find hit on another sheet, reached from an undocked window: the
    /// window cannot switch sheets inside its swap, so the sheet opens here,
    /// on the main window, and the jump lands there (#2037).
    SheetOpenRequested {
        path: std::path::PathBuf,
        open: Box<crate::app::handlers::canvas::SheetOpen>,
    },
    /// The off-thread assembly of the hierarchy of the project (by root sheet)
    /// an undocked window shows finished (#2021).
    WindowHierarchyAssembled(
        std::path::PathBuf,
        u64,
        Result<Box<crate::app::state::navigator_job::NavigatorAssembly>, String>,
    ),
    /// User choice (Save All / Discard All / Cancel) on the app-quit
    /// confirmation modal, shown when the user tries to exit Signex
    /// while any document is dirty. Reuses `ProjectCloseChoice`.
    AppQuitConfirm(ProjectCloseChoice),
    /// A Project tab message (#1202), routed to the `project_tab` slice.
    Tab(crate::app::ProjectTabMsg),
    /// A Rules tab message (#2398), routed to the `rules_tab` slice.
    Rules(crate::app::RulesTabMsg),
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
        /// The New Schematic setup dialog's values (#2232); `None`: the
        /// project's sheet defaults.
        choice: Option<crate::app::SheetChoice>,
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
        result: Result<crate::app::project_entry_disk::EntryPatch, String>,
        /// The setup dialog's designator standard, applied only when the
        /// write succeeded (#2232).
        designators: Option<signex_types::designator::DesignatorStandard>,
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
