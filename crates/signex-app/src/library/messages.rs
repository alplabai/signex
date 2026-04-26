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
    ComponentClass, ComponentId, ComponentSummary, DistributorSource, LifecycleState, UseSite,
    Version,
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
