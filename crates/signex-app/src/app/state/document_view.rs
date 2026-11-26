//! The GUI side of the open documents: dock, panels, dialogs, drafts and
//! per-window views. Moved out of `DocumentState`, the proto-core, in ADR-0006
//! slice 4a (#2326).

use crate::dock::DockArea;

use super::{
    BomPreviewState, ErrorNotice, LiveDrawingEdit, NetlistIncompletePrompt, PageSetupMemory,
    PreviewState, ProjectId, RasterQueue, TabPlacements, navigator_job, net_nav,
    window_hierarchies,
};

pub struct DocumentViewState {
    pub dock: DockArea,
    /// Change detection + debounce for persisting `dock` (#692). The
    /// write itself runs as a `Task`; see `handlers/dock/layout_persist.rs`.
    pub dock_layout_sync: crate::dock::DockLayoutSync,
    /// The Components filter + Library Browser queries and the debounce
    /// that persists them (#859); see `handlers/dock/search_prefs_persist.rs`.
    pub search_prefs: crate::persist::SearchPrefsSync,
    /// Off-thread Navigator hierarchy assembly (#1470).
    pub navigator_job: navigator_job::NavigatorJob,
    /// Hierarchies of the other projects undocked windows show (#2021).
    pub window_hierarchies: window_hierarchies::WindowHierarchies,
    /// Which font families the Messages panel has already said an open
    /// document names but this machine lacks (#510), so each is reported
    /// once per document.
    pub missing_fonts: crate::fonts::document_report::MissingFontReporter,
    /// v0.26-E — process-local clipboard for footprint-editor pad
    /// Cut / Copy / Paste. Survives tab switches so a pad copied from
    /// one footprint can be pasted into another. `None` until the
    /// first Cut / Copy. Replaced wholesale on every copy. Keyed by
    /// no path on purpose — Altium parity is single-slot, last-write-
    /// wins.
    pub pad_clipboard: Option<crate::library::editor::footprint::state::EditorPad>,
    /// Project tab state per loaded project (#1202): selected section and
    /// the General history. Outlives the tab, dropped with the project.
    pub project_tabs: std::collections::HashMap<ProjectId, crate::app::ProjectTabState>,
    /// The one store of half-typed title-block and paper-edge text (#2027);
    /// every panel context holds a handle to it.
    pub drafts: crate::panels::PanelDrafts,
    /// The drawing field being typed, if any (#2768).
    pub(crate) live_drawing_edit: Option<LiveDrawingEdit>,
    /// The selection the Line Width box's text was typed under (#2767).
    pub(crate) line_width_draft_selection: Option<uuid::Uuid>,
    pub panel_ctx: crate::panels::PanelContext,
    /// The panel context of each undocked document window, so its dock
    /// panels show and edit that window's sheet (#1966).
    pub window_panel_ctx: std::collections::HashMap<iced::window::Id, crate::panels::PanelContext>,
    /// What each window context was last built from; see `window_ctx_key`.
    pub(crate) window_panel_ctx_keys:
        std::collections::HashMap<iced::window::Id, crate::app::runtime::WindowCtxKey>,
    /// Print-preview overlay state. `Some` while the preview dialog is
    /// open. Doubles as the unified PDF Export modal — `File → Export
    /// PDF` and `File → Print Preview` both populate this field.
    pub preview: Option<PreviewState>,
    /// Page setup (size, orientation, margins, scale) each document last used
    /// in the PDF export modal, restored when it reopens for the same
    /// document. Session-only; never written to disk.
    pub page_setup_memory: PageSetupMemory,
    /// The sheet symbol each child tab was descended through (#1133).
    pub tab_placements: TabPlacements,
    /// Sheet history, net highlight, hover insight and the project net index
    /// they read (#1270, #1271, #1272).
    pub net_nav: net_nav::NetNavState,
    /// Generation counter and in-flight job of the preview renderer.
    pub raster_queue: RasterQueue,
    /// The app's one user-visible error card. `Some` while it is shown;
    /// cleared by `OverlayMsg::DismissErrorNotice`.
    ///
    /// Not export-only, despite the heading it used to hardcode: the
    /// project-close path already raised it for "could not save N
    /// file(s)", so a failed close announced itself to the user as
    /// "Export Failed" (#532). The title now travels with the message.
    pub error_notice: Option<ErrorNotice>,
    /// #431 — pending "Export anyway (incomplete)?" prompt. `Some` while the
    /// netlist-incomplete modal is up, waiting on the user's choice. Set by
    /// `handle_export_netlist_finished` when the derived netlist has a hole;
    /// cleared by both prompt actions (Export anyway / Cancel).
    pub netlist_incomplete_prompt: Option<NetlistIncompletePrompt>,
    /// BOM preview state. `Some` while the BOM Export modal is open.
    /// Mirrors the Print Preview pattern: the user adjusts grouping /
    /// include flags / format / variant in the modal and clicks
    /// Export to drive the file picker with the chosen options.
    pub bom_preview: Option<BomPreviewState>,
    /// Right-dock History panel state — current generation counter,
    /// last loaded entries, the path the load was issued for. Driven
    /// by `Message::HistoryLoaded` and re-targeted on tab switch.
    pub history: crate::panels::history::HistoryPanelState,
    /// The History panel state of each undocked document window, swapped in
    /// with its panel context so the window shows and restores its own file
    /// (#2067).
    pub(crate) window_history:
        std::collections::HashMap<iced::window::Id, crate::panels::history::HistoryPanelState>,
    /// The Gerber Viewer tab (mockup rows 193 and 193f): the package shown
    /// and the app-wide recent packages. Its slice owns it.
    pub gerber_viewer: crate::app::gerber_viewer::GerberViewerState,
}
