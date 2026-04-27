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
    LifecycleState, RowId, SimKind, SimModel, UseSite,
};

use super::state::{EditorAddress, PreviewTab};

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
    /// dirty editor addresses the user is about to lose.
    // WS-I: tab-not-window — keyed by `EditorAddress` because the
    // editors live as tabs, not as OS windows.
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
    // ── WS-E: New Component flow ─────────────────────────────────────
    /// File ▸ Library ▸ New Component… — opens the New Component modal.
    NewComponent,
    // WS-H: Project tree library wiring
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
    /// Live-edit of the modal's "Category" field.
    NewComponentSetCategory(String),
    /// User picked a table in the New Component modal.
    NewComponentSetTable(String),
    /// Submit the New Component modal — creates the draft, persists,
    /// opens the editor on the new component.
    NewComponentSubmit,
    // ─────────────────────────────────────────────────────────────────
    /// Toggle the Library left-dock panel's library tree node at
    /// `path` (path relative to the open libraries list).
    ToggleLibraryTreeNode(usize),
    /// v0.9-refactor-2 (DBLib model): open a Component Preview tab for
    /// the row identified by `(library_path, table, row_id)`. Replaces
    /// the previous `OpenEditor { library_path, component_id }` shape.
    OpenComponentRow {
        library_path: PathBuf,
        table: String,
        row_id: RowId,
    },
    /// Open a standalone primitive editor tab for the file at `path`.
    /// Fired by the Component Preview tab's right-click context menu on
    /// the Symbol / Footprint render panes; routed to WS-7's standalone
    /// `.snxsym` / `.snxfpt` editor.
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
    /// Internal trace-only signal: a Component Preview tab was opened
    /// for the given address. WS-5 fires this from
    /// `OpenComponentRow` so WS-6 has a single message to subscribe
    /// to once the row-shaped editor lands.
    ComponentPreviewOpened {
        path: PathBuf,
        table: String,
        row_id: RowId,
    },
    /// Inner-message envelope for events fired from a standalone
    /// primitive editor tab (WS-7). Keyed by file path so the
    /// dispatcher can locate the matching `SymbolEditorState` /
    /// `FootprintEditorState` in `DocumentState.symbol_editors` /
    /// `footprint_editors`. Mirrors the `EditorEvent` shape used
    /// for Component Preview tabs but with a path identity instead
    /// of `(library_path, table, row_id)`.
    PrimitiveEditorEvent {
        path: PathBuf,
        msg: PrimitiveEditorMsg,
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

/// Component Preview inner messages. v0.9-refactor-2 (DBLib model):
/// the surface is preview-only for Symbol/Footprint, so the legacy
/// Symbol/Footprint canvas messages stay defined for backwards
/// compatibility (the standalone `.snxsym`/`.snxfpt` editor in WS-7
/// re-uses them) but they no longer dispatch through the Component
/// Preview tab.
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

    // ── WS-G: Pin Map (Preview-tab inline subsection) ───────
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
    // ── /WS-G ───────────────────────────────────────────────

    // ── WS-F2: Symbol tab ─────────────────────────────────────
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

    // ── WS-F2: Footprint tab ──────────────────────────────────
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

    // ── WS-K: Supply tab ──────────────────────────────────────
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
    // ── /WS-K ─────────────────────────────────────────────────

    // ── WS-J: Params tab ──────────────────────────────────────
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
    // ── /WS-J ─────────────────────────────────────────────────

    // ── WS-L: Sim tab ─────────────────────────────────────────
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
    // ── /WS-L ─────────────────────────────────────────────────
}

// WS-J: Params tab
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
}

/// Selection target on the Symbol canvas — pure-data version of
/// `editor::symbol::state::SymbolSelection`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum SymbolSelectionMsg {
    Pin(usize),
    FieldReference,
    FieldValue,
}

/// Symbol field key — pure-data alias of
/// `editor::symbol::state::FieldKey`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum FieldKeyMsg {
    Reference,
    Value,
}

// WS-7 (refactor-2): standalone primitive editor tabs
/// Inner messages for a [`LibraryMessage::PrimitiveEditorEvent`]
/// envelope. Path-keyed dispatch routes each variant to the symbol or
/// footprint editor state stored on `DocumentState` per the active
/// tab's [`crate::app::TabKind`]. Save (Ctrl+S) flows through the
/// existing schematic-save handler — these variants only cover the
/// editor-side mutations + the explicit "save primitive" command.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum PrimitiveEditorMsg {
    // ── Symbol ─────────────────────────────────────────────
    /// Set the active drawing tool on the Symbol canvas.
    SymbolSetTool(SymbolToolMsg),
    /// Click-to-place a pin on the standalone Symbol canvas at the
    /// given grid-snapped (mm) world position.
    SymbolAddPin { x: f64, y: f64 },
    /// Select a symbol element (pin index / field key).
    SymbolSelect(SymbolSelectionMsg),
    /// Click landed on empty canvas — drop the current selection.
    SymbolDeselect,
    /// Drag the currently-selected element to a new grid-snapped
    /// world position.
    SymbolMoveSelected { x: f64, y: f64 },
    /// Delete-key — drop the currently-selected element.
    SymbolDeleteSelected,
    /// Properties pane — overwrite the pin number string at index.
    SymbolSetPinNumber { idx: usize, number: String },
    /// Properties pane — overwrite the pin name string at index.
    SymbolSetPinName { idx: usize, name: String },

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
