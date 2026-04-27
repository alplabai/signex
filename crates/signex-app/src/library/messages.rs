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
    // ── Sim tab ─────────────────────────────────────────────
    /// Toggle "Has SPICE model". When `false` the editor clears
    /// `draft.shared.simulation` to `None`. When flipped from `false`
    /// to `true` the editor seeds an empty [`SpiceModel`] and rebuilds
    /// the pin-map skeleton from the symbol's pins.
    SimSetEnabled(bool),
    /// Multi-line SPICE body editor action — applied to the local
    /// `text_editor::Content` and then mirrored back into
    /// `draft.shared.simulation.body`.
    SimBodyAction(iced::widget::text_editor::Action),
    /// Edit a single pin → SPICE node mapping row. `pin_number` is the
    /// Standard pin number (the BTreeMap key).
    SimSetPinNode {
        pin_number: String,
        value: String,
    },
    /// Coarse-grained SPICE model snapshot — used for whole-model
    /// replacement (e.g. paste-from-template flows in Phase 2).
    /// WS-F stub: SimModel rewire lives in WS-E. Variant retained so
    /// the message tree's shape doesn't churn between WSes.
    SimChanged,
    // (Where-Used has no inner messages beyond the row click which
    //  fires `LibraryMessage::JumpToUseSite` directly.)
    // ── Symbol tab ──────────────────────────────────────────
    /// Switch the active symbol-canvas tool.
    SymbolSetTool(SymbolToolMsg),
    /// Place a new pin at the snapped world coordinate.
    SymbolAddPin {
        x: f64,
        y: f64,
    },
    /// Select an existing pin or field on the canvas.
    SymbolSelect(SymbolSelectionMsg),
    /// Drop the current selection (background click).
    SymbolDeselect,
    /// Drag the currently-selected element to a new world coordinate.
    SymbolMoveSelected {
        x: f64,
        y: f64,
    },
    /// Delete-key on the canvas — removes the selected pin (fields
    /// keep their slot but get cleared).
    SymbolDeleteSelected,
    /// Edit Designator / Value text from the side panel.
    SymbolSetField {
        key: FieldKeyMsg,
        value: String,
    },
    /// Edit a pin number from the side-panel pin table.
    SymbolSetPinNumber {
        idx: usize,
        number: String,
    },
    /// Edit a pin name from the side-panel pin table.
    SymbolSetPinName {
        idx: usize,
        name: String,
    },
    /// "AI: From Datasheet PDF" — opens an `rfd` PDF picker.
    SymbolPickAiPdf,
    /// Result of the PDF picker: `None` = cancelled. The path is read
    /// from disk in the dispatcher and run through
    /// `signex_library::ai_stub::extract_pinout`.
    SymbolPickedAiPdf(Option<std::path::PathBuf>),
    /// User clicked "Apply" in the AI preview card.
    SymbolApplyAiPreview,
    /// User clicked "Cancel" in the AI preview card.
    SymbolDismissAiPreview,
    /// WS-F: persist the current Symbol primitive through the adapter.
    /// Carries the new uuid so the dispatcher can round-trip into the
    /// `LibrarySet` entry under `Component.symbol_ref.uuid`.
    SaveSymbol(uuid::Uuid, Symbol),
    // ── Footprint tab ───────────────────────────────────────
    /// Click-add a pad at the given world position (mm). Pad number
    /// is auto-incremented in the dispatcher.
    FootprintAddPad { x_mm: f64, y_mm: f64 },
    /// Drag a pad to a new world position (mm).
    FootprintMovePad { idx: usize, x_mm: f64, y_mm: f64 },
    /// Hover position update — drives the footer X/Y readout.
    FootprintCursorAt { x_mm: f64, y_mm: f64 },
    /// Select / deselect a pad. `None` clears the selection.
    FootprintSelectPad(Option<usize>),
    /// Delete the currently-selected pad (Del key).
    FootprintDeleteSelected,
    /// Toggle a layer's visibility — the string is the Standard layer
    /// name (e.g. "F.Cu"). Unknown names are silently ignored.
    FootprintToggleLayer(String),
    /// Toggle the auto-fit-courtyard flag.
    FootprintToggleAutoFit,
    /// WS-F: persist the current Footprint primitive through the
    /// adapter. Carries the new uuid so the dispatcher can round-trip
    /// into the `LibrarySet` entry under `Component.footprint_ref.uuid`.
    SaveFootprint(uuid::Uuid, Footprint),
    // ── Body 3D editor pane (WS-F, inside Footprint tab) ─────
    /// Set the procedural body height in mm.
    SetBodyHeight(f32),
    /// Set the body's offset above the PCB surface in mm.
    SetBodyOffsetZ(f32),
    /// Set the body's top RGBA color.
    SetBodyTopColor([f32; 4]),
    /// Set the body's side RGBA color.
    SetBodySideColor([f32; 4]),
    /// Switch the procedural body shape (Extrude / Dome / Cylinder / Custom).
    SetBodyShape(BodyShape),
    // ── STEP attachment (WS-F) ───────────────────────────────
    /// Click "Attach STEP…" — runs the file picker.
    StepAttachDialog,
    /// File-picker resolved. `Some(bytes, filename)` succeeded; `None` =
    /// user cancelled. Dispatcher SHA-256s, copies into `step/<hash>.step`,
    /// and updates `Footprint::step_attachment`.
    StepAttachResult(Option<(Vec<u8>, String)>),
    /// Drop the current STEP attachment.
    StepAttachRemove,
}

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

    // ── WS-J: Params tab ──────────────────────────────────────────────
    /// Set a `ParamValue::Text` parameter's value directly. Text inputs
    /// can flush on every keystroke without a parse step.
    ParamSetText { name: String, value: String },
    /// Live-update the per-row edit buffer for a `ParamValue::Number`
    /// row. The buffer lives on `ComponentEditorState.params_edit_buf`;
    /// the value is committed via `ParamCommitNumber`.
    ParamSetNumberBuf { name: String, buf: String },
    /// Commit the live buffer for a `ParamValue::Number` row — runs the
    /// parse step on blur / Enter and writes the parsed `f64` back into
    /// `draft.parameters`. Bad parses leave the buffer dirty so the
    /// user can fix the typo without losing their text.
    ParamCommitNumber { name: String },
    /// Live-update the per-row edit buffer for a `ParamValue::Measurement`
    /// row's value field. The unit comes from the template (or the
    /// existing `Measurement.unit` for custom rows).
    ParamSetMeasurementBuf { name: String, buf: String },
    /// Commit the live buffer for a `ParamValue::Measurement` row.
    /// Carries the unit so the dispatcher can write it without
    /// double-borrowing the editor's parameter map. Bad parses leave
    /// the buffer dirty.
    ParamCommitMeasurement { name: String, unit: String },
    /// Toggle a `ParamValue::Bool` parameter.
    ParamSetBool { name: String, value: bool },
    /// Drop a parameter from `draft.parameters`. Required-template rows
    /// stay visible (re-rendered as "missing"); custom rows disappear
    /// entirely.
    ParamRemove { name: String },
    /// Add a custom parameter row with an empty value of the chosen
    /// kind. The dispatcher chooses defaults: text "", number 0.0,
    /// bool false, measurement value 0.0 with the supplied unit.
    ParamAddCustom { name: String, kind: ParamKindMsg },
    // ── /WS-J ─────────────────────────────────────────────────────────
}

// WS-J: Params tab
/// Pure-data alias for `ParamKind` so messages don't depend on
/// `signex_library::ParamKind` at the message layer. Mirrors
/// [`signex_library::ParamKind`] but carries the unit string for
/// measurements (the registry-side `ParamSlot.unit` is `Option<String>`,
/// but the message variant always knows its unit at construction time).
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
