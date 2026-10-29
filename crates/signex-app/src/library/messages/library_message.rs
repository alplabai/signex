//! `LibraryMessage` — the top-level library message enum, folded into
//! `Message::Library`. Split from `library/messages/mod.rs`.

use std::path::PathBuf;

use signex_library::{PrimitiveKind, RowId, UseSite};

use super::super::state::{EditorAddress, PrimitivePickerTarget};
use super::{
    BrowserEditMsg, CloseLibraryChoice, EditorMsg, PickerMsg, PrimitiveEdit, PrimitivePickerMsg,
    SettingsMsg,
};

/// Top-level library message — folded into [`Message::Library`](crate::app::Message::Library).
#[derive(Debug, Clone)]
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
    AddLibrarySymbolFilePicked {
        project_id: crate::app::ProjectId,
        path: PathBuf,
    },
    /// F34 — Save-As dialog confirmed for a new footprint library
    /// file (`.snxfpt`). Writes an empty `Footprint` to the picked
    /// path, refreshes the cached library file enumeration, and
    /// opens the file as a primitive editor tab.
    AddLibraryFootprintFilePicked {
        project_id: crate::app::ProjectId,
        path: PathBuf,
    },
    /// The off-thread write behind `AddLibrary{Symbol,Footprint}FilePicked`
    /// finished; on success the file is registered on `project_id` (the
    /// project the dialog was opened for) and opened as a primitive tab.
    AddLibraryFileWritten {
        project_id: crate::app::ProjectId,
        path: PathBuf,
        result: Result<(), String>,
    },
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
    /// "Commit now" on the uncommitted-changes banner: retry the git commit
    /// of everything the library saved without one (#858).
    BrowserCommitPending {
        library_path: PathBuf,
    },
    /// The off-thread "Commit now" finished.
    BrowserCommitPendingDone {
        library_path: PathBuf,
        result: Result<signex_library::CommitOutcome, String>,
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
    // ─────────────────────────────────────────────────────────────────
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
    /// Completion of the off-thread read behind `PlaceLibraryComponent`.
    /// `target_sheet` is the sheet that was active when the gesture
    /// started; the handler refuses to arm anywhere else, and `arm_epoch`
    /// (the placement epoch then) lets it ignore a read the user cancelled.
    LibraryComponentResolved {
        target_sheet: PathBuf,
        arm_epoch: u64,
        library_path: PathBuf,
        result: Result<Box<ResolvedPlacement>, String>,
    },
    /// The off-thread designator scan of a project finished; `generation`
    /// is the request it answers, an older one is dropped.
    DesignatorsScanned {
        generation: u64,
        result: std::sync::Arc<
            Result<std::sync::Arc<crate::app::designator_scan::DesignatorScan>, String>,
        >,
    },
    /// Completion of the off-thread read behind a placed symbol's switch
    /// to another drawing of its row (#1230). `target_sheet` is the sheet
    /// that was active when the switch started.
    SymbolChoiceResolved {
        target_sheet: PathBuf,
        /// The engine instance that held the sheet when the switch started;
        /// a re-opened document is a different instance.
        engine_instance: u64,
        symbol_id: uuid::Uuid,
        /// Request number among the switches of `symbol_id`.
        request: u64,
        /// The drawing's name in the row ("IEC"), for messages.
        label: String,
        result: Result<Box<ResolvedSymbolChoice>, String>,
    },
    /// An action on the selected placed part: Open Datasheet, Edit Symbol,
    /// Change Component (#1301, #1262, #1260), and their follow-ups.
    PlacedPart(super::PlacedPartMsg),
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
    /// `SymbolDocument` / `FootprintDocument` in
    /// `DocumentState.symbol_editors` / `footprint_editors`. Mirrors
    /// the `EditorEvent` shape used for Component Preview tabs but
    /// with a path identity instead of `(library_path, table, row_id)`.
    PrimitiveEditorEvent {
        path: PathBuf,
        msg: PrimitiveEdit,
    },
    // ── Library Browser tab ──────────────────────────────────────────
    /// Open the Library Browser tab for `.snxlib` at `library_path`.
    /// Mounts the library if not already mounted, then pushes a
    /// `TabKind::LibraryBrowser(path)` tab (or activates the existing
    /// one when the path is already open).
    OpenLibraryBrowser(PathBuf),
    /// A `.snxlib` mount prepared off the UI thread has finished — issue
    /// #99 part 2c. Produced by
    /// `library::mount::prepare_mount_off_thread`, handled in
    /// `app/dispatch/library/mod.rs`.
    ///
    /// Carries only the path plus a one-shot cell. What happens next —
    /// open a Library Browser tab, or stay silent — is deliberately
    /// *not* in this message: the handler reads the intent from
    /// `LibraryState::pending_mounts`, because a second request for the
    /// same path can have upgraded it while this preparation was in
    /// flight. A cell whose path is no longer pending was cancelled by
    /// `close_library` and is discarded.
    MountFinished {
        path: PathBuf,
        prepared: crate::library::mount::PreparedMountCell,
    },
    /// An off-thread library refresh (#1374) has read the library. Applied
    /// only while `generation` is still the latest for `path`; a result
    /// superseded by a newer refresh, or for a closed library, is dropped.
    RefreshFinished {
        path: PathBuf,
        generation: u64,
        snapshot: crate::library::refresh::RefreshSnapshotCell,
    },
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
    /// A range filter on a parameter column of this Library Browser tab.
    BrowserRangeFilter {
        library_path: PathBuf,
        msg: super::RangeFilterMsg,
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
    /// The browser grid's scrollable moved. Carries the scroll offset the
    /// grid windows its rows from.
    BrowserGridScrolled {
        library_path: PathBuf,
        offset_y: f32,
    },
    /// The browser grid was laid out at a new height (a resize, including
    /// one the scrollable does not report because the rows now fit).
    BrowserGridResized {
        library_path: PathBuf,
        viewport_h: f32,
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
    /// "Edit Details" on the selected browser row → opens the Edit
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
        filter: super::super::state::LifecycleFilter,
    },
    /// Toggle the per-class filter. `Some(key)` sets it; passing
    /// the currently-active key flips back to `None`.
    BrowserClassFilterClicked {
        library_path: PathBuf,
        key: String,
    },
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
    RecoveryLibraryMissing(super::super::recovery::LibraryMissingChoice),
    RecoveryLibraryMissingLocateResult(Option<PathBuf>),
    RecoveryGitMissing(super::super::recovery::GitMissingChoice),
    RecoveryBrokenBinding(super::super::recovery::BrokenBindingChoice),

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
    /// `skipped_updates_for` (not rendered anywhere yet).
    LibraryUpdatesSkipAll,
    /// User dismissed the modal (Cancel / Esc / X). No state mutates;
    /// drift is left pinned and the schematic stays clean.
    LibraryUpdatesCancel,

    // ── Components Panel (Stage 9 of v0.9-snxlib-as-file) ───────────
    /// Toggle the collapse flag for the named section
    /// ("project" / "installed" / "global"). The dispatcher looks
    /// the section up by name so adding a fourth section later
    /// doesn't ripple through the message enum.
    ComponentsPanelToggleSection(super::super::state::ComponentsMountSource),
    /// Live edit of the Components Panel filter input. Substring
    /// matched across mpn / manufacturer / internal_pn / library
    /// name. Stage 9 ships this; the rich syntax (plan §5) is
    /// follow-up work.
    ComponentsPanelSetFilter(String),
    /// "+ Add Library…" button on the Installed / Global section
    /// header. Opens an `rfd::AsyncFileDialog` with `*.snxlib`
    /// filter and lands in `ComponentsPanelAddLibraryAt`.
    ComponentsPanelAddLibrary(super::super::state::ComponentsMountSource),
    /// Result of the Add Library file dialog. `None` = user
    /// cancelled. Carries the source the picker was opened against
    /// so the dispatcher knows whether to push onto
    /// `installed_libraries` or `global_libraries`.
    ComponentsPanelAddLibraryAt {
        source: super::super::state::ComponentsMountSource,
        path: Option<PathBuf>,
    },
    /// Promote an Installed library to Global — moves the path from
    /// the session-scoped Vec to the persisted TOML file. No-op when
    /// the path isn't currently Installed.
    ComponentsPanelPromoteToGlobal(PathBuf),
    /// "Place into Schematic" button on a Components Panel row.
    /// Wired to the row's "Place" button (#1160); places on the active
    /// schematic like the Place Component picker.
    ComponentsPanelPlace {
        library_path: PathBuf,
        table: String,
        row_id: RowId,
    },
    /// The external-edit poll timer fired (#531). The handler starts the
    /// off-thread `stat` of every mounted `.snxlib`.
    ExternalEditsPollTicked,
    /// The `stat` pass finished, or timed out (#827); see
    /// [`crate::library::external_poll::PollOutcome`].
    ExternalEditsPolled(crate::library::external_poll::PollOutcome),
    /// The off-thread drift pass finished (#912): which embedded library
    /// symbols differ from the mounted libraries' current ones.
    DriftChecked(Box<crate::library::drift::DriftReport>),
    /// The off-thread library read behind the Library Updates modal's Apply
    /// finished; the update itself is applied from this result.
    LibraryUpdateSymbolsRead(Box<crate::library::drift::SymbolsRead>),
    /// The background Refresh All Pricing walk finished. `Err` is a failure
    /// of the walk as a whole (library unreadable, write refused); per-row
    /// failures are inside the report.
    PricingRefreshFinished {
        library_path: PathBuf,
        outcome: std::sync::Arc<Result<signex_library::PricingRefreshReport, String>>,
    },
}

/// What the off-thread read of a library row hands back to `update`:
/// everything needed to register the `LibSymbol` and place the part,
/// with no further IO.
#[derive(Debug, Clone)]
pub struct ResolvedPlacement {
    /// `"<library uuid>:<symbol uuid>"` — `LibSymbol.id` and `Symbol.lib_id`.
    pub lib_id: String,
    pub lib_symbol: signex_types::schematic::LibSymbol,
    pub library_id: uuid::Uuid,
    pub row_id: uuid::Uuid,
    pub library_version: String,
    pub internal_pn: String,
    pub value: String,
    /// The symbol is a power port: the placed instance is `is_power` and
    /// `value` is its net name.
    pub is_power: bool,
    /// Reference prefix from the library symbol's designator (`U?` -> `U`):
    /// the legacy prefix, used when the owning project follows no designator
    /// standard or does not cover `class`.
    pub prefix: String,
    /// The row's component class key (`"diode"`, `"generic"`), for the
    /// project's designator standard (#1232).
    pub class: String,
    /// The row's raw `designator_prefix` parameter, not yet validated: it
    /// beats the standard's table entry for `class` (#1315).
    pub prefix_override: Option<String>,
    /// The part's primary datasheet link and identity fields (Manufacturer,
    /// MPN and the row's parameters). A placement does not stamp them;
    /// Change Component replaces the old part's with them. The footprint is
    /// not among them: the row owns it through `row_id`, and a placed
    /// symbol's footprint text stays empty as on a fresh placement.
    pub datasheet: String,
    pub fields: Vec<(String, String)>,
}

/// The drawing a placed symbol is switched to, read off-thread: the
/// converted symbol and its `"<library uuid>:<symbol uuid>"` key.
#[derive(Debug, Clone)]
pub struct ResolvedSymbolChoice {
    pub lib_id: String,
    pub lib_symbol: signex_types::schematic::LibSymbol,
}
