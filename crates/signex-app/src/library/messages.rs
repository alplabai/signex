//! Library subsystem message tree.
//!
//! Mirrors the existing `Message` → dispatcher → handler split used across
//! the rest of `signex-app`. The top-level `LibraryMessage` is folded into
//! [`crate::app::contracts::Message::Library`]; each sub-enum routes to a
//! purpose-built handler.
//!
//! Keep variants small and copy-cheap where possible — these messages
//! ride through the entire iced update tree, including for the multi-
//! window editor surface (one editor window per `ComponentId`).

use std::path::PathBuf;

use signex_library::{
    AlternateStatus, BodyShape, ComponentClass, ComponentSummary, DistributorSource,
    LifecycleState, PrimitiveKind, PrimitiveRef, RowId, SimKind, SimModel, UseSite,
};
use uuid::Uuid;

use super::state::{EditorAddress, PreviewTab, PrimitivePickerTarget};

// WS-5 (DBLib): kept as type aliases until WS-6 retargets the editor
// at `ComponentPreviewState`. The original `ComponentId` was a
// `uuid::Uuid` newtype; `Version` was a `u32` revision counter. Both
// fold away once the row tier ships everywhere.
#[allow(dead_code)]
pub type ComponentId = Uuid;
#[allow(dead_code)]
pub type Version = u32;

/// Top-level library message — folded into [`Message::Library`].
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum LibraryMessage {
    /// File ▸ Library ▸ Open Library… — runs `rfd::AsyncFileDialog`
    /// on the directory level and lands in [`LibraryMessage::OpenLibraryAt`].
    OpenLibraryDialog,
    /// Result of the `rfd` directory pick. `None` = user cancelled.
    OpenLibraryAt(Option<PathBuf>),
    /// Close an open library (drops the adapter + every editor window
    /// pointing at it). No-op when the path isn't currently open.
    CloseLibrary(PathBuf),
    /// Show the close-library confirmation modal carrying the list of
    /// dirty editor addresses the user is about to lose. Keyed by
    /// `EditorAddress` because Component Preview editors live as tabs,
    /// not as OS windows.
    ConfirmCloseLibrary {
        library_path: PathBuf,
        dirty_editors: Vec<EditorAddress>,
    },
    /// User picked Save All / Discard All / Cancel in the close prompt.
    CloseLibraryConfirm(CloseLibraryChoice),
    /// File ▸ Library ▸ Place Component… — opens the picker modal.
    OpenPicker,
    /// Dismiss the picker modal (Esc / X / outside click).
    ClosePicker,
    // ── New Component flow ────────────────────────────────────────────
    /// File ▸ Library ▸ New Component… — opens the New Component modal.
    NewComponent,
    /// Project tree → right-click → Add New to Project ▸ Component
    /// Library. Carries the active project's root directory; the
    /// dispatcher opens an `rfd::AsyncFileDialog::save_file()` with
    /// `.snxlib` filter so the user picks the library's name and
    /// location (defaults to `<root>/<project>-lib.snxlib` for the
    /// common project-local case, but the user can navigate anywhere
    /// for a shared library). On confirm, dispatches
    /// `CreateLibraryAtPath` with the chosen path.
    CreateLibraryAt(std::path::PathBuf),
    /// Resolution of the "New Component Library" save-as dialog —
    /// `project_path` is the project the library should attach to
    /// (the same file the original `CreateLibraryAt` carried);
    /// `lib_path` is the user's chosen `.snxlib/` directory path.
    /// Opens the "Library Options" modal seeded with `(project_path,
    /// lib_path, use_lfs = false)` so the user can opt into Git LFS
    /// for binary 3D models. Confirming the modal calls
    /// `crate::library::commands::register_pending_library` —
    /// nothing hits disk until the user saves the project (Ctrl+S).
    CreateLibraryAtPath {
        project_path: std::path::PathBuf,
        lib_path: std::path::PathBuf,
    },
    /// "Library Options" modal — toggle the "Use Git LFS for binary
    /// 3D models" checkbox. Default is off.
    LibraryCreateOptionsToggleLfs,
    /// Toggle the "Enable version control" checkbox on the New
    /// Library Options modal. Off by default — fresh libraries land
    /// as plain files; flipping this on triggers `git init` at
    /// create time.
    LibraryCreateOptionsToggleGit,
    /// "Library Options" modal — Create Library button. Registers
    /// a pending library via
    /// `commands::register_pending_library` and stashes the spec on
    /// `LoadedProject.pending_libraries`. The actual `.snxlib/`
    /// directory + git scaffolding are deferred to project save
    /// (`commands::materialize_pending_library`), per the
    /// "wait for explicit user save" invariant.
    LibraryCreateOptionsConfirm,
    /// "Library Options" modal — Cancel button (or Esc). Drops the
    /// modal state without creating anything.
    LibraryCreateOptionsCancel,
    /// F34 — Save-As dialog confirmed for a new symbol library file
    /// (`.snxsym`). Writes an empty `SymbolFile` to the picked path,
    /// refreshes the cached library file enumeration so the project
    /// tree shows the new node, and opens the file as a primitive
    /// editor tab.
    AddLibrarySymbolFilePicked(PathBuf),
    /// F34 — Save-As dialog confirmed for a new footprint library
    /// file (`.snxfpt`). Writes an empty `Footprint` to the picked
    /// path, refreshes the cached library file enumeration, and
    /// opens the file as a primitive editor tab.
    AddLibraryFootprintFilePicked(PathBuf),
    /// Dismiss the New Component modal without creating anything.
    CloseNewComponent,
    /// Live-edit of the New Component modal's "Internal PN" field.
    NewComponentSetInternalPn(String),
    /// User picked a target library in the modal — index into
    /// `LibraryState.open_libraries`.
    NewComponentSetLibrary(usize),
    /// User picked a class in the modal pick_list.
    NewComponentSetClass(ComponentClass),
    /// User picked a target table (filename stem) in the modal
    /// pick_list. Rows live inside category tables, so the modal
    /// needs the user to pick a destination table — populated from
    /// `manifest().tables()` plus the default `<class>s` slot when
    /// the manifest declares no overrides.
    NewComponentSetTable(String),
    /// User picked the "+ New Table…" sentinel in the Table dropdown
    /// (or hit the "+ New Table" button) — opens the inline create
    /// form on `NewComponentState.creating_table`.
    NewComponentBeginCreateTable,
    /// Live-edit of the new-table name field.
    NewComponentSetNewTableName(String),
    /// Cancel the inline create-table form without writing anything.
    NewComponentCancelCreateTable,
    /// Confirm — calls `create_empty_table` on the active library's
    /// adapter, refreshes the components cache, switches the modal's
    /// `table` selection to the freshly-minted name.
    NewComponentConfirmCreateTable,
    /// Toggle the Advanced ▾ disclosure on the New Component modal —
    /// shows / hides the Table picker + `+ New Table…` so the basic
    /// form stays clean for first-time users.
    NewComponentToggleAdvanced,
    /// Library Browser tab strip → "+ Add Table". Flips the
    /// browser into add-table mode where the strip's right edge
    /// shows a name input + Confirm/Cancel.
    BrowserBeginAddTable {
        library_path: PathBuf,
    },
    /// Live-edit of the `+ Add Table` name buffer.
    BrowserSetNewTableName {
        library_path: PathBuf,
        value: String,
    },
    /// Cancel the inline `+ Add Table` form without writing.
    BrowserCancelAddTable {
        library_path: PathBuf,
    },
    /// Confirm `+ Add Table` — calls `create_empty_table` on the
    /// adapter, refreshes the browser cache, switches the active
    /// tab to the new table.
    BrowserConfirmAddTable {
        library_path: PathBuf,
    },
    /// Delete an empty table from the strip's per-tab `×` button.
    /// Adapter refuses non-empty deletes with `Conflict`; the error
    /// surfaces in `LibraryBrowserState.delete_error` so the strip
    /// can show "table is not empty" inline.
    BrowserDeleteTable {
        library_path: PathBuf,
        table: String,
    },
    /// Dismiss the inline delete-table error message.
    BrowserDismissDeleteError {
        library_path: PathBuf,
    },
    /// Sidebar `✎` rename button — flip the matching row into edit
    /// mode (text input replaces label).
    BrowserBeginRenameTable {
        library_path: PathBuf,
        table: String,
    },
    /// Live-edit of the inline rename buffer.
    BrowserSetRenameName {
        library_path: PathBuf,
        value: String,
    },
    /// Cancel the inline rename without writing.
    BrowserCancelRenameTable {
        library_path: PathBuf,
    },
    /// Confirm — calls `rename_table` on the adapter, swaps every
    /// in-memory reference to the table over to the new name.
    BrowserConfirmRenameTable {
        library_path: PathBuf,
    },
    /// Sidebar `+ Class` — opens the inline create form on
    /// `LibraryBrowserState.adding_class`.
    BrowserBeginAddClass {
        library_path: PathBuf,
    },
    BrowserSetNewClassKey {
        library_path: PathBuf,
        value: String,
    },
    BrowserSetNewClassLabel {
        library_path: PathBuf,
        value: String,
    },
    BrowserCancelAddClass {
        library_path: PathBuf,
    },
    /// Append the new class to the library's `[[classes]]` block via
    /// `update_library_classes` and refresh.
    BrowserConfirmAddClass {
        library_path: PathBuf,
    },
    /// Per-row `×` delete — drops the matching class from the
    /// library's `[[classes]]` block. Components referencing the
    /// class by key keep their stored class string; the dropdown
    /// just stops surfacing it.
    BrowserDeleteClass {
        library_path: PathBuf,
        key: String,
    },
    /// Sidebar `✎` rename for a class row — flips the matching row
    /// into edit mode (key + label inputs).
    BrowserBeginRenameClass {
        library_path: PathBuf,
        key: String,
    },
    BrowserSetRenameClassKey {
        library_path: PathBuf,
        value: String,
    },
    BrowserSetRenameClassLabel {
        library_path: PathBuf,
        value: String,
    },
    BrowserCancelRenameClass {
        library_path: PathBuf,
    },
    /// Confirm — writes the renamed class via `update_library_classes`.
    BrowserConfirmRenameClass {
        library_path: PathBuf,
    },
    /// Live-edit of the modal's "Category" field.
    NewComponentSetCategory(String),
    /// Submit the New Component modal — creates the draft, persists,
    /// opens the editor on the new component.
    NewComponentSubmit,
    // ─────────────────────────────────────────────────────────────────
    /// Toggle the Library left-dock panel's library tree node at
    /// `path` (path relative to the open libraries list).
    ToggleLibraryTreeNode(usize),
    /// Open a Component Preview tab for the row identified by
    /// `(library_path, table, row_id)`.
    OpenComponentRow {
        library_path: PathBuf,
        table: String,
        row_id: RowId,
    },
    /// Open a standalone primitive editor tab for the file at `path`.
    /// Fired by the Component Preview tab's right-click context menu
    /// on the Symbol / Footprint render panes; routed to the
    /// standalone `.snxsym` / `.snxfpt` document tab.
    OpenPrimitiveEditor {
        path: PathBuf,
    },
    /// Inner Component Preview message — keyed by
    /// `(library_path, table, row_id)`.
    EditorEvent {
        library_path: PathBuf,
        table: String,
        row_id: RowId,
        msg: EditorMsg,
    },
    /// Picker modal interaction.
    Picker(PickerMsg),
    /// Settings ▸ Library ▸ Distributor APIs panel updates.
    Settings(SettingsMsg),
    /// Click a Where-Used row in the editor → jump to the project /
    /// sheet / instance.
    JumpToUseSite(UseSite),
    /// No-op sink — used by the diff preview canvases in the History tab.
    Noop,
    /// Picker → user clicked Place. Embeds the library row into the
    /// active schematic engine.
    PlaceLibraryComponent {
        library_path: PathBuf,
        table: String,
        row_id: RowId,
    },
    /// Internal trace-only signal: a Component Preview tab was
    /// opened for the given address. Fired alongside
    /// `OpenComponentRow` so downstream observers can attach to a
    /// single message.
    ComponentPreviewOpened {
        path: PathBuf,
        table: String,
        row_id: RowId,
    },
    /// Inner-message envelope for events fired from a standalone
    /// primitive editor tab (`.snxsym` / `.snxfpt`). Keyed by file
    /// path so the dispatcher can locate the matching
    /// `SymbolEditorState` / `FootprintEditorState` in
    /// `DocumentState.symbol_editors` / `footprint_editors`. Mirrors
    /// the `EditorEvent` shape used for Component Preview tabs but
    /// with a path identity instead of `(library_path, table, row_id)`.
    PrimitiveEditorEvent {
        path: PathBuf,
        msg: PrimitiveEditorMsg,
    },
    // ── Library Browser tab ──────────────────────────────────────────
    /// Open the Library Browser tab for `.snxlib` at `library_path`.
    /// Mounts the library if not already mounted, then pushes a
    /// `TabKind::LibraryBrowser(path)` tab (or activates the existing
    /// one when the path is already open).
    OpenLibraryBrowser(PathBuf),
    /// Active table change inside a Library Browser tab — clicked one
    /// of the category tabs in the strip.
    BrowserSelectTable {
        library_path: PathBuf,
        table: String,
    },
    /// Search-buffer edit inside a Library Browser tab.
    BrowserSearchChanged {
        library_path: PathBuf,
        value: String,
    },
    /// Column-header click — toggles sort direction on the matching
    /// key, or sets ascending sort if a different column is clicked.
    /// Stage 8 of `v0.9-snxlib-as-file-plan.md`. The `column_key` is
    /// the stable identifier from `ColumnKind::sort_key()`
    /// (`"internal_pn"`, `"manufacturer"`, `"mpn"`, `"tags"`, or
    /// `"parameters.<key>"`).
    BrowserSortColumn {
        library_path: PathBuf,
        column_key: String,
    },
    /// Row click inside the browser grid — selects the row, drives
    /// the side preview pane.
    BrowserSelectRow {
        library_path: PathBuf,
        table: String,
        row_id: RowId,
    },
    /// User clicked the Add Component button (inside the empty-state
    /// CTA, the action row, or the "+" tab). Pre-sets the New
    /// Component modal to the active library + table.
    BrowserAddComponent {
        library_path: PathBuf,
        /// Pre-selected destination table — `None` from the empty
        /// state CTA when the library has no tables yet.
        table: Option<String>,
    },
    /// User clicked Delete Selected on the browser action row. Phase 2
    /// — opens the confirm modal; the actual delete happens via
    /// `BrowserDeleteRowConfirm` only if the user confirms.
    BrowserDeleteRowRequest {
        library_path: PathBuf,
        table: String,
        row_id: RowId,
    },
    /// User clicked Delete in the confirm modal — fires the actual
    /// `adapter.delete_row` call.
    BrowserDeleteRowConfirm {
        library_path: PathBuf,
        table: String,
        row_id: RowId,
    },
    /// User dismissed the delete confirm modal without deleting.
    BrowserDeleteRowCancel {
        library_path: PathBuf,
    },
    /// Open the Symbol/Footprint primitive picker modal. `target`
    /// determines what happens when the user picks something.
    OpenPrimitivePicker {
        kind: PrimitiveKind,
        target: PrimitivePickerTarget,
    },
    /// Inner-message envelope for primitive picker events.
    PrimitivePicker(PrimitivePickerMsg),
    /// Double-click on a row in the browser grid → opens the Edit
    /// Component Details modal. Loads the row from the cached tables
    /// and seeds `EditRowModalState.draft`.
    BrowserOpenEditModal {
        library_path: PathBuf,
        table: String,
        row_id: RowId,
    },
    /// Inner-message envelope for events fired by the Edit Component
    /// Details modal. Keyed by library_path so the dispatcher can find
    /// the matching `LibraryBrowserState.edit_modal`.
    BrowserEdit {
        library_path: PathBuf,
        msg: BrowserEditMsg,
    },
    /// Live edit of a cell in the browser grid (Deliverable C).
    /// Updates the per-cell edit buffer; the change is flushed to the
    /// row on `BrowserCellCommit` (Enter / blur).
    BrowserCellEdit {
        library_path: PathBuf,
        row_id: RowId,
        column: String,
        value: String,
    },
    /// Commit the per-cell edit buffer to the row and persist via
    /// `adapter.update_row`. Drops the buffer entry on success.
    BrowserCellCommit {
        library_path: PathBuf,
        table: String,
        row_id: RowId,
        column: String,
    },
    /// Drop the per-cell edit buffer (Esc).
    BrowserCellCancel {
        library_path: PathBuf,
        row_id: RowId,
        column: String,
    },
    /// Pick a lifecycle filter mode for the active Library Browser tab.
    /// Drives which rows render in the grid (Stage 18 of
    /// `v0.9-snxlib-as-file-plan.md` — surfaces `ComponentRow.state` as
    /// a first-class browser filter).
    BrowserSetLifecycleFilter {
        library_path: PathBuf,
        filter: super::state::LifecycleFilter,
    },
    /// Toggle the per-class filter. `Some(key)` sets it; passing
    /// the currently-active key flips back to `None`.
    BrowserClassFilterClicked {
        library_path: PathBuf,
        key: String,
    },
    /// Right-click on a Library Browser row → "Refresh Pricing".
    /// Stage 18 stub — real distributor adapter wiring lands later.
    BrowserRefreshPricing {
        library_path: PathBuf,
        table: String,
        row_id: RowId,
    },
    /// Library node right-click → "Refresh All Pricing".
    /// Stage 18 stub — touches every row in every cached table once
    /// the per-row stub is fleshed out.
    LibraryRefreshAllPricing(PathBuf),
    // ── Tools ▸ Document Options modal ──────────────────────────────
    /// Tools menu fired Document Options for the library at
    /// `library_path`. Opens the modal pre-filled with the library's
    /// current display settings.
    OpenDocumentOptions {
        library_path: PathBuf,
    },
    /// Modal — pick a new sheet color preset.
    DocumentOptionsSetSheetColor(crate::panels::SheetColor),
    /// Modal — toggle the visible-grid checkbox.
    DocumentOptionsToggleGrid,
    /// Modal — cycle the visible grid spacing.
    DocumentOptionsCycleGridSize,
    /// Modal — cycle the coordinate display unit.
    DocumentOptionsCycleUnit,
    /// Modal — apply the draft to the library and close.
    DocumentOptionsApply,
    /// Modal — drop the draft and close.
    DocumentOptionsCancel,

    // Recovery dialogs (Stage 10 of v0.9-snxlib-as-file).
    RecoveryLibraryMissing(super::recovery::LibraryMissingChoice),
    RecoveryLibraryMissingLocateResult(Option<PathBuf>),
    RecoveryGitMissing(super::recovery::GitMissingChoice),
    RecoveryBrokenBinding(super::recovery::BrokenBindingChoice),

    // ── Library Updates Available modal (Stage 16) ─────────────────────
    /// Internal: the schematic-open scan finished and produced drift —
    /// open the modal in Team mode, or silently apply in Personal
    /// mode. Carried as a single message so the dispatcher's open path
    /// stays linear (scan first, decide branch, fire this message).
    /// The dispatcher mounts the state directly when this fires; no
    /// payload because the state is already on `LibraryState`.
    /// Toggle one row's checkbox in the modal — the symbol_uuid keys
    /// the entry inside the modal state's `entries` vec.
    LibraryUpdatesToggleSelection(uuid::Uuid),
    /// User clicked Update Selected Components — apply the picked
    /// updates to the schematic engine + dirty-mark + close modal.
    LibraryUpdatesApply,
    /// User clicked Skip All — close the modal and record the path on
    /// `skipped_updates_for` so the status bar can flag it persistently.
    LibraryUpdatesSkipAll,
    /// User dismissed the modal (Cancel / Esc / X). No state mutates;
    /// drift is left pinned and the schematic stays clean.
    LibraryUpdatesCancel,

    // ── Components Panel (Stage 9 of v0.9-snxlib-as-file) ───────────
    /// Toggle the collapse flag for the named section
    /// ("project" / "installed" / "global"). The dispatcher looks
    /// the section up by name so adding a fourth section later
    /// doesn't ripple through the message enum.
    ComponentsPanelToggleSection(super::state::ComponentsMountSource),
    /// Live edit of the Components Panel filter input. Substring
    /// matched across mpn / manufacturer / internal_pn / library
    /// name. Stage 9 ships this; the rich syntax (plan §5) is
    /// follow-up work.
    ComponentsPanelSetFilter(String),
    /// "+ Add Library…" button on the Installed / Global section
    /// header. Opens an `rfd::AsyncFileDialog` with `*.snxlib`
    /// filter and lands in `ComponentsPanelAddLibraryAt`.
    ComponentsPanelAddLibrary(super::state::ComponentsMountSource),
    /// Result of the Add Library file dialog. `None` = user
    /// cancelled. Carries the source the picker was opened against
    /// so the dispatcher knows whether to push onto
    /// `installed_libraries` or `global_libraries`.
    ComponentsPanelAddLibraryAt {
        source: super::state::ComponentsMountSource,
        path: Option<PathBuf>,
    },
    /// Promote an Installed library to Global — moves the path from
    /// the session-scoped Vec to the persisted TOML file. No-op when
    /// the path isn't currently Installed.
    #[allow(dead_code)]
    ComponentsPanelPromoteToGlobal(PathBuf),
    /// "Manage…" button on the Global section header — opens the
    /// global libraries management dialog. Stage 9 stub: logs and
    /// no-ops so the wiring path is observable.
    #[allow(dead_code)]
    ComponentsPanelManageGlobal,
    /// "Add to Project" button on a Components Panel row — adds
    /// the row's library to the active project's
    /// `Project.libraries` list. Stage 9 stub.
    #[allow(dead_code)]
    ComponentsPanelAddToProject {
        library_path: PathBuf,
    },
    /// "Place into Schematic" button on a Components Panel row.
    /// Stage 9 stub — full ghost-component drag is polish work.
    #[allow(dead_code)]
    ComponentsPanelPlace {
        library_path: PathBuf,
        table: String,
        row_id: RowId,
    },
}

/// User choice from the close-library confirmation modal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum CloseLibraryChoice {
    SaveAll,
    DiscardAll,
    Cancel,
}

/// Component Preview inner messages. The surface is preview-only
/// for Symbol/Footprint; the canvas messages stay defined here so
/// the standalone `.snxsym` / `.snxfpt` document tabs can reuse
/// them, but they no longer dispatch through the Component Preview
/// tab.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum EditorMsg {
    /// User clicked a Preview tab pill (Preview / Parameters / Supply /
    /// Datasheet / Simulation).
    SelectTab(PreviewTab),
    /// Save the current row to the table — calls
    /// `adapter.update_row(&table, &row, "edit message")`.
    SaveDraft,
    /// Same as [`SaveDraft`] for the Component Preview surface — kept
    /// distinct so future Commit semantics (lifecycle promotion etc.)
    /// can layer in without renaming the SaveDraft message.
    Commit,
    /// Open the review-request UI.
    SubmitForReview,
    SubmitForReviewNotesChanged(String),
    SubmitForReviewConfirm,
    SubmitForReviewCancel,
    SubmitForReviewResult(Result<(), String>),
    /// Footer "Where Used" — switches the active preview tab to
    /// Preview (the where-used footer line lives there).
    OpenWhereUsedTab,
    /// User dismissed the preview tab (Close X or Ctrl+W).
    CloseEditor,

    // ── Datasheet tab ────────────────────────────────────────
    /// Switch the datasheet picker between URL / Pinned PDF modes.
    DatasheetSetMode(crate::library::editor::datasheet_picker::DatasheetMode),
    /// Live edit of the URL field on the Datasheet tab.
    DatasheetSetUrl(String),
    /// Open the Pinned-PDF upload dialog.
    DatasheetUploadDialog,
    /// Async result of the Pinned-PDF upload — `Some((bytes, filename))`
    /// on pick, `None` on cancel.
    DatasheetUploadResult(Option<(Vec<u8>, String)>),

    // ── Component-level setters ─────────────────────────────
    /// Set the row's lifecycle state from the Preview tab header.
    SetLifecycle(LifecycleState),

    // ── Pin Map (Preview-tab inline subsection) ─────────────
    /// Toolbar — clear every override and revert to default 1:1 by
    /// pin/pad number equality.
    PinMapAutoMatchByNumber,
    /// Toolbar — match by pin name → pad number where unambiguous.
    /// Stub: emits a tracing warn until the name-based heuristic
    /// ships in a follow-up patch (see plan §12 task list).
    PinMapAutoMatchByName,
    /// Toolbar — drop every entry in `Revision::pin_map_overrides`.
    /// Equivalent to `PinMapAutoMatchByNumber` for the v0.9 algorithm.
    PinMapClearOverrides,
    /// Click "[Override]" on a row — expands the inline editor for
    /// that pin's row. Carries the symbol pin number.
    PinMapOpenOverrideEdit(String),
    /// Live edit of the override pad-number text input. The dispatcher
    /// keeps the buffer on `PinMapTabState.override_buf`.
    PinMapOverrideBufChanged {
        pin: String,
        value: String,
    },
    /// User clicked "Save" inside the inline editor — push a
    /// `PinPadOverride` onto the active draft.
    PinMapAddOverride {
        pin: String,
        pad: String,
    },
    /// User clicked "Cancel" inside the inline editor — discard the
    /// edit buffer + collapse the row.
    PinMapCancelOverrideEdit,
    /// User clicked "Remove" on an overridden row — drops that pin's
    /// entry from `Revision::pin_map_overrides`.
    PinMapRemoveOverride {
        pin: String,
    },

    // ── Symbol canvas (used by standalone .snxsym tab) ─────────
    /// Set the active drawing tool on the Symbol canvas.
    SymbolSetTool(SymbolToolMsg),
    /// Click-to-place a pin on the symbol canvas at the given grid-
    /// snapped (mm) world position.
    SymbolAddPin {
        x: f64,
        y: f64,
    },
    /// Select a symbol element (pin index / field key) — emitted by
    /// the canvas hit-test on left-click.
    SymbolSelect(SymbolSelectionMsg),
    /// Click landed on empty canvas — drop the current selection.
    SymbolDeselect,
    /// Drag the currently-selected element to a new grid-snapped
    /// world position. Field drag is a no-op for now.
    SymbolMoveSelected {
        x: f64,
        y: f64,
    },
    /// Delete-key — drop the currently-selected element.
    SymbolDeleteSelected,
    /// Properties pane — set the value text of one of the canonical
    /// symbol fields (Designator / Value).
    SymbolSetField {
        key: FieldKeyMsg,
        value: String,
    },
    /// Properties pane — overwrite the pin number string at index.
    SymbolSetPinNumber {
        idx: usize,
        number: String,
    },
    /// Properties pane — overwrite the pin name string at index.
    SymbolSetPinName {
        idx: usize,
        name: String,
    },
    /// Toolbar — open the system file picker for an AI-stub PDF.
    SymbolPickAiPdf,
    /// Async file picker returned — `Some(bytes)` or `None` when the
    /// user cancelled. Wraps the heuristic result inline so the view
    /// can render the preview card without further async hops.
    SymbolPickedAiPdf(Option<Vec<u8>>),
    /// User clicked Apply on the AI preview card.
    SymbolApplyAiPreview,
    /// User clicked Cancel on the AI preview card.
    SymbolDismissAiPreview,
    /// Fire-and-forget save of the active symbol primitive — typically
    /// chained off SaveDraft via the dispatcher. Boxed so the
    /// containing enum stays cheap to clone and propagate.
    SaveSymbol(uuid::Uuid, Box<signex_library::Symbol>),

    // ── Footprint canvas (used by standalone .snxfpt tab) ──────
    /// Click-to-place a pad at the given world position. Fires from
    /// the canvas program on a press-without-drag.
    FootprintAddPad {
        x_mm: f64,
        y_mm: f64,
    },
    /// v0.18.12 — click-to-place a non-plated through hole. Fires
    /// from the canvas program when `PadsTool::PlaceHole` is active.
    FootprintAddHole {
        x_mm: f64,
        y_mm: f64,
    },
    /// v0.18.15 — click-to-place a silk-layer text label.
    FootprintAddText {
        x_mm: f64,
        y_mm: f64,
    },
    /// v0.18.15.1 — click during a Place Track gesture. The
    /// dispatcher decides whether this is the first or second
    /// click based on `editor.state.track_first`.
    FootprintTrackClick {
        x_mm: f64,
        y_mm: f64,
    },
    /// v0.18.15.1 — Esc / right-click during a Place Track
    /// gesture. Clears `editor.state.track_first` so the next
    /// click starts a fresh segment.
    FootprintTrackCancel,
    /// v0.18.15.3 — click during a Place Arc 3-click gesture. The
    /// dispatcher uses `state.place_arc_pending` to decide whether
    /// this click stashes the centre, the start, or commits the
    /// arc.
    FootprintArcClick {
        x_mm: f64,
        y_mm: f64,
    },
    /// v0.18.15.3 — Esc / right-click during Place Arc.
    FootprintArcCancel,
    /// v0.18.15.4 — Place Polygon click (appends a vertex).
    FootprintPolygonClick {
        x_mm: f64,
        y_mm: f64,
    },
    /// v0.18.15.4 — commit the in-flight polygon to silk_f.
    FootprintPolygonCommit,
    /// v0.18.15.4 — drop the in-flight polygon stash.
    FootprintPolygonCancel,
    /// v0.18.18 — select a silk-front graphic (or clear with `None`).
    FootprintSelectSilkF(Option<usize>),
    /// v0.18.18 — delete the selected silk-front graphic. No-op
    /// when `selected_silk_f` is `None`.
    FootprintDeleteSilkF,
    /// Drag the pad at `idx` to a new world position.
    FootprintMovePad {
        idx: usize,
        x_mm: f64,
        y_mm: f64,
    },
    /// Cursor moved over the canvas — drives the footer X/Y readout.
    FootprintCursorAt {
        x_mm: f64,
        y_mm: f64,
    },
    /// v0.13.1 Phase 6.3 — Sketch-mode click-to-place. Fires when
    /// `EditorMode::Sketch` is active and the user clicks empty
    /// canvas. Routes through the dispatcher's
    /// `FootprintSketchPlacePoint` handler which adds the Point +
    /// runs solve + bake.
    FootprintSketchPlacePoint {
        x_mm: f64,
        y_mm: f64,
    },
    /// v0.13.2 Phase 6.4 — Sketch-mode multi-click tool gesture
    /// (Line / Circle / Arc). Carries the snap-to-existing-point id
    /// when the click landed within `SNAP_RADIUS_PX` of an existing
    /// sketch Point. The dispatcher advances the active tool's
    /// state machine and emits the gesture-completing AddEntity.
    FootprintSketchToolClick {
        x_mm: f64,
        y_mm: f64,
        snap_id: Option<signex_sketch::id::SketchEntityId>,
    },
    /// v0.13.2 — Escape during a multi-click gesture; clears
    /// `tool_pending` without emitting a SketchEdit.
    FootprintSketchToolEscape,
    /// v0.13.3 — Sketch entity selection from canvas. `None` = clear.
    FootprintSketchSelect {
        id: Option<signex_sketch::id::SketchEntityId>,
        shift: bool,
    },
    /// v0.13.3 — Drag-move a Point entity by `(dx, dy)` mm.
    FootprintSketchMovePoint {
        id: signex_sketch::id::SketchEntityId,
        dx: f64,
        dy: f64,
    },
    /// Select / deselect a pad. `None` deselects everything.
    FootprintSelectPad(Option<usize>),
    /// Delete-key — remove the currently-selected pad.
    FootprintDeleteSelected,
    /// Toolbar — toggle a layer's visibility. Carries the Standard layer
    /// name string; the dispatcher maps to `FpLayer`.
    FootprintToggleLayer(String),
    /// Toolbar — toggle the auto-fit-courtyard flag.
    FootprintToggleAutoFit,
    /// v0.15 — Pads-mode tool switch (Select / PlacePad). Right-
    /// click on the canvas publishes this with `Select` to cancel
    /// the active tool.
    FootprintSetPadsTool(crate::library::editor::footprint::state::PadsTool),
    /// v0.15 — Sketch-mode tool switch (Select / Point / Line /
    /// Circle / Arc). Right-click on the canvas publishes this
    /// with `Select` to cancel the active tool / pending gesture.
    FootprintSketchSetTool(crate::library::editor::footprint::state::SketchTool),
    /// v0.16.1 — toggle construction-mode. Sticky pill on the
    /// sketch active bar; when on, every newly-minted entity gets
    /// `construction = true`.
    FootprintSketchToggleConstruction,
    /// v0.16.1 — TAB pause/resume during pad placement. Toggles
    /// `state.placement_paused`; while `true` the canvas ignores
    /// empty-canvas clicks so the user can adjust defaults.
    FootprintTogglePlacementPause,
    /// v0.16.2 — assign / clear a role attr (PadAttr / SilkAttr /
    /// CourtyardAttr / etc.) on the entity at `id`. The dispatcher
    /// clears every `*Attr` slot first, then writes the matching one
    /// with sensible defaults. Pad on a non-Point is a silent no-op.
    FootprintSketchSetRole {
        id: signex_sketch::id::SketchEntityId,
        role: RoleTag,
    },
    /// Fire-and-forget save of the active footprint primitive. Boxed
    /// so the containing enum stays cheap to clone and propagate.
    SaveFootprint(uuid::Uuid, Box<signex_library::Footprint>),
    /// Body 3D editor pane — set extruded body height (mm).
    SetBodyHeight(f32),
    /// Body 3D editor pane — set body offset above PCB (mm).
    SetBodyOffsetZ(f32),
    /// Body 3D editor pane — set the body-top RGBA colour.
    SetBodyTopColor([f32; 4]),
    /// Body 3D editor pane — set the body-side RGBA colour.
    SetBodySideColor([f32; 4]),
    /// Body 3D editor pane — set the procedural shape variant.
    SetBodyShape(BodyShape),
    /// STEP attach — open the system file picker.
    StepAttachDialog,
    /// Async file picker returned. `Some((bytes, filename))` on pick,
    /// `None` on cancel.
    StepAttachResult(Option<(Vec<u8>, String)>),
    /// Drop the existing STEP attachment from the footprint primitive.
    StepAttachRemove,

    // ── Supply tab ────────────────────────────────────────────
    // Primary MPN
    /// Edit the primary MPN's manufacturer string.
    SupplyPrimarySetManufacturer(String),
    /// Edit the primary MPN's MPN string.
    SupplyPrimarySetMpn(String),
    /// Pick the primary MPN's approval status.
    SupplyPrimarySetStatus(AlternateStatus),
    /// Edit the primary MPN's free-form notes.
    SupplyPrimarySetNotes(String),

    // Alternates
    /// Append a fresh blank alternate row.
    SupplyAlternateAdd,
    /// Edit the manufacturer of the alternate at `idx`.
    SupplyAlternateSetManufacturer {
        idx: usize,
        value: String,
    },
    /// Edit the MPN of the alternate at `idx`.
    SupplyAlternateSetMpn {
        idx: usize,
        value: String,
    },
    /// Pick the approval status of the alternate at `idx`.
    SupplyAlternateSetStatus {
        idx: usize,
        value: AlternateStatus,
    },
    /// Edit the free-form notes of the alternate at `idx`.
    SupplyAlternateSetNotes {
        idx: usize,
        value: String,
    },
    /// Drop the alternate row at `idx`.
    SupplyAlternateRemove {
        idx: usize,
    },

    // Distributor listings
    /// Append a fresh blank distributor listing row.
    SupplyListingAdd,
    /// Pick the distributor source for the listing at `idx`. The
    /// dispatcher converts `DistributorSource` to the canonical string
    /// stored on `DistributorListing.distributor`.
    SupplyListingSetDistributor {
        idx: usize,
        value: DistributorSource,
    },
    /// Edit the SKU of the distributor listing at `idx`.
    SupplyListingSetSku {
        idx: usize,
        value: String,
    },
    /// Edit the URL of the distributor listing at `idx`. Empty string
    /// clears the field back to `None`.
    SupplyListingSetUrl {
        idx: usize,
        value: String,
    },
    /// Drop the distributor listing row at `idx`.
    SupplyListingRemove {
        idx: usize,
    },
    // ── Parameters tab ────────────────────────────────────────
    /// Set a `ParamValue::Text` parameter's value directly. Text inputs
    /// can flush on every keystroke without a parse step.
    ParamSetText {
        name: String,
        value: String,
    },
    /// Live-update the per-row edit buffer for a `ParamValue::Number`
    /// row. The buffer lives on `ComponentEditorState.params_edit_buf`;
    /// the value is committed via `ParamCommitNumber`.
    ParamSetNumberBuf {
        name: String,
        buf: String,
    },
    /// Commit the live buffer for a `ParamValue::Number` row.
    ParamCommitNumber {
        name: String,
    },
    /// Live-update the per-row edit buffer for a `ParamValue::Measurement`
    /// row's value field.
    ParamSetMeasurementBuf {
        name: String,
        buf: String,
    },
    /// Commit the live buffer for a `ParamValue::Measurement` row.
    ParamCommitMeasurement {
        name: String,
        unit: String,
    },
    /// Toggle a `ParamValue::Bool` parameter.
    ParamSetBool {
        name: String,
        value: bool,
    },
    /// Drop a parameter from `draft.parameters`.
    ParamRemove {
        name: String,
    },
    /// Add a custom parameter row with an empty value of the chosen kind.
    ParamAddCustom {
        name: String,
        kind: ParamKindMsg,
    },
    // ── Simulation tab ────────────────────────────────────────
    /// Toggle the "Has SPICE Model" checkbox. `true` constructs a fresh
    /// `SimModel` and binds it via `Revision::sim_ref`; `false` clears
    /// both `editor.sim` and `editor.draft.sim_ref`.
    SimSetEnabled(bool),
    /// SPICE dialect picker — Spice3 / Ngspice / LtSpice / VerilogA.
    SimSetKind(SimKind),
    /// Live edit of the SimModel `name` field.
    SimSetName(String),
    /// Multi-line edit on the SPICE deck `text_editor`. Action is
    /// applied to `editor.sim_body`; the resulting text mirrors back
    /// onto `editor.sim?.body` so persistence picks it up on save.
    SimBodyAction(iced::widget::text_editor::Action),
    /// Set or clear the SPICE node binding for one symbol pin number.
    /// Empty `value` removes the key from `default_node_map`.
    SimSetPinNode {
        pin_number: String,
        value: String,
    },
    /// Fire-and-forget save of the active SimModel primitive.
    SaveSim(uuid::Uuid, Box<SimModel>),
}

/// Pure-data alias for `ParamKind` so messages don't depend on
/// `signex_library::ParamKind` at the message layer.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ParamKindMsg {
    Text,
    Number,
    Bool,
    /// Carries the unit string ("ohm", "F", "V", …).
    Measurement(String),
}

/// Tool selection on the Symbol canvas — pure-data alias for the
/// canvas's own `SymbolTool` so messages don't depend on the canvas
/// module type tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum SymbolToolMsg {
    Select,
    AddPin,
    PlaceRectangle,
    PlaceLine,
    PlaceCircle,
    PlaceArc,
    PlaceText,
}

/// v0.13.3 — selection-aware constraint kind tag. The dispatcher
/// resolves these against the editor's primary + secondary
/// selection slots into the matching `ConstraintKind` and emits the
/// SketchEdit. Tags that don't apply to the current selection are
/// no-ops in the dispatcher.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SketchConstraintTag {
    /// 1 Point selected → fix it in place.
    Fixed,
    /// 2 Points selected → make them coincident.
    Coincident,
    /// 2 Points selected + dimension input → DistancePtPt(target_mm).
    DistancePtPt,
    /// 1 Line selected → horizontal.
    Horizontal,
    /// 1 Line selected → vertical.
    Vertical,
    /// 2 Lines selected → parallel.
    Parallel,
    /// 2 Lines selected → perpendicular.
    Perpendicular,
    /// 2 Lines selected → equal length.
    EqualLength,
    /// 1 Point + 1 Line selected → point on line.
    PointOnLine,
    /// 1 Point + 1 Line selected → midpoint.
    Midpoint,
}

/// v0.16.2 — role tag attached to a sketch entity. The Sketch-mode
/// inspector emits one of these via
/// [`PrimitiveEditorMsg::FootprintSketchSetRole`]; the dispatcher
/// clears every `*Attr` slot on the target entity and writes the
/// matching one with sensible defaults. Bake auto-emits whatever
/// geometry the role implies (pad / silk segment / courtyard
/// polygon / mask opening / pour / paste aperture / keepout / board
/// cutout). `Pad` is only valid on a Point — non-Point entities
/// fall through as a silent no-op.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoleTag {
    /// Clear every `*Attr` slot on the entity.
    Unassigned,
    /// `PadAttr` — bakes as an SMD pad. Point-only.
    Pad,
    /// `SilkAttr { layer: TopSilk }` — top-side silkscreen line/arc.
    SilkTop,
    /// `SilkAttr { layer: BottomSilk }`.
    SilkBottom,
    /// `CourtyardAttr` — closed loop becomes the courtyard polygon.
    Courtyard,
    /// `KeepoutAttr` with `NO_ROUTING` defaults on TopCopper.
    Keepout,
    /// `BoardCutoutAttr { through: true }` — board cutout polygon.
    Cutout,
    /// `MaskOpeningAttr { layer: TopSolderMask }`.
    MaskOpeningTop,
    /// `MaskOpeningAttr { layer: BottomSolderMask }`.
    MaskOpeningBottom,
    /// `MaskExcludeAttr { layer: TopSolderMask }`.
    MaskExcludeTop,
    /// `MaskExcludeAttr { layer: BottomSolderMask }`.
    MaskExcludeBottom,
    /// `PourAttr { layer: TopCopper, .. }` with thermal-relief defaults.
    PourTop,
    /// `PourAttr { layer: BottomCopper, .. }`.
    PourBottom,
    /// `PasteApertureAttr { layer: TopPaste }`.
    PasteApertureTop,
    /// `PasteApertureAttr { layer: BottomPaste }`.
    PasteApertureBottom,
}

impl RoleTag {
    /// Display order for the inspector's pick_list. Mirrors the
    /// docstring order on the enum.
    pub const ALL: &'static [RoleTag] = &[
        RoleTag::Unassigned,
        RoleTag::Pad,
        RoleTag::SilkTop,
        RoleTag::SilkBottom,
        RoleTag::Courtyard,
        RoleTag::Keepout,
        RoleTag::Cutout,
        RoleTag::MaskOpeningTop,
        RoleTag::MaskOpeningBottom,
        RoleTag::MaskExcludeTop,
        RoleTag::MaskExcludeBottom,
        RoleTag::PourTop,
        RoleTag::PourBottom,
        RoleTag::PasteApertureTop,
        RoleTag::PasteApertureBottom,
    ];

    /// Human-readable label rendered in the inspector dropdown.
    pub fn label(self) -> &'static str {
        match self {
            RoleTag::Unassigned => "Unassigned",
            RoleTag::Pad => "Pad",
            RoleTag::SilkTop => "Silk · Top",
            RoleTag::SilkBottom => "Silk · Bottom",
            RoleTag::Courtyard => "Courtyard",
            RoleTag::Keepout => "Keepout",
            RoleTag::Cutout => "Board Cutout",
            RoleTag::MaskOpeningTop => "Mask Opening · Top",
            RoleTag::MaskOpeningBottom => "Mask Opening · Bottom",
            RoleTag::MaskExcludeTop => "Mask Exclude · Top",
            RoleTag::MaskExcludeBottom => "Mask Exclude · Bottom",
            RoleTag::PourTop => "Pour · Top",
            RoleTag::PourBottom => "Pour · Bottom",
            RoleTag::PasteApertureTop => "Paste · Top",
            RoleTag::PasteApertureBottom => "Paste · Bottom",
        }
    }
}

impl std::fmt::Display for RoleTag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// Selection target on the Symbol canvas — pure-data version of
/// `editor::symbol::state::SymbolSelection`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum SymbolSelectionMsg {
    Pin(usize),
    FieldReference,
    FieldValue,
    /// A placed `SymbolGraphic` at the given index in the active
    /// symbol's `graphics` vector. Drives the right-dock Properties
    /// panel's Graphic branch.
    Graphic(usize),
}

/// Symbol field key — pure-data alias of
/// `editor::symbol::state::FieldKey`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum FieldKeyMsg {
    Reference,
    Value,
}

/// Resize-handle identity for a Symbol graphic — pure-data alias of
/// `editor::symbol::state::GraphicHandle`. Carried by
/// `PrimitiveEditorMsg::SymbolMoveGraphicHandle` so the dispatcher
/// knows which handle of which graphic the canvas is dragging.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(dead_code)]
pub enum GraphicHandleMsg {
    /// Rectangle corner — `0=TL, 1=TR, 2=BR, 3=BL` (Standard y-up).
    RectCorner(u8),
    /// Line endpoint — `0=from, 1=to`.
    LineEndpoint(u8),
    /// Circle radius handle.
    CircleRadius,
    /// Arc start point on the circumference.
    ArcStart,
    /// Arc end point on the circumference.
    ArcEnd,
    /// Text anchor / `position` field.
    TextAnchor,
}

/// Inner messages for a [`LibraryMessage::PrimitiveEditorEvent`]
/// envelope. Path-keyed dispatch routes each variant to the symbol
/// or footprint editor state stored on `DocumentState` per the
/// active tab's [`crate::app::TabKind`]. Save (Ctrl+S) flows through
/// the existing schematic-save handler — these variants only cover
/// the editor-side mutations + the explicit "save primitive" command.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum PrimitiveEditorMsg {
    // ── Symbol ─────────────────────────────────────────────
    /// Set the active drawing tool on the Symbol canvas.
    SymbolSetTool(SymbolToolMsg),
    /// Click-to-place a pin on the standalone Symbol canvas at the
    /// given grid-snapped (mm) world position.
    SymbolAddPin { x: f64, y: f64 },
    /// Stamp a default-sized 10×5 mm rectangle centred on `(x, y)`.
    SymbolAddRectangle { x: f64, y: f64 },
    /// Stamp a 5 mm horizontal line starting at `(x, y)`.
    SymbolAddLine { x: f64, y: f64 },
    /// Stamp a 2 mm-radius circle (Altium "Ellipse") centred on
    /// `(x, y)`.
    SymbolAddCircle { x: f64, y: f64 },
    /// Stamp a default arc (radius 2 mm, 0°→90° quadrant) centred
    /// on `(x, y)`.
    SymbolAddArc { x: f64, y: f64 },
    /// Stamp a default "Text" label anchored at `(x, y)`. Edit the
    /// content via the Properties panel after placement.
    SymbolAddText { x: f64, y: f64 },
    /// Select a symbol element (pin index / field key).
    SymbolSelect(SymbolSelectionMsg),
    /// Click landed on empty canvas — drop the current selection.
    SymbolDeselect,
    /// Drag the currently-selected element to a new grid-snapped
    /// world position.
    SymbolMoveSelected { x: f64, y: f64 },
    /// Drag-to-resize: move one resize handle of the graphic at
    /// `idx` to grid-snapped world coordinates `(x, y)`. Fires
    /// continuously while the user holds and drags a graphic handle
    /// in the Select tool. The dispatcher mutates the matching field
    /// on `SymbolGraphic.kind` (rect corner / line endpoint / circle
    /// radius / arc angle / text anchor).
    SymbolMoveGraphicHandle {
        idx: usize,
        handle: GraphicHandleMsg,
        x: f64,
        y: f64,
    },
    /// Delete-key — drop the currently-selected element.
    SymbolDeleteSelected,
    /// Properties pane — overwrite the pin number string at index.
    SymbolSetPinNumber { idx: usize, number: String },
    /// Properties pane — overwrite the pin name string at index.
    SymbolSetPinName { idx: usize, name: String },

    // ── View / camera (Altium-style pan/zoom/grid) ─────────
    /// Right- or middle-button pan delta in screen pixels.
    /// Updates `editor.camera.offset`.
    SymbolPan { dx: f32, dy: f32 },
    /// Mouse-wheel zoom centred on the cursor screen position
    /// `(sx, sy)` in canvas-local pixels. Positive `delta` zooms
    /// in, negative zooms out. Updates `editor.camera`.
    SymbolZoom { sx: f32, sy: f32, delta: f32 },
    /// Fit the active symbol's bounding box into the viewport.
    /// Bound to the Home key + the Fit button on the toolbar.
    SymbolFit,
    /// Cursor world position in mm — drives the status footer X/Y
    /// readout. `None` clears the readout when the cursor leaves.
    SymbolCursorAt {
        x_mm: Option<f64>,
        y_mm: Option<f64>,
    },
    /// Toolbar / context-menu — pick the sheet background colour
    /// preset (Black / White / Dark Gray / Light Gray / Cream),
    /// matching Altium's per-document Sheet Color. Applies to the
    /// `.snxlib` containing this `.snxsym` so every primitive
    /// editor opened from the same library shares the colour.
    SymbolSetSheetColor(crate::panels::SheetColor),
    /// Status-footer click on the `Grid` label — toggles whether
    /// the dot grid renders. Applies to the containing `.snxlib`.
    SymbolToggleGrid,
    /// Status-footer click on the grid spacing — cycles through
    /// `crate::canvas::grid::GRID_SIZES_MM`. Applies to the
    /// containing `.snxlib`.
    SymbolCycleGridSize,
    /// Status-footer click on the unit label — cycles
    /// mm → mil → inch → um → mm. Applies to the containing
    /// `.snxlib`.
    SymbolCycleUnit,

    // ── Multi-part component ───────────────────────────────
    /// Toolbar — step the active sub-part down one (Altium "←
    /// Part" arrow). Clamps at `1`. Drives the canvas pin filter +
    /// the active-part badge in the toolbar.
    SymbolPrevPart,
    /// Toolbar — step the active sub-part up one (Altium "Part →"
    /// arrow). Clamps at the symbol's max declared `part_number`
    /// (i.e. doesn't auto-create new parts; that's the Tools ▸
    /// New Part flow's job).
    SymbolNextPart,
    /// Tools ▸ New Part — bumps the symbol's max `part_number` by
    /// one and switches `active_part` to the new value. The new
    /// part starts with no pins; the user adds pins with the
    /// active_part selected.
    SymbolNewPart,
    /// Tools ▸ Remove Part — drops the active part. Pins scoped to
    /// that part get demoted to `part_number = 1` (defensive — keep
    /// the data, lose only the partition); the active part falls
    /// back to `1`. No-op when only one part exists.
    SymbolRemovePart,

    // ── Footprint ──────────────────────────────────────────
    /// v0.18.7 — switch which footprint inside the multi-footprint
    /// `.snxfpt` envelope is being edited. Wraps `active_idx` on the
    /// `FootprintEditorState` wrapper. The dispatcher refreshes the
    /// canvas pad list + camera fit on switch.
    FootprintSelectActiveIdx(usize),
    /// v0.18.7 — append a new empty footprint to the active
    /// `.snxfpt` envelope and switch the editor onto it. Names the
    /// new footprint `Footprint N` where N is the next free index.
    FootprintAddNewSibling,
    /// Click-to-place a pad at the given world position.
    FootprintAddPad { x_mm: f64, y_mm: f64 },
    /// v0.18.12 — Click-to-place a non-plated through hole (NPT) at
    /// the given world position. Mints a `Pad` with `kind = NptHole`,
    /// no copper / mask / paste, drill diameter from
    /// `next_pad_defaults.size_x_mm`. The active bar's "Place Hole"
    /// tool fires this on empty-canvas click.
    FootprintAddHole { x_mm: f64, y_mm: f64 },
    /// v0.18.15 — Click-to-place a silk-layer text label. Appends an
    /// `FpGraphic { kind: Text { ... } }` to `footprint.silk_f`
    /// with placeholder content "TEXT" + 1mm size. The user edits
    /// the content via the Properties panel later.
    FootprintAddText { x_mm: f64, y_mm: f64 },
    /// v0.18.15.1 — click during a Place Track 2-click gesture.
    FootprintTrackClick { x_mm: f64, y_mm: f64 },
    /// v0.18.15.1 — Esc / right-click during Place Track.
    FootprintTrackCancel,
    /// v0.18.15.3 — click during a Place Arc 3-click gesture.
    FootprintArcClick { x_mm: f64, y_mm: f64 },
    /// v0.18.15.3 — Esc / right-click during Place Arc.
    FootprintArcCancel,
    /// v0.18.15.4 — Place Polygon click (appends a vertex).
    FootprintPolygonClick { x_mm: f64, y_mm: f64 },
    /// v0.18.15.4 — explicit polygon commit.
    FootprintPolygonCommit,
    /// v0.18.15.4 — Esc / right-click during Place Polygon.
    FootprintPolygonCancel,
    /// v0.18.18 — silk-front graphic selection.
    FootprintSelectSilkF(Option<usize>),
    /// v0.18.18 — delete the selected silk-front graphic.
    FootprintDeleteSilkF,
    /// v0.18.14 — Selection Filter pill toggle from the unified
    /// active bar. Mirrors the panel-side
    /// `PanelMsg::FpEditorToggleSelectionFilter` but flows through
    /// the per-tab editor dispatch so the active bar can mutate
    /// `editor.state.selection_filter` directly.
    FootprintToggleSelectionFilter(crate::library::editor::footprint::state::SelectionFilterKind),
    /// Drag the pad at `idx` to a new world position.
    FootprintMovePad { idx: usize, x_mm: f64, y_mm: f64 },
    /// Cursor moved over the canvas — drives the footer X/Y readout.
    FootprintCursorAt { x_mm: f64, y_mm: f64 },
    /// Select / deselect a pad. `None` deselects everything.
    FootprintSelectPad(Option<usize>),
    /// Delete-key — remove the currently-selected pad.
    FootprintDeleteSelected,
    /// Toolbar — toggle a layer's visibility. Carries the Standard layer
    /// name string; the dispatcher maps to `FpLayer`.
    FootprintToggleLayer(String),
    /// Toolbar — toggle the auto-fit-courtyard flag.
    FootprintToggleAutoFit,

    // ── v0.13.1 — Sketch mode (Phase 6) ────────────────────
    /// Toolbar — switch the editor mode. `Normal` is the existing
    /// pad-list authoring; `Sketch` opens the parametric sketcher;
    /// `View3d` is the body 3D preview.
    FootprintSetMode(crate::library::editor::footprint::state::EditorMode),
    /// Sketch tool — place a Point at the given world-mm position.
    /// Triggers a solve + bake via the dispatcher.
    FootprintSketchPlacePoint { x_mm: f64, y_mm: f64 },
    /// Sketch inspector — edit / insert a parameter source string.
    /// Triggers a solve + bake.
    FootprintSketchEditParameter { name: String, expr: String },
    /// Sketch inspector — toggle the auto-pause hysteresis state
    /// machine. Used by the inspector's "Live solve paused (resume)"
    /// button.
    FootprintSketchToggleAutoPause,
    /// v0.13.2 — Tool palette: switch the active drawing tool.
    /// Clears any in-flight multi-click gesture (`tool_pending`) so
    /// switching tools mid-gesture doesn't leave dangling anchors.
    FootprintSketchSetTool(crate::library::editor::footprint::state::SketchTool),
    /// v0.16.1 — toggle construction-mode (sticky).
    FootprintSketchToggleConstruction,
    /// v0.16.1 — TAB pause/resume during pad placement.
    FootprintTogglePlacementPause,
    /// v0.16.2 — set the role attr on a sketch entity. Inspector
    /// emits this when the user picks a value from the Role dropdown;
    /// dispatcher routes through
    /// `apply_sketch_role_with_warnings`.
    FootprintSketchSetRole {
        id: signex_sketch::id::SketchEntityId,
        role: RoleTag,
    },
    /// v0.15 — Pads-mode tool switch (Select / PlacePad). Right-
    /// click cancels back to Select via the same dispatch.
    FootprintSetPadsTool(crate::library::editor::footprint::state::PadsTool),
    /// v0.15 — global tool-cancel (Esc). Resets both `pads_tool`
    /// AND `active_tool` (sketch) to Select + clears
    /// `tool_pending`. Mode-agnostic, so the same Esc dispatch
    /// works whichever mode the user is in.
    FootprintToolEscape,
    /// v0.13.2 — Canvas left-click in Sketch mode while a multi-click
    /// drawing tool is active. The dispatcher advances the per-tool
    /// state machine on `tool_pending` and emits the appropriate
    /// `SketchEdit` (AddEntity Line / Circle / Arc) when the gesture
    /// completes. `snap_id` carries the sketch Point under the cursor
    /// (within `SNAP_RADIUS_PX`) for auto-Coincident snap.
    FootprintSketchToolClick {
        x_mm: f64,
        y_mm: f64,
        snap_id: Option<signex_sketch::id::SketchEntityId>,
    },
    /// v0.13.2 — Escape during a multi-click gesture: discard
    /// `tool_pending` without emitting a SketchEdit.
    FootprintSketchToolEscape,

    // ── v0.13.3 — selection / constraint submenu / dimension ──
    /// v0.13.3 — Select a sketch entity. `None` clears the selection;
    /// `Some(id, false)` replaces the primary selection;
    /// `Some(id, true)` adds to the secondary selection slot.
    FootprintSketchSelect {
        id: Option<signex_sketch::id::SketchEntityId>,
        shift: bool,
    },
    /// v0.13.3 — Drag-move a Point entity by `(dx, dy)` in mm. Fires
    /// from the canvas while the user drags a selected Point in
    /// Sketch mode. Emits `SketchEdit::MovePoint`.
    FootprintSketchMovePoint {
        id: signex_sketch::id::SketchEntityId,
        dx: f64,
        dy: f64,
    },
    /// v0.13.3 — Add a constraint based on the current selection.
    /// The inspector's selection-aware submenu emits a `Tag` that
    /// the dispatcher maps into the appropriate `ConstraintKind`
    /// using `selected_sketch` + `selected_sketch_secondary`.
    FootprintSketchAddConstraintForSelection(SketchConstraintTag),
    /// v0.13.3 — Inline numeric input for the Dimension tool /
    /// editable Distance value. Updates `state.dimension_input`.
    FootprintSketchDimensionInput(String),

    // ── Save ───────────────────────────────────────────────
    /// Explicit "Save this primitive tab to disk" — fires from the
    /// editor's Save button. Ctrl+S also routes here for primitive
    /// tabs via the `save_active_document` dispatch path.
    Save,
}

/// Picker modal messages.
#[derive(Debug, Clone)]
pub enum PickerMsg {
    FilterChanged(String),
    SelectComponent(ComponentSummary),
    PlaceSelected,
}

/// Edit Component Details modal sub-message tree — keeps
/// `LibraryMessage` digestible by grouping all the per-field setters
/// under a single sub-enum.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum BrowserEditMsg {
    SetInternalPn(String),
    SetClass(ComponentClass),
    SetState(LifecycleState),
    SetDatasheetUrl(String),
    SetManufacturer(String),
    SetMpn(String),
    /// Live edit of a parameter row's value or unit. The dispatcher
    /// keeps the buffer keyed by `key`; commit happens on
    /// `BrowserEditMsg::CommitParam`.
    SetParamValue {
        key: String,
        value: String,
    },
    SetParamUnit {
        key: String,
        unit: String,
    },
    /// Commit the live param buffer for `key` to the draft.
    CommitParam {
        key: String,
    },
    /// Append a fresh blank parameter row.
    AddParam,
    /// Drop the parameter row at `key`.
    DeleteParam {
        key: String,
    },
    /// Live edit of the comma-separated tags string (Stage 18). Stored
    /// as `parameters["tags"]` on save — a free-form `ParamValue::Text`
    /// preserves the raw user-typed list.
    SetTags(String),
    /// Open the Symbol primitive picker scoped to this edit modal.
    OpenSymbolPicker,
    /// Open the Footprint primitive picker scoped to this edit modal.
    OpenFootprintPicker,
    /// Submit the modal — calls `adapter.update_row` and refreshes the
    /// browser cache. On success the modal closes; on failure the
    /// error surfaces inline.
    Save,
    /// Dismiss the modal without saving.
    Cancel,
}

/// Primitive picker modal sub-messages.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum PrimitivePickerMsg {
    /// Live update of the filter text input.
    SetFilter(String),
    /// Commit a picked `PrimitiveRef` — applies to the picker's
    /// configured target.
    Pick(PrimitiveRef),
    /// User clicked "Browse filesystem…" — fires `AsyncFileDialog`.
    Browse,
    /// Result of the filesystem browse. `Some(path)` when the user
    /// picked a `.snxsym` / `.snxfpt` file; `None` when cancelled.
    BrowseResult(Option<PathBuf>),
    /// Dismiss the picker without picking.
    Cancel,
}

/// Settings → Library → Distributor APIs panel messages.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum SettingsMsg {
    DigiKeyConnect,
    DigiKeyCancel,
    DigiKeyOAuthResult {
        /// Generation tag stamped at the time `DigiKeyConnect` spawned
        /// the worker. The handler ignores results whose generation
        /// no longer matches `digikey_flow_generation` — the user has
        /// since cancelled and started a fresh flow, and applying the
        /// stale outcome would clobber the new flow's state.
        generation: u64,
        connected_label: Option<String>,
        error: Option<String>,
    },
    MouserApiKeyChanged(String),
    MouserTest,
    MouserTestResult(Result<(), String>),
    PreferenceUp(DistributorSource),
    PreferenceDown(DistributorSource),
}
