use std::path::PathBuf;

use signex_types::schematic::SchematicSheet;
use signex_types::theme::ThemeId;

use crate::canvas::CanvasEvent;
use crate::dock::DockMessage;
use crate::menu_bar::MenuMessage;
use crate::tab_bar::TabMessage;
use crate::toolbar::ToolMessage;

use super::selection_request::SelectionRequest;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragTarget {
    LeftPanel,
    RightPanel,
    BottomPanel,
    ComponentsSplit,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum Message {
    Menu(MenuMessage),
    Tool(ToolMessage),
    /// Tab-bar message carrying the id of the window whose tab bar emitted
    /// it. Lets the handler distinguish main-window tab reorder/select
    /// (which mutates `document_state.active_tab`) from an undocked
    /// window's tab bar, which only has one visible tab and must not
    /// clobber the main window's active index.
    Tab {
        window_id: iced::window::Id,
        msg: TabMessage,
    },
    Dock(DockMessage),
    /// UI / chrome message family (ADR-0001 D3). Theme, unit + grid
    /// toggles, the grid picker, layout drag, window resize, and the
    /// status-bar request enum. Routed to `dispatch_ui_message`.
    Ui(UiMsg),
    CanvasEvent(CanvasEvent),
    /// Canvas event stamped with the window that produced it. The
    /// dispatch layer swaps the window's `SchematicCanvas` into the
    /// main canvas slot for the duration of the handler so the
    /// hundreds of `active_canvas_mut()` call sites read and write the
    /// right canvas transparently. Keyboard-generated canvas events
    /// (FitAll shortcut, etc.) continue to use the unwrapped
    /// `Message::CanvasEvent` variant and always target the main
    /// window.
    CanvasEventInWindow {
        window_id: iced::window::Id,
        event: CanvasEvent,
    },
    /// Grid Properties dialog — namespaced (ADR-0001 D3).
    GridProperties(GridPropertiesMsg),
    /// Custom Selection Filter modal message family — namespaced
    /// (ADR-0001 D3). Routed to `dispatch_selection_filter_message`.
    SelectionFilter(SelectionFilterMsg),
    /// File / save message family (ADR-0001 D3). Namespaced under
    /// `Message::File` and routed to `dispatch_file_message`.
    File(FileMsg),
    /// Edit-command message family (ADR-0001 D3). Namespaced under
    /// `Message::Edit` and routed to `dispatch_edit_message`.
    Edit(EditMsg),
    Selection(SelectionRequest),
    /// Overlay / modal-chrome message family (ADR-0001 D3). Panel-list
    /// toggle, panel open, find/replace open, keyboard-shortcuts and
    /// first-run-tour dismiss, modal drag, canvas focus-at, auto-focus
    /// toggle. Routed to `dispatch_overlay_message`.
    Overlay(OverlayMsg),
    ActiveBar(crate::active_bar::ActiveBarMsg),
    /// In-canvas text-edit message family (ADR-0001 D3). Namespaced
    /// under `Message::TextEdit` and routed to
    /// `dispatch_text_edit_message`.
    TextEdit(TextEditMsg),
    /// Context-menu subsystem message family — namespaced (ADR-0001
    /// D3). Canvas / project-tree / tab right-click menus plus the
    /// submenu hover state machine. Routed to
    /// `dispatch_context_menu_message`.
    ContextMenu(ContextMenuMsg),
    /// Project lifecycle message family (ADR-0001 D3). Namespaced
    /// under `Message::Project` and routed to `dispatch_project_message`.
    Project(ProjectMsg),
    /// Rename modal message family — namespaced (ADR-0001 D3). Routed
    /// to `dispatch_rename_message`.
    Rename(RenameMsg),
    /// Remove-from-project modal message family — namespaced (ADR-0001
    /// D3). Routed to `dispatch_remove_message`.
    Remove(RemoveMsg),
    /// Enable Version Control modal message family — namespaced
    /// (ADR-0001 D3). Routed to `dispatch_enable_version_control_message`.
    EnableVersionControl(EnableVersionControlMsg),
    /// Preferences modal message family — namespaced (ADR-0001 D3).
    /// Routed to `dispatch_preferences_message`.
    Preferences(PreferencesMsg),
    FindReplaceMsg(crate::find_replace::FindReplaceMsg),
    /// Window lifecycle / docking / native-chrome message
    /// family (ADR-0001 D3). Routed to `dispatch_window_message`.
    Window(WindowMsg),
    /// ERC dialog message family — namespaced (ADR-0001 D3). Routed to
    /// `dispatch_erc_message`.
    Erc(ErcMsg),
    /// Annotate dialog message family — namespaced (ADR-0001 D3). Routed
    /// to `dispatch_annotate_message`.
    Annotate(AnnotateMsg),
    /// Move Selection dialog family (ADR-0001 D3). Altium numeric
    /// ΔX / ΔY move. Routed to `dispatch_move_selection_message`.
    MoveSelection(MoveSelectionMsg),
    /// Per-net colour overrides + F5 palette open/close — namespaced
    /// (ADR-0001 D3). Routed to `dispatch_net_color_message`.
    NetColor(NetColorMsg),
    /// Parameter Manager dialog family (ADR-0001 D3). Bulk parameter
    /// editor. Routed to `dispatch_parameter_manager_message`.
    ParameterManager(ParameterManagerMsg),
    /// Click on a pin-connection matrix cell: cycle its severity
    /// Error → Warning → Info → Off → (back to baseline default).
    PinMatrixCellCycled {
        row: u8,
        col: u8,
    },
    /// Cycle Altium's rubber-band selection mode
    /// Inside → Outside → TouchingLine → Inside. Bound to Shift+S.
    CycleSelectionMode,
    /// Close the in-flight lasso polygon (Enter key). Commits the
    /// selection if >= 3 vertices, otherwise cancels.
    LassoCommit,
    /// Apply an edit to a placed SchDrawing. Dispatched from the
    /// post-placement Properties panel (Line / Rect / Circle / Arc /
    /// Polygon editable rows). Engine replaces the stored drawing by
    /// uuid with full undo.
    UpdateDrawingField(uuid::Uuid, DrawingFieldEdit),
    /// Export subsystem message family — namespaced (ADR-0001 D3).
    /// PDF / netlist / BOM export lifecycle plus the export-error
    /// modal dismiss. Routed to `dispatch_export_message`.
    Export(ExportMsg),
    /// BOM-preview modal — namespaced family (ADR-0001 D3). Routed to
    /// `dispatch_bom_preview_message`.
    BomPreview(BomPreviewMsg),
    /// Print-preview modal — namespaced family (ADR-0001 D3). Routed to
    /// `dispatch_print_preview_message`.
    PrintPreview(PrintPreviewMsg),
    /// v0.9 Library subsystem message — folded under one variant so
    /// the dispatcher can route to `library_dispatch::handle` in one
    /// shot. See `crate::library::LibraryMessage` for the inner
    /// shape.
    Library(crate::library::LibraryMessage),
    /// Command-palette message family (ADR-0001 D3). Namespaced under
    /// `Message::CommandPalette` and routed to
    /// `dispatch_command_palette_message`.
    CommandPalette(CommandPaletteMsg),
    /// Async result of a per-file Git history load issued by the
    /// right-dock History panel. `generation` is the value of
    /// `DocumentState.history.generation` at the time the load was
    /// kicked off; the handler discards stale results whose
    /// generation no longer matches the current counter (the user
    /// has switched tabs since). `path` is the file the load
    /// targeted, surfaced for diagnostic logging only — the
    /// generation token is the authoritative staleness check.
    HistoryLoaded {
        generation: u32,
        path: std::path::PathBuf,
        result: Result<Vec<signex_widgets::HistoryEntry>, String>,
    },
    /// v0.14.2 — keyboard shortcut for footprint editor mode switch.
    /// Routed from the global `1` / `2` / `3` key handler in
    /// `bootstrap.rs::subscription`. The dispatcher checks whether
    /// the active tab is a footprint editor; if yes, sets the mode;
    /// otherwise no-op so the keys don't steal text input on other
    /// tabs.
    FootprintModeShortcut(crate::library::editor::footprint::state::EditorMode),
    /// v0.15 — Esc-key tool cancel routed through the dispatcher.
    /// If the active tab is a footprint editor, fires
    /// `FootprintToolEscape` (resets PadsTool + SketchTool +
    /// tool_pending); otherwise falls back to the schematic
    /// `Tool::Select` reset.
    EscapePressed,
    Noop,
}

/// Window lifecycle, docking, and native-chrome message family
/// (ADR-0001 D3). Namespaced under `Message::Window`, routed to
/// `dispatch_window_message`.
#[derive(Debug, Clone)]
pub enum WindowMsg {
    /// Resize event carrying the window id. Forwarded by the
    /// `iced::window::resize_events()` subscription so the dispatcher
    /// can drop non-main-window resizes before they touch
    /// `ui_state.window_size`.
    WindowResizedFor(iced::window::Id, f32, f32),
    /// Fired once `iced::window::open` completes for the initial main
    /// window — lets us stash the id so `view(id)` knows which window is
    /// the primary app shell versus a detached modal / undocked tab.
    MainWindowOpened(iced::window::Id),
    /// OS-reported scale factor for the main window. Fired on window
    /// open and on every main-window resize (winit emits a resize event
    /// when Windows moves the window across monitors with different
    /// scale factors). Stored in `ui_state.main_window_scale` and used
    /// by the menu bar to pick the crispest wordmark PNG tier.
    MainWindowScaleChanged(f32),
    /// Fired when any secondary (non-main) window closes. Cleans up the
    /// corresponding entry in `ui_state.windows` so the app can re-attach
    /// the modal or tab to the main window's overlay stack.
    SecondaryWindowClosed(iced::window::Id),
    /// OS-level close request (native close button, Alt+F4, taskbar
    /// close) for a window. In `iced::daemon` mode windows do NOT
    /// auto-close on this event, so the app decides what to do: the
    /// main window routes through the unsaved-changes guard
    /// (`CloseMainWindow`); every other window closes directly.
    WindowCloseRequested(iced::window::Id),
    /// Pop a modal out of the main window into its own OS window. Altium
    /// triggers this when the user drags the modal's title bar past the
    /// main window edge, or clicks the pop-out icon in the title bar.
    DetachModal(super::state::ModalId),
    /// Fired after `window::open` resolves for a detached modal — stores
    /// the new window's id in `ui_state.windows` so `view(id)` can render
    /// it and `SecondaryWindowClosed` can reattach when the user dismisses
    /// the window.
    DetachedModalOpened {
        modal: super::state::ModalId,
        id: iced::window::Id,
    },
    /// Pop a document tab into its own OS window (Altium-style tab
    /// undocking). Fires from the tab bar's ↗ button or when a tab drag
    /// crosses the main window edge.
    UndockTab(usize),
    /// `iced::window::open` resolved for an undocked tab — records the
    /// window id so the tab bar hides the tab while its window lives.
    UndockedTabOpened {
        path: std::path::PathBuf,
        id: iced::window::Id,
    },
    /// Reattach an undocked tab to the main window's tab bar. Closing
    /// the secondary window triggers this implicitly via
    /// `SecondaryWindowClosed`; the in-window "Reattach" button emits it
    /// directly.
    ReattachTab(iced::window::Id),
    /// Convert a floating in-app panel into its own OS window. Fires
    /// when the floating panel's drag crosses the main window edge.
    DetachFloatingPanel(usize),
    /// `iced::window::open` resolved for a detached panel — records its
    /// id + panel kind so `view(id)` can render the panel's content.
    DetachedPanelOpened {
        kind: crate::panels::PanelKind,
        id: iced::window::Id,
    },
    /// User pressed on the borderless modal's header — start an OS-level
    /// window drag for the window hosting this modal. Lets the user move
    /// the detached modal even though `decorations: false` removed the
    /// native title bar.
    StartDetachedWindowDrag(super::state::ModalId),
    /// User pressed on empty chrome (menu-bar row outside buttons) — start
    /// an OS-level window drag for the main borderless window. The chrome
    /// is the replacement for the OS title bar.
    StartMainWindowDrag,
    /// User pressed one of the 6 px edge strips around the borderless
    /// main window — ask the OS to start a resize drag in that
    /// direction. Replaces the WS_THICKFRAME edges we lose when
    /// decorations are disabled.
    StartMainWindowResize(iced::window::Direction),
    /// User pressed one of the 6 px edge strips around a borderless
    /// detached modal window — ask the OS to start a resize drag in
    /// that direction. Same trick as the main window; without this
    /// the modals couldn't be resized because `decorations: false`
    /// strips the OS chrome.
    StartDetachedModalResize {
        modal: super::state::ModalId,
        direction: iced::window::Direction,
    },
    /// Custom min/max/close buttons in the borderless main-window chrome.
    MinimizeMainWindow,
    ToggleMaximizeMainWindow,
    CloseMainWindow,
}

/// Command-palette message family (ADR-0001 D3). Namespaced under
/// `Message::CommandPalette` and routed to
/// `dispatch_command_palette_message`.
#[derive(Debug, Clone)]
pub enum CommandPaletteMsg {
    /// Open the command palette dropdown and focus the chrome-strip
    /// search bar. Bound to Ctrl+Shift+P. Idempotent — already-open
    /// keeps state, just refocuses the input.
    Open,
    /// Close the dropdown without executing. Bound to Esc and to
    /// click-outside. Leaves the chrome-strip input visible (it's the
    /// always-on placeholder) but unfocused; query is preserved so a
    /// re-open continues where the user left off.
    Close,
    /// Live query update from the chrome-strip text_input. Resets the
    /// selected row to 0 because the result list reorders on every
    /// keystroke.
    QueryChanged(String),
    /// Move the highlighted row by `delta` (clamped to result count).
    /// Wired to ArrowUp / ArrowDown when the palette is open.
    MoveSelection(i32),
    /// Click on a specific row in the dropdown — sets selected_index
    /// and executes in one shot.
    Select(usize),
    /// Execute the currently selected entry. Wired to Enter and to
    /// `text_input::on_submit`.
    ExecuteSelected,
}

/// In-canvas text-edit message family (ADR-0001 D3). Namespaced under
/// `Message::TextEdit` and routed to `dispatch_text_edit_message`.
#[derive(Debug, Clone)]
pub enum TextEditMsg {
    /// Live text update from the in-place editor overlay.
    Changed(String),
    /// Commit the in-place edit back to the engine (Enter / blur).
    Submit,
}

/// Overlay / modal-chrome message family (ADR-0001 D3). Namespaced
/// under `Message::Overlay` and routed to `dispatch_overlay_message`.
#[derive(Debug, Clone)]
pub enum OverlayMsg {
    /// Toggle the "＋ panel" dropdown in the right dock header.
    TogglePanelList,
    /// Open (dock) a panel of the given kind in the right column.
    OpenPanel(crate::panels::PanelKind),
    /// Open the Find modal (replace row hidden).
    OpenFind,
    /// Open the Find/Replace modal with the replace row visible.
    OpenReplace,
    /// Close the Help ▸ Keyboard Shortcuts modal — fired by the close
    /// chrome ✕ and by Esc dismiss handling.
    CloseKeyboardShortcuts,
    /// Dismiss the first-run tour card and persist the flag so it
    /// never reappears. Fired by the card's ✕ button, by Esc, and by
    /// the first canvas interaction after launch.
    DismissFirstRunTour,
    /// User pressed the title bar of a modal at window-space (x, y) —
    /// begin dragging it. The next DragMove events update its offset.
    ModalDragStart {
        modal: super::state::ModalId,
        x: f32,
        y: f32,
    },
    /// Modal drag released (mouse-up). Clears `modal_dragging`.
    ModalDragEnd,
    /// Navigate to a world-space point on the canvas; optionally
    /// replace the current selection with the given item. Used for
    /// click-to-zoom in the Messages panel.
    FocusAt {
        world_x: f64,
        world_y: f64,
        select: Option<signex_types::schematic::SelectedItem>,
    },
    /// Toggle AutoFocus — dim everything not in the current selection.
    ToggleAutoFocus,
}

/// UI / chrome message family (ADR-0001 D3). Namespaced under
/// `Message::Ui` and routed to `dispatch_ui_message`. Groups theme,
/// unit + grid toggles, the footprint grid picker, layout drag, main-
/// window resize, and the status-bar request enum. Canvas events keep
/// their own top-level `Message::CanvasEvent(…)` variants.
#[derive(Debug, Clone)]
pub enum UiMsg {
    /// Switch the active colour theme and persist the preference.
    #[allow(dead_code)]
    ThemeChanged(ThemeId),
    /// Cycle the display unit (mm / mil / inch).
    UnitCycled,
    /// Toggle grid visibility across every canvas.
    GridToggle,
    /// Clear the active canvas background cache (grid density change).
    GridCycle,
    /// v0.18.10 — open the Altium-style grid picker popup at the
    /// current `last_mouse_pos`. Footprint editor only (today);
    /// other contexts fall through to no-op.
    GridPickerOpen,
    /// v0.18.10 — dismiss the grid picker without picking.
    GridPickerClose,
    /// v0.18.10 — user picked a grid step from the picker. The
    /// payload is the step in mm; the dispatcher writes it to the
    /// active footprint editor's `state.snap_options.grid_step_mm`.
    GridPickerSelect(f64),
    /// Layout drag started on a dock splitter / floating element.
    DragStart(DragTarget),
    /// Layout drag moved to window-space (x, y).
    DragMove(f32, f32),
    /// Layout drag released.
    DragEnd,
    /// Main-window content resized (width, height) in logical px.
    WindowResized(f32, f32),
    /// Status-bar request (unit / grid / snap toggles, panel list,
    /// properties). Kept as its own sub-enum in `StatusBarRequest`.
    StatusBar(StatusBarRequest),
}

/// Move Selection dialog message family (ADR-0001 D3). Namespaced under
/// `Message::MoveSelection` and routed to `dispatch_move_selection_message`.
#[derive(Debug, Clone)]
pub enum MoveSelectionMsg {
    /// Open the Move Selection dialog (Altium numeric ΔX / ΔY move).
    Open,
    /// Close the dialog without applying.
    Close,
    /// ΔX text-field edit.
    DxChanged(String),
    /// ΔY text-field edit.
    DyChanged(String),
    /// Apply the current ΔX / ΔY to every selected item. Closes the
    /// dialog on success.
    Apply,
}

/// BOM-preview modal message family (ADR-0001 D3). Namespaced under
/// `Message::BomPreview` and routed to `dispatch_bom_preview_message`.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum BomPreviewMsg {
    /// User changed BOM grouping (Grouped / Ungrouped / Flat).
    SetGrouping(signex_output::BomGrouping),
    /// User changed BOM output format (CSV / XLSX / HTML).
    SetFormat(signex_output::BomFormat),
    /// User toggled "Include DNP" in the BOM preview modal.
    SetIncludeDnp(bool),
    /// User toggled "Include Not Fitted" in the BOM preview modal.
    SetIncludeNotFitted(bool),
    /// User toggled a single column on / off in the BOM preview
    /// column picker. The handler flips the column's presence in
    /// `BomOptions.columns`, preserving the existing display order
    /// when re-adding so the user's column ordering survives toggles.
    ToggleColumn(signex_output::BomColumn),
    /// User picked a variant in the BOM preview variant dropdown.
    /// `None` means the "Base" (no-variant) view.
    SetVariant(Option<String>),
    /// User clicked a column header — set or cycle the sort spec.
    /// First click on a column sorts ascending; same column again
    /// flips to descending; a third click clears the sort and goes
    /// back to rollup order.
    SortColumn(usize),
    /// User started dragging a column header. Carries the source
    /// index in `options.columns`.
    ColumnDragStart(usize),
    /// User dropped a dragged column header onto another header.
    /// The source column moves to the destination index, preserving
    /// the user's column order intent.
    ColumnDragDrop(usize),
    /// Cursor entered a column header — used by the in-progress
    /// drag-reorder feedback to highlight the drop target.
    ColumnHoverEnter(usize),
    /// Cursor left a column header. Clears the hover state for that
    /// idx; the next on_enter on a sibling header replaces it.
    ColumnHoverExit(usize),
    /// User pressed a column's right-edge resize handle. Stores
    /// the start x and start width on `BomPreviewState`; subsequent
    /// mouse-move events compute the new width as
    /// `start_width + (current_x - start_x)`.
    ColumnResizeStart(usize),
    /// User released the mouse — clears the in-flight resize state.
    ColumnResizeEnd,
    /// User clicked a Properties-sidebar tab (General / Columns) in
    /// the BOM preview modal.
    SetSidebarTab(super::state::BomSidebarTab),
    /// User clicked Export in the BOM preview modal — drives the file
    /// dialog with the live options.
    Export,
    /// User dismissed the BOM preview modal.
    Close,
}

/// Print-preview modal message family (ADR-0001 D3). Namespaced under
/// `Message::PrintPreview` and routed to `dispatch_print_preview_message`.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum PrintPreviewMsg {
    /// User triggered print preview via Ctrl+P or menu. Open preview dialog.
    Requested,
    /// User selected a page in the print preview thumbnail list.
    SelectPage(usize),
    /// User changed preview colour mode.
    SetColourMode(signex_output::ColourMode),
    /// User changed preview page range to all sheets.
    SetPageRangeAll,
    /// User changed preview page range to current sheet.
    SetPageRangeCurrent,
    /// User changed preview page range to one specific page.
    SetPageRangeSpecific,
    /// User edited the specific page input in preview.
    SetSpecificPageInput(String),
    /// User toggled "Fit to Page" in the unified PDF preview modal.
    SetFitToPage(bool),
    /// User toggled "Include Title Block" in the unified PDF preview modal.
    SetIncludeTitleBlock(bool),
    /// Mouse wheel scrolled over the preview image. Carries the
    /// vertical delta — positive = scroll up = zoom in. Multiplies
    /// `PreviewState.zoom` by `ZOOM_STEP` per notch.
    Zoom(f32),
    /// User clicked the "Export PDF" button in the preview dialog.
    Export,
    /// User closed the print preview dialog.
    Close,
    /// User clicked the Preview / Settings tab inside the unified
    /// Export PDF modal.
    SetTab(super::state::PdfPreviewTab),
    /// User pressed mouse-down on the preview viewport — kicks off
    /// pan-drag. The handler reads the cursor from
    /// `interaction_state.last_mouse_pos` rather than carrying it on
    /// the message; iced builds messages eagerly at view-render time
    /// so embedded coords would be one frame stale.
    PanStart,
    /// User released the pan drag — clears `panning`.
    PanFinished,
    /// User toggled a project file in the Settings → Files list.
    ToggleFile(std::path::PathBuf),
    /// Select all project files in the Settings → Files list.
    SelectAllFiles,
    /// Deselect all project files (effectively "no override —
    /// fall back to all").
    ClearAllFiles,
    /// Variant picker dropdown — None = Base.
    SetVariant(Option<String>),
    SetUsePhysicalStructure(bool),
    SetPhysicalDesignators(bool),
    SetPhysicalNetLabels(bool),
    SetPhysicalPorts(bool),
    SetPhysicalSheetNumber(bool),
    SetPhysicalDocumentNumber(bool),
    SetIncludeNoErcMarkers(bool),
    SetIncludeParameterSets(bool),
    SetIncludeProbes(bool),
    SetIncludeBlankets(bool),
    SetIncludeNotes(bool),
    SetIncludeCollapsedNotes(bool),
    SetQuality(super::state::PdfQuality),
    SetBookmarkZoom(f32),
    SetGenerateNetsInfo(bool),
    SetBookmarkPins(bool),
    SetBookmarkNetLabels(bool),
    SetBookmarkPorts(bool),
    SetIncludeComponentParameters(bool),
    SetGlobalBookmarks(bool),
    SetPcbColourMode(signex_output::ColourMode),
}

/// Grid Properties dialog message family (ADR-0001 D3). Namespaced
/// under `Message::GridProperties` and routed to
/// `dispatch_grid_properties_message`.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum GridPropertiesMsg {
    /// v0.18.11 — open the Cartesian Grid Editor modal (Ctrl+G).
    /// Footprint editor only; other contexts no-op.
    Open,
    /// v0.18.11 — close the Grid Properties modal (Cancel button or
    /// Esc). Discards in-flight edits.
    Close,
    /// v0.18.11 — Grid Properties modal: text input bound to the
    /// Step X field. Strings are validated on Apply, not per
    /// keystroke, so partial input doesn't fight the user.
    SetStepX(String),
    /// v0.18.11 — Grid Properties modal: text input bound to the
    /// Step Y field.
    SetStepY(String),
    /// v0.18.11 — Grid Properties modal: toggle the X/Y link.
    /// When linked, editing Step X mirrors into Step Y.
    ToggleLink,
    /// v0.18.11 — Grid Properties modal: Apply button. Validates +
    /// writes the active footprint editor's `snap_options.grid_step_mm`
    /// (and Y if/when separate axes ship). v0.18.19: also commits
    /// fine/coarse display + multiplier.
    Apply,
    /// v0.18.19 — Grid Properties modal: Fine grid display style.
    SetFineDisplay(crate::library::editor::footprint::state::GridDisplay),
    /// v0.18.19 — Grid Properties modal: Coarse grid display style.
    SetCoarseDisplay(crate::library::editor::footprint::state::GridDisplay),
    /// v0.18.19 — Grid Properties modal: Multiplier (5x / 10x / 2x / 1x).
    SetMultiplier(u32),
}

/// Per-net colour override message family (ADR-0001 D3). Namespaced
/// under `Message::NetColor` and routed to
/// `dispatch_net_color_message`.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum NetColorMsg {
    /// Open the F5 Net Color palette (detached modal).
    Open,
    /// Close the Net Color palette.
    Close,
    /// Assign a color to a net label text, or clear the override.
    Set {
        net: String,
        color: Option<signex_types::theme::Color>,
    },
    /// Show / hide the custom net-color picker modal.
    CustomShow(bool),
    /// Live-update the draft colour as the user drags the picker.
    CustomDraft(iced::Color),
    /// Commit the draft colour and arm net-colour flood mode.
    CustomSubmit(iced::Color),
    /// Edit one R/G/B channel of the custom-picker draft via text
    /// input. Parsed as 0-255; invalid values ignored.
    CustomChannel(Channel, String),
}

/// Parameter Manager dialog message family (ADR-0001 D3). Namespaced
/// under `Message::ParameterManager` and routed to
/// `dispatch_parameter_manager_message`.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum ParameterManagerMsg {
    /// Open the Parameter Manager dialog (bulk parameter editor).
    Open,
    /// Close the dialog.
    Close,
    /// Edit a single parameter on a single symbol via the manager.
    Edit {
        symbol_uuid: uuid::Uuid,
        key: String,
        value: String,
    },
}

/// Annotate dialog message family (ADR-0001 D3). Namespaced under
/// `Message::Annotate` and routed to `dispatch_annotate_message`.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum AnnotateMsg {
    /// Auto-annotate every unannotated symbol (reference ends in `?`).
    /// Three modes: incremental, reset+renumber, reset-only.
    Run(signex_engine::AnnotateMode),
    /// Show the Annotate Schematics modal with preview of proposed changes.
    OpenDialog,
    /// Dismiss the Annotate dialog without applying.
    CloseDialog,
    /// Change the Annotate dialog's order-of-processing choice.
    OrderChanged(super::state::AnnotateOrder),
    /// Show the Reset-Annotations confirm modal.
    OpenResetConfirm,
    /// Dismiss the Reset-Annotations confirm modal.
    CloseResetConfirm,
    /// Toggle the "locked against reannotation" flag on a symbol from
    /// inside the Annotate dialog. Locked symbols keep their current
    /// designator even under Reset & Renumber.
    ToggleLock(uuid::Uuid),
}

/// ERC dialog message family (ADR-0001 D3). Namespaced under
/// `Message::Erc` and routed to `dispatch_erc_message`.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum ErcMsg {
    /// Run the ERC engine against the active schematic snapshot and populate
    /// `ui_state.erc_violations`. Bound to F8.
    Run,
    /// Show the ERC modal (severity matrix + pin-compatibility matrix).
    OpenDialog,
    /// Dismiss the ERC dialog.
    CloseDialog,
    /// Override the severity for a single rule from within the ERC dialog.
    SeverityChanged(signex_erc::RuleKind, signex_erc::Severity),
}

/// Preferences modal message family (ADR-0001 D3). Namespaced under
/// `Message::Preferences` and routed to `dispatch_preferences_message`.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum PreferencesMsg {
    /// Open the Preferences modal.
    Open,
    /// Close the Preferences modal.
    Close,
    /// Navigate to a Preferences pane.
    Nav(crate::preferences::PrefNav),
    /// Forward an inner Preferences message to the pane handler.
    Inner(crate::preferences::PrefMsg),
}

/// Enable Version Control modal message family (ADR-0001 D3).
/// Namespaced under `Message::EnableVersionControl` and routed to
/// `dispatch_enable_version_control_message`.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum EnableVersionControlMsg {
    /// Toggle the LFS checkbox on the Enable Version Control modal.
    ToggleLfs,
    /// Toggle the per-item "Track" checkbox on the Enable Version
    /// Control modal. Index is into `EnableVersionControlState::items`.
    /// Untracked items are written into a generated `.gitignore` at
    /// confirm time so they sit outside the initial commit.
    ToggleItem(usize),
    /// Confirm — runs `git init` + initial commit at the project
    /// dir, refreshes the panel ctx so any in-tree dirty markers
    /// reflect the new state.
    Confirm,
    /// Dismiss the Enable Version Control modal without writing.
    Close,
}

/// Rename modal message family (ADR-0001 D3). Namespaced under
/// `Message::Rename` and routed to `dispatch_rename_message`.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum RenameMsg {
    /// Text input in the rename modal — updates the live buffer.
    BufferChanged(String),
    /// Commit the rename: fs::rename + update in-memory sheet / tab
    /// state. Errors surface in `RenameDialogState::error`.
    Submit,
    /// Dismiss the rename modal without applying.
    Close,
}

/// Remove-from-project modal message family (ADR-0001 D3). Namespaced
/// under `Message::Remove` and routed to `dispatch_remove_message`.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum RemoveMsg {
    /// User picked Delete / Exclude in the Remove modal.
    Confirm(RemoveChoice),
    /// Dismiss the Remove modal without applying.
    Close,
}

/// Context-menu subsystem message family (ADR-0001 D3). Namespaced
/// under `Message::ContextMenu` and routed to
/// `dispatch_context_menu_message`. Covers the canvas right-click
/// menu, the Projects-panel tree right-click menu, the document-tab
/// right-click menu, and the shared submenu hover/open state machine.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum ContextMenuMsg {
    Show(f32, f32),
    Close,
    Action(ContextAction),
    /// Right-click landed on a specific tree node — open the per-node
    /// context menu at `last_mouse_pos`. `path = None` → background menu.
    ShowProjectTree(Option<Vec<usize>>),
    /// Dismiss the Projects-panel tree context menu.
    CloseProjectTree,
    /// Menu item picked — route the action.
    ProjectTreeAction(ProjectTreeAction),
    /// Right-click on a document tab — open the per-tab context menu
    /// at `last_mouse_pos`. Carries the clicked tab's index.
    ShowTab(usize),
    /// Dismiss the document-tab right-click menu.
    CloseTab,
    /// Menu item picked — route the action.
    TabAction(TabContextAction),
    /// Expand a click-to-open submenu inside the right-click context
    /// menu (Place or Align). Toggles off when the same kind is fired
    /// twice, otherwise replaces the current submenu.
    SubmenuOpen(ContextSubmenu),
    /// Hover entered a submenu launcher row — start the 200 ms
    /// hover-open timer for that submenu (and cancel any pending
    /// close).
    SubmenuHover(ContextSubmenu),
    /// Hover left the submenu launcher row — cancels any pending
    /// open and starts the 150 ms close timer if a submenu is open.
    SubmenuLeave,
    /// Hover entered the open submenu panel — cancels the close timer
    /// so the panel stays visible while the cursor traverses it.
    SubmenuEnterPanel,
    /// Hover left the open submenu panel — starts the close timer.
    SubmenuLeavePanel,
    /// 50 ms tick fired by the subscription while the context menu is
    /// open; promotes a mature `pending_submenu` into an actual open
    /// and a mature `pending_submenu_close` into an actual close.
    SubmenuTickHover,
}

/// Export subsystem message family (ADR-0001 D3). Namespaced under
/// `Message::Export` and routed to `dispatch_export_message`. Covers
/// the PDF / netlist / BOM export lifecycle plus the export-error
/// modal dismiss.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum ExportMsg {
    /// Open the unified PDF Export overlay (File → Export → PDF…). Now
    /// delegates to `handle_print_preview_requested`, which sets up
    /// `document_state.preview` with the rasterized pages plus every
    /// PDF setting in one modal.
    PdfOpenDialog,
    /// Completion of PDF export — carries either the saved path or error.
    PdfFinished(Result<std::path::PathBuf, String>),
    /// Completion of netlist export — carries either the saved path or error.
    NetlistFinished(Result<std::path::PathBuf, String>),
    /// User invoked File → Export → Bill of Materials… — open the
    /// BOM preview modal instead of going straight to the file
    /// dialog. Mirrors Print Preview.
    BomRequested,
    /// Completion of BOM export — carries either the saved path or error.
    BomFinished(Result<std::path::PathBuf, String>),
    /// User clicked the OK button on the export-error modal.
    DismissError,
}

/// Custom Selection Filter modal message family (ADR-0001 D3).
/// Namespaced under `Message::SelectionFilter` and routed to
/// `dispatch_selection_filter_message`. Drives the footprint editor's
/// selection-filter customization modal.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum SelectionFilterMsg {
    /// v0.18.14.1 — Custom Selection Filter modal launcher. Opens
    /// the 8-row checkbox table over the active footprint editor.
    OpenCustom,
    /// v0.18.14.1 — Custom Selection Filter modal: Cancel / Esc.
    /// Discards the in-flight draft.
    CloseCustom,
    /// v0.18.14.1 — Custom Selection Filter modal: per-row checkbox
    /// toggle.
    ToggleCustomKind(crate::library::editor::footprint::state::SelectionFilterKind),
    /// v0.18.14.1 — Custom Selection Filter modal: Apply button.
    /// Writes the draft into the active footprint editor's
    /// `state.selection_filter` then closes.
    ApplyCustom,
}

/// File / save message family (ADR-0001 D3). Namespaced under
/// `Message::File` and routed to `dispatch_file_message`.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum FileMsg {
    Opened(Option<PathBuf>),
    /// File ▸ New Project — destination picked by the Save-As dialog.
    /// `None` when the user cancelled the picker; on `Some(path)` the
    /// handler writes a fresh `<stem>.snxprj` (empty marker file — the
    /// parser is directory-driven) plus a blank `<stem>.snxsch` next
    /// to it, then loads the project + opens the schematic tab.
    NewProject(Option<PathBuf>),
    #[allow(dead_code)]
    SchematicLoaded(Box<SchematicSheet>),
    Save,
    SaveAs(PathBuf),
    /// User picked a destination from the Save-As dialog spawned the
    /// first time a freshly-minted `.snxsym` / `.snxfpt` editor tab is
    /// saved (the in-memory tab opened by `Add New ▸ Symbol` /
    /// `Add New ▸ Footprint`). Re-keys the editor + tab from the
    /// suggested path to the user's choice, then writes the file.
    /// `from_path` is the suggested in-memory path the editor is
    /// currently keyed under; `to_path` is the rfd result.
    SavePrimitiveAs {
        from_path: PathBuf,
        to_path: PathBuf,
    },
}

/// Project lifecycle message family (ADR-0001 D3). Namespaced under
/// `Message::Project` and routed to `dispatch_project_message`.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum ProjectMsg {
    /// User picked an option in the project-close confirmation modal
    /// (Save All / Discard All / Cancel) shown when closing a
    /// project that still has entries in `dirty_paths`.
    CloseConfirm(ProjectCloseChoice),
    /// User choice (Save All / Discard All / Cancel) on the app-quit
    /// confirmation modal, shown when the user tries to exit Signex
    /// while `dirty_paths` is non-empty. Reuses `ProjectCloseChoice`.
    AppQuitConfirm(ProjectCloseChoice),
    /// Dismiss the Project Options metadata modal.
    CloseOptions,
    /// Result of the `Add Existing to Project…` file picker. Carries
    /// the owning project's index plus the user's picks (`None` on
    /// cancel, otherwise one or more paths from `pick_files`) so the
    /// handler can copy each into the project directory in turn.
    AddExistingFilePicked {
        project_idx: usize,
        paths: Option<Vec<std::path::PathBuf>>,
    },
    /// Result of the `Add New ▸ Schematic` Save-As dialog. `None`
    /// when the user cancelled; on `Some(path)` the handler writes
    /// a blank `.snxsch`, registers it on the project, and marks
    /// the .snxprj dirty.
    AddNewSchematicPicked {
        project_idx: usize,
        path: Option<std::path::PathBuf>,
    },
    /// v0.23 — Async project-git commit completed. The dispatcher
    /// removes the `(project_root, rel_path)` entry from
    /// `inflight_git_commits` and logs success/failure. `result.Ok`
    /// carries the formatted commit OID; `result.Err` carries the
    /// error string. Best-effort — a failure here doesn't roll back
    /// the on-disk save (data is already on disk; this just means git
    /// didn't capture it).
    GitCommitDone {
        project_root: std::path::PathBuf,
        rel_path: std::path::PathBuf,
        result: Result<String, String>,
    },
}

/// Edit-command message family (ADR-0001 D3). Namespaced under
/// `Message::Edit` and routed to `dispatch_edit_message`.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum EditMsg {
    /// Delete the current selection. In a footprint editor this routes
    /// to the footprint dispatcher's `DeleteSelected`; otherwise the
    /// schematic engine removes the selected elements.
    DeleteSelected,
    /// Undo the most recent edit.
    Undo,
    /// Redo the most recently undone edit.
    Redo,
    /// Rotate the current selection.
    RotateSelected,
    /// Mirror the current selection about the X axis.
    MirrorSelectedX,
    /// Mirror the current selection about the Y axis.
    MirrorSelectedY,
    /// Copy the current selection to the clipboard.
    Copy,
    /// Cut the current selection to the clipboard.
    Cut,
    /// Paste the clipboard contents.
    Paste,
    /// Smart-paste the clipboard contents.
    SmartPaste,
    /// Duplicate the current selection in place.
    Duplicate,
}

/// Per-shape edit descriptor. The Properties panel dispatches one of
/// these per numeric text-input edit; the handler looks up the stored
/// drawing, applies the field change, and emits
/// `Command::UpdateSchDrawing` with the patched variant.
#[derive(Debug, Clone, Copy)]
pub enum DrawingFieldEdit {
    Width(f64),
    Fill(signex_types::schematic::FillType),
    LineStartX(f64),
    LineStartY(f64),
    LineEndX(f64),
    LineEndY(f64),
    RectStartX(f64),
    RectStartY(f64),
    RectWidthMm(f64),
    RectHeightMm(f64),
    CircleCenterX(f64),
    CircleCenterY(f64),
    CircleRadius(f64),
    ArcCenterX(f64),
    ArcCenterY(f64),
    ArcRadius(f64),
    ArcStartAngle(f64),
    ArcEndAngle(f64),
    /// Override the stroke colour; `None` restores the theme default.
    StrokeColor(Option<signex_types::schematic::StrokeColor>),
}

/// R / G / B channel selector for the custom net-colour picker inputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    R,
    G,
    B,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum ContextAction {
    Copy,
    Cut,
    Paste,
    SmartPaste,
    OpenChildSheet,
    Delete,
    SelectAll,
    ZoomFit,
    RotateSelected,
    MirrorX,
    MirrorY,
    /// Run an Active Bar action from a context-menu submenu (Place /
    /// Align). Closes both menus and dispatches the action through the
    /// existing Active Bar handler so all the placement / transform
    /// logic stays in one place.
    ActiveBar(crate::active_bar::ActiveBarAction),
}

/// Which click-to-open submenu is currently expanded inside the right-
/// click context menu, if any. Owned by `InteractionState` and cleared
/// alongside `context_menu` whenever the menu closes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextSubmenu {
    Place,
    Align,
    /// Project-tree → "Add New to Project ›" launcher. Items are
    /// version-tagged placeholders today — actual document creation
    /// lands with project-write support in v0.9.
    AddNewToProject,
}

#[derive(Debug, Clone)]
pub struct TextEditState {
    pub uuid: uuid::Uuid,
    pub kind: signex_types::schematic::SelectedKind,
    pub text: String,
    pub original_text: String,
    /// World-space position of the object being edited (mm). Converted to
    /// screen coords at render time so the inline editor tracks pan/zoom.
    pub world_x: f64,
    pub world_y: f64,
}

#[derive(Debug, Clone)]
pub struct ContextMenuState {
    pub x: f32,
    pub y: f32,
}

/// v0.18.10 — Altium-style grid picker popup state. Anchors the
/// floating menu at the cursor position when `G` is pressed.
#[derive(Debug, Clone)]
pub struct GridPickerState {
    pub x: f32,
    pub y: f32,
}

/// v0.18.11 — Cartesian Grid Editor modal state. Carries the
/// in-flight Step X / Step Y string buffers + the X/Y link toggle.
/// Writes happen on `GridPropertiesApply`; close discards.
///
/// v0.18.19 added Fine / Coarse display + Multiplier draft fields.
#[derive(Debug, Clone)]
pub struct GridPropertiesState {
    pub step_x_mm: String,
    pub step_y_mm: String,
    pub link_xy: bool,
    pub fine_display: crate::library::editor::footprint::state::GridDisplay,
    pub coarse_display: crate::library::editor::footprint::state::GridDisplay,
    pub multiplier: u32,
}

/// v0.18.14.1 — Custom Selection Filter modal draft state. Mirrors
/// `SelectionFilter` from `library::editor::footprint::state` so
/// the user can flip flags without touching the live editor until
/// they hit Apply.
#[derive(Debug, Clone)]
pub struct SelectionFilterCustomState {
    pub pads: bool,
    pub tracks: bool,
    pub arcs: bool,
    pub pours: bool,
    pub bodies_3d: bool,
    pub keepouts: bool,
    pub cutouts: bool,
    pub texts: bool,
    pub vias: bool,
    pub regions: bool,
    pub fills: bool,
    pub other: bool,
}

/// State for the Projects-panel tree-view right-click menu. The menu's
/// action set is computed from `path` (leaf vs branch vs empty) at render
/// time, so we only need to store the anchor coordinates + the clicked
/// path (or `None` for the background menu).
#[derive(Debug, Clone)]
pub struct ProjectTreeContextMenuState {
    pub x: f32,
    pub y: f32,
    /// `Some(path)` = right-click on a specific node; `None` = right-click
    /// in empty tree area, offering only the generic actions.
    pub path: Option<Vec<usize>>,
}

/// State for the "Close Project — Unsaved Edits" confirmation modal.
/// Opens only when the user closes a project that has at least one
/// entry in `DocumentState.dirty_paths` rooted in the project's
/// directory; the modal lists every dirty file by filename so the
/// user can see what they're about to lose.
#[derive(Debug, Clone)]
pub struct ProjectCloseConfirmState {
    /// Project root tree path the close was requested for. Stored so
    /// the modal's confirm action can dispatch back to
    /// `close_project_at_tree_path` without re-resolving from the
    /// project list (which may shift if the user closes another
    /// project while this modal is up — Altium's modal is dismiss-
    /// only, so this is defence-in-depth).
    pub tree_path: Vec<usize>,
    /// Project display name shown in the modal header.
    pub project_name: String,
    /// Absolute paths of dirty files inside the project's directory.
    /// The view layer renders the file basenames; the handler uses
    /// the absolute paths to locate the engines for save / discard.
    pub dirty_paths: Vec<std::path::PathBuf>,
}

/// State for the "Exit Signex — Unsaved Edits" confirmation modal.
/// Opens when the user requests app exit (chrome ✕, File ▸ Exit,
/// Alt+F4) while `DocumentState.dirty_paths` is non-empty. Lists
/// every dirty file across the whole workspace so the user sees what
/// they are about to lose before choosing Save All / Discard All /
/// Cancel. Reuses `ProjectCloseChoice` for the three outcomes.
#[derive(Debug, Clone)]
pub struct AppQuitConfirmState {
    /// Absolute paths of every dirty file in the workspace, sorted
    /// for a stable display order. The view renders the basenames.
    pub dirty_paths: Vec<std::path::PathBuf>,
}

/// User choice from the project-close confirmation modal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectCloseChoice {
    /// Save every dirty file in the project, then close.
    SaveAll,
    /// Drop the engines for every dirty file in the project without
    /// writing to disk, then close.
    DiscardAll,
    /// Dismiss the modal; the project stays open.
    Cancel,
}

/// State for the document-tab right-click menu. The menu's items are
/// derived from `tab_idx` (the clicked tab) so the same menu builder
/// works for any tab; mutually exclusive with the canvas and project-
/// tree context menus.
#[derive(Debug, Clone)]
pub struct TabContextMenuState {
    pub x: f32,
    pub y: f32,
    pub tab_idx: usize,
}

/// Concrete actions dispatched when the user picks a menu item in the
/// document-tab right-click menu.
#[derive(Debug, Clone)]
pub enum TabContextAction {
    /// Close just this tab.
    Close(usize),
    /// Close every tab except the one at this index.
    CloseAllOthers(usize),
    /// Close every open tab.
    CloseAll,
    /// Pop the tab at this index into its own OS window.
    Undock(usize),
}

/// Concrete actions dispatched when the user picks a menu item in the
/// Projects-panel tree-view context menu.
#[derive(Debug, Clone)]
pub enum ProjectTreeAction {
    /// Open the file backed by this leaf in the current document slot.
    OpenNode(Vec<usize>),
    /// Expand (or collapse) a specific branch node.
    ToggleNode(Vec<usize>),
    /// Recursively expand every node in the tree.
    ExpandAll,
    /// Recursively collapse every node in the tree.
    CollapseAll,
    /// Re-scan the project and rebuild the tree from current state.
    Refresh,
    /// Close every open document tab without closing the project
    /// itself. Fired from the project-root "Close Project Documents"
    /// menu item.
    CloseAllDocuments,
    /// Reveal a file (leaf click) or the project directory (root
    /// click) in the OS file manager. The tree path's first index
    /// picks which project's directory the operation resolves
    /// against — leaves nested under project B reveal in B's dir
    /// even when project A is active. A single-element path means
    /// the project root row was clicked.
    RevealInExplorer(Vec<usize>),
    /// Fire the print preview flow — only surfaced on leaves that are
    /// already the active tab.
    PrintActive,
    /// Open the sheet-rename modal for this leaf, preloaded with the
    /// current filename.
    OpenRenameDialog(Vec<usize>),
    /// Open the "Remove from Project" modal (Delete / Exclude / Cancel)
    /// for this leaf.
    OpenRemoveDialog(Vec<usize>),
    /// Close the entire project whose root is at this tree path. Closes
    /// every open tab backed by the project, drops the `LoadedProject`
    /// from the workspace, and promotes another project (or `None`) to
    /// active. The tree path's first index selects the project; other
    /// indices are ignored so the action is safe to fire from any node
    /// underneath a project root.
    CloseProject(Vec<usize>),
    /// v0.9 project-root: run ERC across the project. Promotes the
    /// project to active, opens its `schematic_root` if no schematic
    /// from this project is currently active, then dispatches the
    /// existing ERC dialog.
    ValidateProject(Vec<usize>),
    /// v0.9 project-root: open the rename modal seeded with the
    /// project name (the `.snxprj` stem). Submit renames the trio
    /// `<old>.snxprj` / `<old>.snxsch` / `<old>.snxpcb` in lockstep.
    OpenProjectRenameDialog(Vec<usize>),
    /// v0.9 project-root: open the Project Options metadata modal.
    OpenProjectOptions(Vec<usize>),
    /// v0.9 project-root: open a file dialog and add the picked file
    /// to the project. Files outside the project directory are copied
    /// in; files already inside just trigger a tree refresh.
    AddExistingToProject(Vec<usize>),
    /// v0.9 project-root → Add New ▸ Schematic. Spawns a Save-As
    /// dialog scoped to the project directory; the result writes a
    /// blank `.snxsch`, registers it as a SheetEntry, marks the
    /// project dirty, and refreshes the tree (no tab opens).
    AddNewSchematic(Vec<usize>),
    /// project-root → Add New ▸ Symbol Library. Save-As dialog
    /// scoped to the project dir, writes an empty `.snxsym`, opens
    /// the file as a primitive editor tab. Altium parity: Schematic
    /// Library is a top-level project document.
    AddProjectSymbolLibrary(Vec<usize>),
    /// project-root → Add New ▸ PCB Library. Save-As dialog
    /// scoped to the project dir, writes an empty `.snxfpt`, opens
    /// the file as a primitive editor tab.
    AddProjectFootprintLibrary(Vec<usize>),
    /// v0.11 project-root: open the Enable Version Control confirm
    /// modal. Runs `git init` at the project dir, optionally seeds
    /// `.gitattributes` for binary-model LFS, and creates the
    /// initial commit covering the entire project tree. Only
    /// enabled when the project dir has no `.git/` already.
    OpenEnableVersionControl(Vec<usize>),
    /// Right-click on a plain-files `.snxlib` node → opens the
    /// Enable Version Control modal scoped to that library directory.
    OpenLibraryEnableVersionControl(Vec<usize>),
}

/// State for the rename modal. Tracks the target file, the live
/// edit buffer, and the clicked tree path so we can rebuild the tree
/// after a successful rename without rediscovering the project.
#[derive(Debug, Clone)]
pub struct RenameDialogState {
    pub target_path: std::path::PathBuf,
    pub tree_path: Vec<usize>,
    pub buffer: String,
    pub error: Option<String>,
    /// `true` when this rename targets the project root — the submit
    /// handler renames the `<old>.snxprj` plus the companion
    /// `<old>.snxsch` / `<old>.snxpcb` (whichever exist on disk) so the
    /// trio stays grouped under the new project name. `false` is the
    /// per-file rename used by sheet leaves.
    pub is_project_rename: bool,
}

/// One file/directory entry surfaced on the Enable Version Control
/// picker. The user can opt items out of the initial commit by
/// untoggling `tracked`; untracked entries get written into a
/// generated `.gitignore` so they sit outside the repo from day one.
#[derive(Debug, Clone)]
pub struct TrackItem {
    pub absolute: std::path::PathBuf,
    pub relative: String,
    /// Short kind badge ("Schematic", "PCB", "Library", "Folder",
    /// "Config", etc.) shown next to the path in the picker.
    pub label: String,
    /// True for directory entries — drives trailing-slash in the
    /// generated `.gitignore` pattern.
    pub is_directory: bool,
    pub tracked: bool,
}

/// Whether the Enable Version Control modal is initialising a
/// project repo (whole-project tree) or a library repo (a single
/// `.snxlib` directory). Branches the confirm handler so it can
/// run `git init` against the right working tree and emit the
/// scope-appropriate log line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionControlScope {
    Project,
    Library,
}

/// State for the "Enable Version Control" confirm modal — opened
/// from the project root context menu when the project directory
/// has no `.git/` yet, or from a plain-files `.snxlib` node's
/// right-click menu. Confirm runs `git2::Repository::init` at
/// `project_dir`, optionally writes `.gitattributes` for binary-
/// model LFS, generates a `.gitignore` from the unticked items,
/// and stages an initial commit covering the picked subset.
#[derive(Debug, Clone)]
pub struct EnableVersionControlState {
    /// Whether this dialog is scoped to a project (the `.snxprj` +
    /// surrounding tree) or to a library directory (a single
    /// `.snxlib` and its `symbols/` / `footprints/` siblings).
    pub scope: VersionControlScope,
    /// For `Project`: path to the `.snxprj` file. For `Library`:
    /// path to the `library.toml` (or equivalent manifest) inside
    /// the library directory — used only for display.
    pub project_path: std::path::PathBuf,
    /// Working tree root the new repo will live at. For projects
    /// this is the `.snxprj` parent; for libraries the `.snxlib`
    /// parent (i.e. the library's root_dir).
    pub project_dir: std::path::PathBuf,
    /// Display name for the modal header. Project: project name.
    /// Library: filename stem of the `.snxlib` (e.g. "MyLib").
    pub project_name: String,
    /// Per-entry tracking picker — tickable rows for each top-level
    /// schematic / pcb / library (project scope) or each top-level
    /// manifest / subdirectory (library scope). Untracked rows get
    /// written into `.gitignore` at confirm time.
    pub items: Vec<TrackItem>,
    /// "Track binary 3D models via Git LFS" checkbox. Off by
    /// default; only writes `.gitattributes` when on.
    pub use_lfs: bool,
    /// Pre-formatted intro paragraph that interpolates the working
    /// tree path. Computed once at modal-open time so the view
    /// doesn't allocate a fresh `String` on every render frame.
    pub intro_text: String,
    /// Last error from a confirm attempt — surfaces inline so the
    /// user can fix the cause (LFS not installed, etc.) and retry
    /// without reopening the modal.
    pub error: Option<String>,
}

/// State for the read-only "Project Options" modal — the v0.9 surface
/// is a metadata summary (name / dir / schematic root / pcb file /
/// libraries). Editing happens through the dedicated rename / library
/// flows; a future revision can promote this to a full editor.
#[derive(Debug, Clone)]
pub struct ProjectOptionsState {
    pub project_idx: usize,
    pub name: String,
    pub directory: String,
    pub schematic_root: Option<String>,
    pub pcb_file: Option<String>,
    pub library_count: usize,
}

/// State for the "Remove from Project" modal. `Delete` removes the file
/// from disk; `Exclude` drops it from the session's sheet list but
/// leaves the file in place.
#[derive(Debug, Clone)]
pub struct RemoveDialogState {
    pub target_path: std::path::PathBuf,
    pub tree_path: Vec<usize>,
    pub display_name: String,
}

/// User choice from the Remove-from-Project modal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoveChoice {
    /// Remove from project AND delete the file on disk.
    DeleteFile,
    /// Remove from project; leave the file in its folder.
    ExcludeFromProject,
}

#[derive(Debug, Clone)]
pub enum StatusBarRequest {
    CycleUnit,
    ToggleGrid,
    ToggleSnap,
    TogglePanelList,
    /// Click on the selection-summary segment opens the Properties panel
    /// scoped to the current selection.
    OpenPropertiesForSelection,
}
