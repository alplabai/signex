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
}

/// Remove-from-project modal message family (ADR-0001 D3). Namespaced
/// under `Message::Remove` and routed to `dispatch_remove_message`.
#[derive(Debug, Clone)]
pub enum RemoveMsg {
    /// User picked Delete / Exclude in the Remove modal.
    Confirm(RemoveChoice),
    /// Dismiss the Remove modal without applying.
    Close,
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
    /// context menu at `last_mouse_pos`. `path = None` → background menu.
    ShowProjectTree(Option<Vec<usize>>),
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
    /// 50 ms tick fired by the subscription while the context menu is
    /// open; promotes a mature `pending_submenu` into an actual open
    /// and a mature `pending_submenu_close` into an actual close.
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
    /// Completion of PDF export — carries either the saved path or error.
    PdfFinished(Result<std::path::PathBuf, String>),
    /// Completion of netlist export — carries either the saved path or error.
    NetlistFinished(Result<std::path::PathBuf, String>),
    /// User invoked File → Export → Bill of Materials… — open the
    /// BOM preview modal instead of going straight to the file
    /// dialog. Mirrors Print Preview.
    BomRequested,
    /// Completion of BOM export — carries either the saved path or error.
    BomFinished(Result<std::path::PathBuf, String>),
    /// User clicked the OK button on the export-error modal.
    DismissError,
    /// #431 — user clicked "Export anyway (incomplete)" on the
    /// netlist-incomplete prompt. Writes the partial `.net` with an INCOMPLETE
    /// header comment listing the omitted pages, then clears the prompt.
    NetlistExportAnyway,
    /// #431 — user clicked "Cancel" on the netlist-incomplete prompt (or
    /// clicked outside it). Writes nothing and clears the prompt.
    NetlistCancelIncomplete,
}

/// Custom Selection Filter modal message family (ADR-0001 D3).
/// Namespaced under `Message::SelectionFilter` and routed to
/// `dispatch_selection_filter_message`. Drives the footprint editor's
/// selection-filter customization modal.
#[derive(Debug, Clone)]
pub enum SelectionFilterMsg {
    /// v0.18.14.1 — Custom Selection Filter modal launcher. Opens
    /// the 8-row checkbox table over the active footprint editor.
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
    /// handler writes a fresh `<stem>.snxprj` (empty marker file — the
    /// parser is directory-driven) plus a blank `<stem>.snxsch` next
    /// to it, then loads the project + opens the schematic tab.
    NewProject(Option<PathBuf>),
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
    /// Completion of the async PCB read+parse kicked off by
    /// `open_pcb_file`. Same shape as `SchematicOpenFinished`.
    PcbOpenFinished {
        path: PathBuf,
        title: String,
        result: Result<Box<signex_types::pcb::PcbBoard>, String>,
    },
    Save,
    SaveAs(PathBuf),
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
    /// project that still has entries in `dirty_paths`.
    CloseConfirm(ProjectCloseChoice),
    /// User choice (Save All / Discard All / Cancel) on the app-quit
    /// confirmation modal, shown when the user tries to exit Signex
    /// while `dirty_paths` is non-empty. Reuses `ProjectCloseChoice`.
    AppQuitConfirm(ProjectCloseChoice),
    /// Dismiss the Project Options metadata modal.
    CloseOptions,
    /// Result of the `Add Existing to Project…` file picker. Carries
    /// the owning project's index plus the user's picks (`None` on
    /// cancel, otherwise one or more paths from `pick_files`) so the
    /// handler can copy each into the project directory in turn.
    AddExistingFilePicked {
        project_idx: usize,
        paths: Option<Vec<std::path::PathBuf>>,
    },
    /// Result of the `Add New ▸ Schematic` Save-As dialog. `None`
    /// when the user cancelled; on `Some(path)` the handler writes
    /// a blank `.snxsch`, registers it on the project, and marks
    /// the .snxprj dirty.
    AddNewSchematicPicked {
        project_idx: usize,
        path: Option<std::path::PathBuf>,
    },
    /// v0.23 — Async project-git commit completed. The dispatcher
    /// removes the `(project_root, rel_path)` entry from
    /// `inflight_git_commits` and logs success/failure. `result.Ok`
    /// carries the formatted commit OID; `result.Err` carries the
    /// error string. Best-effort — a failure here doesn't roll back
    /// the on-disk save (data is already on disk; this just means git
    /// didn't capture it).
    GitCommitDone {
        project_root: std::path::PathBuf,
        rel_path: std::path::PathBuf,
        result: Result<String, String>,
    },
}

/// Edit-command message family (ADR-0001 D3). Namespaced under
/// `Message::Edit` and routed to `dispatch_edit_message`.
#[derive(Debug, Clone)]
pub enum EditMsg {
    /// Delete the current selection. In a footprint editor this routes
    /// to the footprint dispatcher's `DeleteSelected`; otherwise the
    /// schematic engine removes the selected elements.
    DeleteSelected,
    /// Undo the most recent edit.
    Undo,
    /// Redo the most recently undone edit.
    Redo,
    /// Rotate the current selection.
    RotateSelected,
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
}
