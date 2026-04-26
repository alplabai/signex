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
    ComponentId, ComponentSummary, DistributorSource, LifecycleState, SpiceModel, UseSite, Version,
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
    /// If any editor against this library has unsaved drafts, the
    /// dispatcher diverts to [`LibraryMessage::ConfirmCloseLibrary`]
    /// instead and shows the dirty-editors prompt.
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
    /// File ▸ Library ▸ New Component… — opens the New Component modal.
    NewComponent,
    /// Dismiss the New Component modal without creating anything.
    CloseNewComponent,
    /// Live-edit of the New Component modal's "Internal PN" field.
    NewComponentSetInternalPn(String),
    /// User picked a target library in the modal — index into
    /// `LibraryState.open_libraries`.
    NewComponentSetLibrary(usize),
    /// Submit the New Component modal — creates the draft, persists,
    /// opens the editor on the new component.
    NewComponentSubmit,
    /// Toggle the Library left-dock panel's library tree node at
    /// `path` (path relative to the open libraries list).
    ToggleLibraryTreeNode(usize),
    /// Open the Component Editor in a new OS window for `id` inside
    /// the library at `library_path`. The handler runs
    /// `iced::window::open` and stores the resulting id once
    /// [`LibraryMessage::EditorWindowOpened`] resolves.
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
    /// gets the message. Mirrors the pattern used by
    /// `Message::Tab { window_id, msg }` for tab undocking.
    EditorEvent {
        window_id: iced::window::Id,
        msg: EditorMsg,
    },
    /// Picker modal interaction — search text, list selection, etc.
    Picker(PickerMsg),
    /// Settings ▸ Library ▸ Distributor APIs panel updates.
    Settings(SettingsMsg),
    /// Click a Where-Used row in the editor → jump to the project /
    /// sheet / instance. Stubbed in Phase 1 — emits
    /// `tracing::info!` and is intercepted by Phase 2 to actually
    /// open the project + select the instance.
    JumpToUseSite(UseSite),
    /// No-op sink — used by the diff preview canvases in the History
    /// tab. The canvases are read-only previews; their `()` event
    /// stream maps through this variant so they slot into the
    /// editor's `LibraryMessage` Element tree without dragging the
    /// canvas crate's generic state into the editor view.
    Noop,
    /// Picker → user clicked Place. Embeds the library component into
    /// the active schematic engine. The dispatcher resolves the
    /// `(library_id, content_hash)` source-ref and emits the engine
    /// command (Phase 3 wires the actual `Command::PlaceLibrarySymbol`
    /// once the schematic side gains the embed/source-ref slots).
    PlaceLibraryComponent {
        library_path: PathBuf,
        component_id: ComponentId,
        version: Version,
    },
}

/// User choice from the close-library confirmation modal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseLibraryChoice {
    /// Save every dirty draft, then close the library.
    SaveAll,
    /// Drop every dirty draft, then close the library.
    DiscardAll,
    /// Dismiss the modal; the library stays open.
    Cancel,
}

/// Component Editor inner messages. Carried inside
/// [`LibraryMessage::EditorEvent`] so the dispatcher can route by
/// `window_id` first, then by editor-tab logic.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum EditorMsg {
    /// User clicked a tab pill (Overview, Symbol, …).
    SelectTab(EditorTab),
    /// Save the current draft locally without committing.
    SaveDraft,
    /// Auto-bump the version, prompt for changelog, commit.
    Commit,
    /// Open the review-request UI (only when
    /// `manifest.workflow.review_required`).
    SubmitForReview,
    /// Reviewer-notes text input changed in the SubmitForReview modal.
    SubmitForReviewNotesChanged(String),
    /// User clicked Submit in the SubmitForReview modal.
    SubmitForReviewConfirm,
    /// User clicked Cancel / X / Esc in the SubmitForReview modal.
    SubmitForReviewCancel,
    /// Async result of the SubmitForReview save_revision call. `Ok(())`
    /// closes the modal; `Err(reason)` keeps it open with `reason`
    /// surfaced in the footer status line.
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
    /// Plain URL — Phase 1 only stores
    /// `DatasheetRef::Url`. Phase 2 adds the hash-pinned upload flow.
    OverviewSetDatasheet(String),
    OverviewSetLifecycle(LifecycleState),
    // ── Datasheet picker (URL ↔ Pinned PDF) ─────────────────
    /// User flipped the URL/Pinned toggle on the Overview tab.
    DatasheetSetMode(crate::library::editor::datasheet_picker::DatasheetMode),
    /// URL text-input changed (active when mode == URL). Convenience
    /// alias for `OverviewSetDatasheet` so the picker widget can stay
    /// decoupled from the older field name.
    DatasheetSetUrl(String),
    /// Click "Upload PDF…" — runs `rfd::AsyncFileDialog`.
    DatasheetUploadDialog,
    /// File-picker resolved. `Some(bytes, filename)` = succeeded;
    /// `None` = user cancelled.
    DatasheetUploadResult(Option<(Vec<u8>, String)>),
    // ── 3D model upload + transform ─────────────────────────
    /// Click "Upload STEP/WRL/GLB…" — runs `rfd::AsyncFileDialog`.
    Model3dUploadDialog,
    /// File-picker resolved. `Some(bytes, filename)` = succeeded;
    /// `None` = user cancelled.
    Model3dUploadResult(Option<(Vec<u8>, String)>),
    /// Click "Remove" on the 3D model card.
    Model3dRemove,
    /// Edit one axis of the 3D model offset (`offset[axis] = value`).
    Model3dSetOffset { axis: usize, value: f64 },
    /// Edit one axis of the 3D model rotation.
    Model3dSetRotation { axis: usize, value: f64 },
    // ── Params tab ──────────────────────────────────────────
    ParamSetKey {
        idx: usize,
        key: String,
    },
    ParamSetValueText {
        idx: usize,
        value: String,
    },
    ParamAddRow,
    ParamRemoveRow(usize),
    // ── Supply tab ──────────────────────────────────────────
    SupplyAddRow,
    SupplyRemoveRow(usize),
    SupplySetDistributor {
        idx: usize,
        value: String,
    },
    SupplySetSku {
        idx: usize,
        value: String,
    },
    SupplySetUrl {
        idx: usize,
        value: String,
    },
    /// Top-of-tab "paste a distributor URL" field.
    SupplyPasteUrlChanged(String),
    /// User clicked "Refresh from API" — stubbed in Phase 1.
    SupplyRefreshFromApi,
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
    /// replacement (e.g. paste-from-template flows in Phase 2). The
    /// canonical save path documented in LIBRARY_PLAN §10.
    SimChanged(SpiceModel),
    // (Where-Used has no inner messages beyond the row click which
    //  fires `LibraryMessage::JumpToUseSite` directly.)
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
    /// Replace the entire footprint sexpr — used when external code
    /// (e.g. paste, AI-stub) wants to atomically swap in a new
    /// footprint. Re-parses into the editor state.
    FootprintEdited(String),
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
}

/// Picker modal messages.
#[derive(Debug, Clone)]
pub enum PickerMsg {
    /// Filter text changed (live narrowing).
    FilterChanged(String),
    /// User selected one of the result rows.
    SelectComponent(ComponentSummary),
    /// User clicked "Place" on the currently-selected row. Phase 1
    /// only logs; Phase 2 wires this into the schematic engine.
    PlaceSelected,
}

/// Settings → Library → Distributor APIs panel messages.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum SettingsMsg {
    /// "Connect" button on the DigiKey row — kicks off the OAuth2
    /// PKCE flow on a worker thread.
    DigiKeyConnect,
    /// User clicked Cancel while the DigiKey OAuth flow was in flight.
    /// Cancels the listener thread + clears the "Waiting for browser…"
    /// status.
    DigiKeyCancel,
    /// Async resolution of the DigiKey OAuth flow. Carries a tuple of
    /// (account_label, error_reason) where exactly one is `Some`.
    /// `Cancelled` is represented as `(None, None)`.
    DigiKeyOAuthResult {
        connected_label: Option<String>,
        error: Option<String>,
    },
    /// Mouser API-key input changed.
    MouserApiKeyChanged(String),
    /// Mouser "Test" button — kicks off a real Mouser request on a
    /// worker thread.
    MouserTest,
    /// Async result of the Mouser test path. `Ok(())` triggers a
    /// keyring writeback + ✓ status; `Err(reason)` shows the failure.
    MouserTestResult(Result<(), String>),
    /// Move a distributor up in the order-of-preference list.
    PreferenceUp(DistributorSource),
    /// Move a distributor down in the order-of-preference list.
    PreferenceDown(DistributorSource),
}
