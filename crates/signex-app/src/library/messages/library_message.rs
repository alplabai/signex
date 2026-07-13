//! `LibraryMessage` — the top-level library message enum, folded into
//! `Message::Library`. Split from `library/messages/mod.rs`.

use std::path::PathBuf;

use signex_library::{ComponentClass, PrimitiveKind, RowId, UseSite};

use super::super::state::{EditorAddress, PrimitivePickerTarget};
use super::{
    BrowserEditMsg, CloseLibraryChoice, EditorMsg, PickerMsg, PrimitiveEdit, PrimitivePickerMsg,
    SettingsMsg,
};

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
        msg: PrimitiveEdit,
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
        filter: super::super::state::LifecycleFilter,
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
