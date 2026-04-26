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
    BodyShape, ComponentId, ComponentSummary, DistributorSource, Footprint, LifecycleState, Symbol,
    UseSite, Version,
};

use super::state::EditorTab;

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
    /// dirty editor windows the user is about to lose.
    ConfirmCloseLibrary {
        library_path: PathBuf,
        dirty_editors: Vec<iced::window::Id>,
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
    /// Open the Component Editor in a new OS window for `id` inside
    /// the library at `library_path`.
    OpenEditor {
        library_path: PathBuf,
        component_id: ComponentId,
    },
    /// `iced::window::open` resolved for a Component Editor window.
    EditorWindowOpened {
        library_path: PathBuf,
        component_id: ComponentId,
        window_id: iced::window::Id,
    },
    /// Inner editor message — `window_id` selects which open editor
    /// gets the message.
    EditorEvent {
        window_id: iced::window::Id,
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

    // TODO(WS-F): Symbol / Footprint / 3D / Sim editor messages —
    //  add when those tabs land. WS-E only ships the
    //  shape needed for the modal + Overview round-trip.
    // TODO(WS-G): Pin Map auto-match / override messages.
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
