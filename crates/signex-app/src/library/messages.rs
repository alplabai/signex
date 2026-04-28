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

use super::state::{EditorAddress, PreviewTab, PrimitivePickerTarget};

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
    /// dispatcher prompts for a name (default `<project>-lib`),
    /// creates `<root>/<name>.snxlib` via [`crate::library::commands::create_library`],
    /// then registers it in `Project::libraries` so the project
    /// tree picks it up on the next `refresh_panel_ctx`.
    CreateLibraryAt(std::path::PathBuf),
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
    /// Live-edit of the modal's "Category" field.
    NewComponentSetCategory(String),
    /// Submit the New Component modal — mints fresh Symbol + Footprint
    /// primitives, builds a [`signex_library::ComponentRow`] binding
    /// them by `PrimitiveRef`, and inserts it into the chosen table.
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
    OpenPrimitiveEditor { path: PathBuf },
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
    BrowserDeleteRowCancel { library_path: PathBuf },
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
    /// Select / deselect a pad. `None` deselects everything.
    FootprintSelectPad(Option<usize>),
    /// Delete-key — remove the currently-selected pad.
    FootprintDeleteSelected,
    /// Toolbar — toggle a layer's visibility. Carries the Standard layer
    /// name string; the dispatcher maps to `FpLayer`.
    FootprintToggleLayer(String),
    /// Toolbar — toggle the auto-fit-courtyard flag.
    FootprintToggleAutoFit,
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
    /// Stamp a 2 mm-radius circle centred on `(x, y)`.
    SymbolAddCircle { x: f64, y: f64 },
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
    /// matching Altium's per-document Sheet Color.
    SymbolSetSheetColor(crate::panels::SheetColor),

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
    /// Click-to-place a pad at the given world position.
    FootprintAddPad { x_mm: f64, y_mm: f64 },
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
        connected_label: Option<String>,
        error: Option<String>,
    },
    MouserApiKeyChanged(String),
    MouserTest,
    MouserTestResult(Result<(), String>),
    PreferenceUp(DistributorSource),
    PreferenceDown(DistributorSource),
}
