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
    AlternateStatus, BodyShape, ComponentClass, ComponentId, ComponentSummary, DistributorSource,
    LifecycleState, UseSite, Version,
};

use super::state::{EditorAddress, EditorTab};

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
    /// Submit the New Component modal — creates the draft, persists,
    /// opens the editor on the new component.
    NewComponentSubmit,
    // ─────────────────────────────────────────────────────────────────
    /// Toggle the Library left-dock panel's library tree node at
    /// `path` (path relative to the open libraries list).
    ToggleLibraryTreeNode(usize),
    // WS-I: tab-not-window
    /// Open the Component Editor for `(library_path, component_id)`
    /// as a tab in the main window's tab bar. Detach into a separate
    /// window remains available via the existing tab-undock flow.
    OpenEditor {
        library_path: PathBuf,
        component_id: ComponentId,
    },
    // WS-I: tab-not-window
    /// Inner editor message — keyed by `(library_path, component_id)`
    /// so the same EditorEvent dispatches to the editor regardless of
    /// whether it's hosted inline as a tab or in an undocked window.
    /// The legacy `EditorWindowOpened` daemon-window setup is gone —
    /// editors are tabs first.
    EditorEvent {
        library_path: PathBuf,
        component_id: ComponentId,
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
    /// Picker → user clicked Place. Embeds the library component into
    /// the active schematic engine.
    PlaceLibraryComponent {
        library_path: PathBuf,
        component_id: ComponentId,
        version: Version,
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

/// Component Editor inner messages. WS-E carries the **shape** so the
/// dispatcher and view tree compile; WS-F (Symbol/Footprint) and WS-G
/// (Pin Map) replace the per-tab handlers. The variants tagged
/// `TODO(WS-F)` / `TODO(WS-G)` are placeholders that will be reworked.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum EditorMsg {
    /// User clicked a tab pill (Overview, Symbol, …).
    SelectTab(EditorTab),
    /// Save the current draft locally without committing.
    SaveDraft,
    /// Auto-bump the version, prompt for changelog, commit.
    Commit,
    /// Open the review-request UI.
    SubmitForReview,
    SubmitForReviewNotesChanged(String),
    SubmitForReviewConfirm,
    SubmitForReviewCancel,
    SubmitForReviewResult(Result<(), String>),
    /// Footer "Where Used" — switches the active editor tab.
    OpenWhereUsedTab,
    /// User dismissed the editor (Close X or Ctrl+W).
    CloseEditor,

    // ── Overview tab ─────────────────────────────────────────
    OverviewSetDisplayName(String),
    OverviewSetInternalPn(String),
    OverviewSetMpn(String),
    OverviewSetManufacturer(String),
    OverviewSetDescription(String),
    OverviewSetDatasheet(String),
    OverviewSetLifecycle(LifecycleState),

    // ── History tab ─────────────────────────────────────────
    HistorySelectRevision(Version),
    // ── WS-G: Pin Map ───────────────────────────────────────
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
    PinMapOverrideBufChanged { pin: String, value: String },
    /// User clicked "Save" inside the inline editor — push a
    /// `PinPadOverride` onto the active draft.
    PinMapAddOverride { pin: String, pad: String },
    /// User clicked "Cancel" inside the inline editor — discard the
    /// edit buffer + collapse the row.
    PinMapCancelOverrideEdit,
    /// User clicked "Remove" on an overridden row — drops that pin's
    /// entry from `Revision::pin_map_overrides`.
    PinMapRemoveOverride { pin: String },
    // ── /WS-G ───────────────────────────────────────────────

    // ── WS-F2: Symbol tab ─────────────────────────────────────
    /// Set the active drawing tool on the Symbol canvas.
    SymbolSetTool(SymbolToolMsg),
    /// Click-to-place a pin on the symbol canvas at the given grid-
    /// snapped (mm) world position.
    SymbolAddPin { x: f64, y: f64 },
    /// Select a symbol element (pin index / field key) — emitted by
    /// the canvas hit-test on left-click.
    SymbolSelect(SymbolSelectionMsg),
    /// Click landed on empty canvas — drop the current selection.
    SymbolDeselect,
    /// Drag the currently-selected element to a new grid-snapped
    /// world position. Field drag is a no-op for now.
    SymbolMoveSelected { x: f64, y: f64 },
    /// Delete-key — drop the currently-selected element.
    SymbolDeleteSelected,
    /// Properties pane — set the value text of one of the canonical
    /// symbol fields (Designator / Value).
    SymbolSetField { key: FieldKeyMsg, value: String },
    /// Properties pane — overwrite the pin number string at index.
    SymbolSetPinNumber { idx: usize, number: String },
    /// Properties pane — overwrite the pin name string at index.
    SymbolSetPinName { idx: usize, name: String },
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
    SupplyAlternateSetManufacturer { idx: usize, value: String },
    /// Edit the MPN of the alternate at `idx`.
    SupplyAlternateSetMpn { idx: usize, value: String },
    /// Pick the approval status of the alternate at `idx`.
    SupplyAlternateSetStatus { idx: usize, value: AlternateStatus },
    /// Edit the free-form notes of the alternate at `idx`.
    SupplyAlternateSetNotes { idx: usize, value: String },
    /// Drop the alternate row at `idx`.
    SupplyAlternateRemove { idx: usize },

    // Distributor listings
    /// Append a fresh blank distributor listing row.
    SupplyListingAdd,
    /// Pick the distributor source for the listing at `idx`. The
    /// dispatcher converts `DistributorSource` to the canonical string
    /// stored on `DistributorListing.distributor`.
    SupplyListingSetDistributor { idx: usize, value: DistributorSource },
    /// Edit the SKU of the distributor listing at `idx`.
    SupplyListingSetSku { idx: usize, value: String },
    /// Edit the URL of the distributor listing at `idx`. Empty string
    /// clears the field back to `None`.
    SupplyListingSetUrl { idx: usize, value: String },
    /// Drop the distributor listing row at `idx`.
    SupplyListingRemove { idx: usize },
    // ── /WS-K ─────────────────────────────────────────────────

    // ── WS-J: Params tab ──────────────────────────────────────
    /// Set a `ParamValue::Text` parameter's value directly. Text inputs
    /// can flush on every keystroke without a parse step.
    ParamSetText { name: String, value: String },
    /// Live-update the per-row edit buffer for a `ParamValue::Number`
    /// row. The buffer lives on `ComponentEditorState.params_edit_buf`;
    /// the value is committed via `ParamCommitNumber`.
    ParamSetNumberBuf { name: String, buf: String },
    /// Commit the live buffer for a `ParamValue::Number` row.
    ParamCommitNumber { name: String },
    /// Live-update the per-row edit buffer for a `ParamValue::Measurement`
    /// row's value field.
    ParamSetMeasurementBuf { name: String, buf: String },
    /// Commit the live buffer for a `ParamValue::Measurement` row.
    ParamCommitMeasurement { name: String, unit: String },
    /// Toggle a `ParamValue::Bool` parameter.
    ParamSetBool { name: String, value: bool },
    /// Drop a parameter from `draft.parameters`.
    ParamRemove { name: String },
    /// Add a custom parameter row with an empty value of the chosen kind.
    ParamAddCustom { name: String, kind: ParamKindMsg },
    // ── /WS-J ─────────────────────────────────────────────────
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
