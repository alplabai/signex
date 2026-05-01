pub struct UiState {
    pub theme_id: signex_types::theme::ThemeId,
    pub unit: signex_types::coord::Unit,
    pub grid_visible: bool,
    pub snap_enabled: bool,
    pub cursor_x: f64,
    pub cursor_y: f64,
    pub zoom: f64,
    pub grid_size_mm: f32,
    pub visible_grid_mm: f32,
    pub snap_hotspots: bool,
    pub ui_font_name: String,
    pub canvas_font_name: String,
    pub canvas_font_size: f32,
    pub canvas_font_bold: bool,
    pub canvas_font_italic: bool,
    pub left_width: f32,
    pub right_width: f32,
    pub bottom_height: f32,
    pub window_size: (f32, f32),
    /// OS-reported scale factor for the main window (1.0 at 100 %,
    /// 1.25 at 125 %, 2.0 at 200 %, etc.). Populated on
    /// `MainWindowOpened` and refreshed on every main-window resize,
    /// which also fires when Windows moves the window to a different
    /// monitor. Used to pick the 1x/2x/3x wordmark PNG so the brand
    /// lockup stays 1:1 with device pixels.
    pub main_window_scale: f32,
    pub panel_list_open: bool,
    pub preferences_open: bool,
    pub find_replace: crate::find_replace::FindReplaceState,
    pub preferences_nav: crate::preferences::PrefNav,
    pub preferences_draft_theme: signex_types::theme::ThemeId,
    pub preferences_draft_font: String,
    pub power_port_style: signex_render::PowerPortStyle,
    pub preferences_draft_power_port_style: signex_render::PowerPortStyle,
    pub label_style: signex_render::LabelStyle,
    pub preferences_draft_label_style: signex_render::LabelStyle,
    pub multisheet_style: signex_render::MultisheetStyle,
    pub preferences_draft_multisheet_style: signex_render::MultisheetStyle,
    pub grid_style: signex_render::GridStyle,
    pub preferences_draft_grid_style: signex_render::GridStyle,
    pub preferences_dirty: bool,
    pub custom_theme: Option<signex_types::theme::CustomThemeFile>,
    /// Rename-sheet modal state. Opened from the Projects-panel tree
    /// context menu; `None` when the modal is closed.
    pub rename_dialog: Option<crate::app::RenameDialogState>,
    /// Remove-from-project modal state (Delete / Exclude / Cancel).
    pub remove_dialog: Option<crate::app::RemoveDialogState>,
    /// "Close Project — Unsaved Edits" confirmation modal. `Some`
    /// while the user is being asked to save / discard / cancel a
    /// close request that intersects `dirty_paths`. Cleared on any
    /// of the three button choices.
    pub project_close_confirm: Option<crate::app::ProjectCloseConfirmState>,
    pub erc: super::ErcState,
    pub annotate: super::AnnotateState,
    pub net_color: super::NetColorState,
    /// AutoFocus mode — when true, non-selected items dim on the canvas.
    pub auto_focus: bool,
    /// Per-modal offset in window pixels from the centered position.
    /// Updated when the user drags the title bar. Persists until the app
    /// closes so reopening a dialog lands where it was last placed.
    pub modal_offsets: std::collections::HashMap<super::ModalId, (f32, f32)>,
    /// Active modal drag: which modal is being dragged + the last mouse
    /// position so the delta can be computed from the next DragMove event.
    pub modal_dragging: Option<(super::ModalId, f32, f32)>,
    /// Active tab drag: which document tab is being dragged + the last
    /// mouse position. Used by auto-detach — when the cursor crosses the
    /// main window edge the tab undocks into its own OS window.
    pub tab_dragging: Option<(usize, f32, f32)>,
    /// Move-Selection dialog state (Altium's numeric DeltaX / DeltaY move).
    pub move_selection: super::MoveSelectionState,
    /// Parameter Manager dialog state.
    pub parameter_manager_open: bool,
    /// Active "pick a reference item" mode for z-order operations
    /// (BringToFrontOf / SendToBackOf). When Some, the next canvas click
    /// resolves the reference uuid and submits the Reorder command.
    pub reorder_picker: Option<super::ReorderPicker>,
    /// Altium-style rubber-band selection mode. Drives how the box
    /// drag classifies hits (Inside / Outside / TouchingLine).
    pub selection_mode: signex_render::schematic::hit_test::SelectionMode,
    /// Altium-style lasso in flight. `Some(points)` means the user
    /// started a lasso — each canvas click appends a vertex; a
    /// double-click or a click on the first vertex closes the polygon
    /// and commits the selection. Escape or right-click cancels.
    pub lasso_polygon: Option<Vec<signex_types::schematic::Point>>,
    /// Id of the primary app window — set once `iced::window::open` for
    /// the main window resolves. Every `view(id)` call checks this to
    /// decide whether it's rendering the main shell or a secondary
    /// (detached modal / undocked tab) window.
    pub main_window_id: Option<iced::window::Id>,
    /// Every non-main window Signex owns, keyed by its iced id. Lets
    /// `view(id)` dispatch between the main shell, detached modals, and
    /// (later) undocked tabs. `SecondaryWindowClosed` removes entries so
    /// the detached content reattaches to the main window.
    pub windows: std::collections::HashMap<iced::window::Id, super::WindowKind>,
    /// Paths whose async save (v0.9.1 perf path) is currently running
    /// off the UI thread. Drives the "Saving..." pill in the status bar
    /// and is cleared on `Message::SaveFileFinished`. Failed saves
    /// stay in `save_error` for a few seconds so the operator sees
    /// what happened.
    pub saving_paths: std::collections::HashSet<std::path::PathBuf>,
    /// Last save error message and the time it was set. The status
    /// bar shows this briefly, then `tick_save_error` clears it.
    pub save_error: Option<(String, std::time::Instant)>,
}
