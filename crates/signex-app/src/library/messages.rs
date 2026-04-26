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
    ComponentId, ComponentSummary, DistributorSource, LifecycleState, UseSite, Version,
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
    /// File ▸ Library ▸ Place Component… — opens the picker modal.
    OpenPicker,
    /// Dismiss the picker modal (Esc / X / outside click).
    ClosePicker,
    /// File ▸ Library ▸ New Component… — opens a brand-new editor
    /// window seeded with a draft revision.
    NewComponent,
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
    // (Where-Used has no inner messages beyond the row click which
    //  fires `LibraryMessage::JumpToUseSite` directly.)
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
    /// "Connect" button on the DigiKey row — kicks off OAuth (stub).
    DigiKeyConnect,
    /// Mouser API-key input changed.
    MouserApiKeyChanged(String),
    /// Mouser "Test" button.
    MouserTest,
    /// Move a distributor up in the order-of-preference list.
    PreferenceUp(DistributorSource),
    /// Move a distributor down in the order-of-preference list.
    PreferenceDown(DistributorSource),
}
