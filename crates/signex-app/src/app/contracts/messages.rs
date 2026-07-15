//! Sub-message enums: window / palette / overlay / selection / panel edits.

use super::*;

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
    /// Tools -> PCB Trace Calculator opens the IPC-2221 utility window.
    OpenPcbTraceCalculator,
    /// `iced::window::open` resolved for the PCB Trace Calculator.
    PcbTraceCalculatorOpened(iced::window::Id),
    /// OS-level close request (native close button, Alt+F4, taskbar
    /// close) for a window. In `iced::daemon` mode windows do NOT
    /// auto-close on this event, so the app decides what to do: the
    /// main window routes through the unsaved-changes guard
    /// (`CloseMainWindow`); every other window closes directly.
    WindowCloseRequested(iced::window::Id),
    /// Pop a modal out of the main window into its own OS window. Altium
    /// triggers this when the user drags the modal's title bar past the
    /// main window edge, or clicks the pop-out icon in the title bar.
    DetachModal(crate::app::state::ModalId),
    /// Fired after `window::open` resolves for a detached modal — stores
    /// the new window's id in `ui_state.windows` so `view(id)` can render
    /// it and `SecondaryWindowClosed` can reattach when the user dismisses
    /// the window.
    DetachedModalOpened {
        modal: crate::app::state::ModalId,
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
    StartDetachedWindowDrag(crate::app::state::ModalId),
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
        modal: crate::app::state::ModalId,
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
        modal: crate::app::state::ModalId,
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
    /// Toggle electrical-hotspot snapping (the keymap's
    /// `toggle_electrical_grid` command). Namespaced here rather than as
    /// a flat root variant, per ADR-0001 D3.
    ToggleSnapHotspots,
    /// A raw keystroke forwarded from the keyboard subscription for
    /// keymap resolution. Multi-stroke chords are accumulated in
    /// `UiState::keymap_pending_sequence` and resolved here in `update`
    /// (where `&mut self` is available), which keeps the chord buffer
    /// out of a process-global static.
    KeymapStroke(crate::keymap::KeyStroke),
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
    SetSidebarTab(crate::app::state::BomSidebarTab),
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
    SetTab(crate::app::state::PdfPreviewTab),
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
    SetQuality(crate::app::state::PdfQuality),
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
