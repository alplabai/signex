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
    /// Commit the draft colour: colour the picker's net when it was opened
    /// from a palette row, else arm net-colour flood mode.
    CustomSubmit(iced::Color),
    /// Edit one R/G/B channel of the custom-picker draft via text
    /// input. Parsed as 0-255; invalid values ignored.
    CustomChannel(Channel, String),
    /// The palette's filter text changed.
    FilterChanged(String),
    /// A palette row was clicked: its chooser opens, or closes when it was
    /// the open one.
    RowOpened(String),
    /// A press landed outside the open chooser, or on its own row: it closes.
    ChooserDismissed,
    /// A press landed in `window`: the chooser closes unless that is the
    /// palette's own window, whose presses the card sorts out itself.
    PressedInWindow(iced::window::Id),
    /// The chooser's Custom… was pressed: the custom picker opens for that
    /// net.
    CustomOpenedFor(String),
    /// The custom picker's Hex field changed; applied when it parses.
    CustomHexChanged(String),
    /// The palette footer's "Clear all colours" was pressed: the active
    /// sheet's net colours go in one undo step.
    ClearedAll,
    /// The Clear toast's Undo was pressed: the clear is undone while it is
    /// still the sheet's newest edit.
    ToastUndone,
    /// The Clear toast made at this engine revision timed out.
    ToastExpired(u64),
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
    /// A cell's text changed: write it to `sheet` now. Keystrokes in one
    /// cell share one undo step; an empty value keeps the parameter and
    /// empties it.
    Edit {
        sheet: std::path::PathBuf,
        symbol_uuid: uuid::Uuid,
        key: String,
        value: String,
    },
    /// Enter in a cell: the next edit of that cell is a new undo step.
    CellSubmitted,
    /// Delete the parameter `key` from every listed symbol, as one undo
    /// step across the listed sheets.
    ParameterRemoved { key: String },
    /// The "new parameter" name box changed.
    NewKeyChanged(String),
    /// Add the typed parameter as a column on every listed symbol, empty,
    /// as one undo step across the listed sheets.
    ColumnAdded,
    /// The header's Active sheet / All sheets switch was set.
    ScopePicked(ParameterScope),
    /// The row filter changed.
    FilterChanged(String),
    /// Clear filter on the no-match card: every row shows again.
    FilterCleared,
    /// Revert changes: every cell edited since the dialog opened goes back
    /// to its value from before, as one undo step.
    SessionReverted,
    /// The pointer entered (`is_hovered`) or left the header of parameter
    /// column `key`. A leave names its column, so it never clears the
    /// hover of the header just entered.
    ColumnHeaderHovered { key: String, is_hovered: bool },
    /// The cell `key` of `symbol_uuid` on `sheet` took (`is_focused`) or
    /// lost the focus. A loss names its cell, so it never clears the cell
    /// just focused.
    CellFocused {
        sheet: std::path::PathBuf,
        symbol_uuid: uuid::Uuid,
        key: String,
        is_focused: bool,
    },
    /// One pane of the table scrolled to `x`, `y`; the others follow, so the
    /// header row and the Reference column stay in view.
    TableScrolled {
        pane: ParameterTablePane,
        x: f32,
        y: f32,
    },
    /// The table's CSV export and import (#1295), routed to
    /// `app::parameter_io`.
    Io(crate::app::parameter_io::ParameterIoMsg),
    /// The closed project sheets All sheets lists were read from disk, under
    /// read `generation`; `unread` names each sheet that could not be read,
    /// with why.
    ClosedSheetsRead {
        generation: u64,
        sheets: crate::app::state::ClosedSheetCopies,
        unread: Vec<(std::path::PathBuf, String)>,
    },
    /// A cell of a closed sheet's row was pressed: open that sheet, so its
    /// cells can be edited.
    ClosedSheetPressed { sheet: std::path::PathBuf },
}

/// Which sheets the Parameter Manager lists (Caner 2026-11-25).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ParameterScope {
    /// The active sheet only.
    #[default]
    ActiveSheet,
    /// Every sheet of the active sheet's project; the closed ones read-only.
    AllSheets,
}

/// The scrolling panes of the Parameter Manager table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterTablePane {
    /// The header row right of Reference: scrolls sideways with the body.
    Header,
    /// The Reference column: scrolls up and down with the body.
    References,
    /// The parameter cells: scroll both ways.
    Body,
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
    /// The worker read the project's sheets for the Re-prefix plan (#1756).
    /// `generation` is the request's token; a result a newer request replaced
    /// is dropped.
    ReprefixSheetsRead {
        generation: u64,
        result: std::sync::Arc<crate::app::handlers::erc::ReprefixSheets>,
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
    /// The change list's Changes / All switch was picked (#75).
    ListViewPicked(crate::app::handlers::erc::AnnotateListView),
    /// The change list's filter text was edited (#75).
    ListFilterEdited(String),
    /// A sheet row's tick was clicked: the sheet file (every placement row
    /// of it) joins or leaves the run (#75).
    SheetTickToggled(std::path::PathBuf),
    /// The sheets table's header tick was clicked: tick (`true`) or untick
    /// every sheet.
    AllSheetsTicked(bool),
    /// The "Fixed options" group was opened or closed.
    FixedOptionsToggled,
    /// Reset All (`ResetOnly`) or Reset & Renumber (`ResetAndRenumber`) was
    /// pressed in the dialog footer: open the confirm for that reset (#76).
    ResetPressed(signex_engine::AnnotateMode),
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
    /// The rule editor's "Changed only" chip was switched; carries the new
    /// state.
    RulesChangedOnlyToggled(bool),
    /// The rule editor's "Filter rules" field changed.
    RuleFilterChanged(String),
    /// Copy this machine's ERC preferences into the active project's
    /// `.snxprj` rule set (written on the next explicit save, #1085).
    StoreRulesInProject,
    /// Repair Pin Junctions: a dot on every pin end of the active sheet that
    /// lies inside a wire with none, as one undo step (2026-11-18).
    PinJunctionRepairRequested,
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
    /// The footer's "Profile" menu button was pressed: the menu opened, or
    /// closed when it was open.
    ProfileMenuToggled,
    /// A row of the "Profile" menu was chosen. The menu closes and the row's
    /// action runs as its own message ([`ErcProfileAction::message`]).
    ProfileMenuRowChosen(ErcProfileAction),
}

/// The rows of the ERC dialog's "Profile" menu (#2413). Each runs the
/// message the dialog's footer button of the same name sent before.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErcProfileAction {
    /// "Store ERC rules in this project".
    StoreInProject,
    /// "Save as profile".
    SaveAsProfile,
    /// "Update from profile".
    UpdateFromProfile,
}

impl ErcProfileAction {
    /// The message the row runs.
    pub fn message(self) -> ErcMsg {
        match self {
            Self::StoreInProject => ErcMsg::StoreRulesInProject,
            Self::SaveAsProfile => ErcMsg::ProfileSaveDialogOpened,
            Self::UpdateFromProfile => ErcMsg::ProfileUpdateRequested,
        }
    }

    /// The row's label.
    pub fn label(self) -> signex_i18n::MessageId {
        use signex_i18n::ids;
        match self {
            Self::StoreInProject => ids::DIALOG_ERC_STORE_RULES,
            Self::SaveAsProfile => ids::DIALOG_ERC_SAVE_AS_PROFILE,
            Self::UpdateFromProfile => ids::DIALOG_ERC_UPDATE_FROM_PROFILE,
        }
    }
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
    /// "Select all" was clicked: every row is ticked.
    AllTicked,
    /// The first commit message field was edited.
    CommitMessageChanged(String),
    /// The "Commit every save" switch was flipped.
    CommitEverySaveToggled,
    /// The worker asking git for a commit author answered (mockup row 26):
    /// `has_identity` is false when git resolves none. Applied only while
    /// the dialog still shows `project_dir`.
    AuthorProbed {
        project_dir: std::path::PathBuf,
        has_identity: bool,
    },
    /// The author name field (shown only when git has none) was edited.
    AuthorNameChanged(String),
    /// The author email field was edited.
    AuthorEmailChanged(String),
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
    /// A card of the Remove modal was clicked: it is selected, and nothing
    /// is removed until the footer action sends `Confirm` (#2262).
    ChoicePicked(RemoveChoice),
}

/// Context-menu subsystem message family (ADR-0001 D3). Namespaced
/// under `Message::ContextMenu` and routed to
/// `dispatch_context_menu_message`. Covers the canvas right-click
/// menu, the Projects-panel tree right-click menu, the document-tab
/// right-click menu, and the shared submenu hover/open state machine.
#[derive(Debug, Clone)]
pub enum ContextMenuMsg {
    /// `message` came from a menu painted in the undocked window `window`:
    /// it acts on the sheet that window shows, not on the main window's
    /// (#1910).
    ScopedToWindow(iced::window::Id, Box<ContextMenuMsg>),
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
    /// A BOM's off-thread write finished; `source` is the document its dialog
    /// was opened for, which closes when the file was written.
    BomWritten {
        source: std::path::PathBuf,
        outcome: Box<ExportWriteOutcome>,
    },
    /// Export Design Notebook PDF (mockup row 202), routed to
    /// `dispatch_notebook_pdf_message`.
    NotebookPdf(crate::app::NotebookPdfMsg),
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
    /// The document a PDF export was made from. Set, the outcome is also
    /// shown in the status line of the Export PDF window open for it.
    pub preview_source: Option<std::path::PathBuf>,
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
    /// All (`true`) or None (`false`) was pressed: every shown kind follows.
    CustomKindsAllSet(bool),
    /// The Preset name field was edited.
    CustomPresetNameEdited(String),
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
    /// An agent asked (`Origin::Agent` when the request was made): Paste
    /// Special pastes at once rather than opening a dialog no one answers.
    pub from_agent: bool,
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
    /// Flip the selected footprints to the other side of the board (#2240).
    FlipSelection,
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
    /// A placement extra on a sheet (#1667).
    Extras(super::PlacementExtrasMsg),
    /// Bottom-up hierarchy (#1276): a sheet symbol from a sheet, and the
    /// sheet entry sync with its preview.
    SheetSync(crate::app::SheetSyncMsg),
    /// A power rail rename's sheet reads finished (mockup row 45).
    RailRename(crate::app::RailRenameMsg),
    /// Schematic tables (#1300): contents sheet, CSV, cell edits.
    Tables(crate::app::TablesMsg),
    /// Named unions (mockup row 158): the canvas menu rows, the keymap and the Create
    /// Union popover.
    Unions(crate::app::unions::UnionsMsg),
    /// A Smart Paste dialog event (mockup row 155).
    SmartPasteDialog(crate::smart_paste::SmartPasteMsg),
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

impl DrawingFieldEdit {
    /// Whether the value can be stored (#2767): a size or radius is above
    /// zero, a stroke is zero or more, and every number is finite.
    pub fn is_valid(self) -> bool {
        match self {
            Self::Width(v) => v.is_finite() && v >= 0.0,
            Self::RectWidthMm(v)
            | Self::RectHeightMm(v)
            | Self::CircleRadius(v)
            | Self::ArcRadius(v) => v.is_finite() && v > 0.0,
            Self::LineStartX(v)
            | Self::LineStartY(v)
            | Self::LineEndX(v)
            | Self::LineEndY(v)
            | Self::RectStartX(v)
            | Self::RectStartY(v)
            | Self::CircleCenterX(v)
            | Self::CircleCenterY(v)
            | Self::ArcCenterX(v)
            | Self::ArcCenterY(v)
            | Self::ArcStartAngle(v)
            | Self::ArcEndAngle(v) => v.is_finite(),
            Self::Fill(_) | Self::StrokeColor(_) | Self::Placement(_) => true,
        }
    }
}
