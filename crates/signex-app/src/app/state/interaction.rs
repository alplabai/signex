//! Canvas interaction state — active tool, per-window canvases, drag /
//! context-menu / text-edit sub-state. Split from `app/state.rs`.

use crate::canvas::SchematicCanvas;
use crate::pcb_canvas::PcbCanvas;

use super::super::{ContextMenuState, DragTarget, DrawMode, TextEditState, Tool};

pub struct InteractionState {
    pub current_tool: Tool,
    /// The main-window schematic canvas. Every non-main window carries
    /// its own `SchematicCanvas` inside `canvases`, keyed by that
    /// window's `iced::window::Id`. The event-dispatch layer swaps a
    /// per-window canvas into this slot while handling an event so the
    /// hundreds of `active_canvas_mut()` call sites don't need to know
    /// about per-window routing.
    pub canvas: SchematicCanvas,
    /// Extra schematic canvases owned by non-main windows (undocked
    /// tabs). Populated on `Message::Window(WindowMsg::UndockedTabOpened)`; drained on
    /// `Message::Window(WindowMsg::SecondaryWindowClosed)`. Reads go through
    /// `canvas_for_window`; writes happen via the dispatch swap trick.
    pub canvases: std::collections::HashMap<iced::window::Id, SchematicCanvas>,
    pub pcb_canvas: PcbCanvas,
    pub dragging: Option<DragTarget>,
    pub drag_start_pos: Option<f32>,
    pub drag_start_size: f32,
    pub tab_drag_origin: Option<(f32, f32)>,
    pub undo_stack: crate::undo::UndoStack,
    pub wire_points: Vec<signex_types::schematic::Point>,
    pub wire_drawing: bool,
    /// 3-click arc placement buffer. Holds the first two clicks
    /// (start, mid); the third click commits as SchDrawing::Arc.
    pub arc_points: Vec<signex_types::schematic::Point>,
    /// Freehand polygon placement buffer. Accumulates clicks until
    /// the user presses Enter / double-clicks / right-clicks.
    pub polyline_points: Vec<signex_types::schematic::Point>,
    /// Two-click shape placement: first click sets the anchor, second
    /// click commits. Used by Tool::Line, Tool::Rectangle, Tool::Circle.
    pub shape_anchor: Option<signex_types::schematic::Point>,
    pub clipboard_wires: Vec<signex_types::schematic::Wire>,
    pub clipboard_buses: Vec<signex_types::schematic::Bus>,
    pub clipboard_labels: Vec<signex_types::schematic::Label>,
    pub clipboard_symbols: Vec<signex_types::schematic::Symbol>,
    pub clipboard_junctions: Vec<signex_types::schematic::Junction>,
    pub clipboard_no_connects: Vec<signex_types::schematic::NoConnect>,
    pub clipboard_text_notes: Vec<signex_types::schematic::TextNote>,
    pub draw_mode: DrawMode,
    pub editing_text: Option<TextEditState>,
    pub context_menu: Option<ContextMenuState>,
    /// Projects-panel tree-view right-click menu state. Separate from
    /// `context_menu` (canvas-scoped) because the two menus have no
    /// overlap in actions and the canvas menu depends on placement /
    /// selection state that does not exist in the panel context.
    pub project_tree_context_menu: Option<crate::app::ProjectTreeContextMenuState>,
    /// v0.18.10 — Altium-style grid picker popup. Anchored at the
    /// cursor when the user presses `G`. Lists the standard grid
    /// ladder (1mil…2.5mm). Picking an item writes the active
    /// footprint editor's `snap_options.grid_step_mm`; outside-click
    /// or Esc dismisses.
    pub grid_picker: Option<crate::app::GridPickerState>,
    /// Document-tab right-click menu state. Anchored at the right-click
    /// coordinates inside the tab strip; carries the index of the
    /// clicked tab so per-tab actions ("Close [filename]") resolve
    /// against the correct entry. Mutually exclusive with
    /// `context_menu` and `project_tree_context_menu` — opening one
    /// dismisses the others.
    pub tab_context_menu: Option<crate::app::TabContextMenuState>,
    /// Currently-expanded submenu inside the right-click context menu
    /// (None when no submenu is shown). Always cleared when
    /// `context_menu` becomes None.
    pub context_submenu: Option<crate::app::ContextSubmenu>,
    /// `(kind, hover_started_at)` for the submenu launcher the cursor
    /// is currently hovering. The 50 ms hover-tick subscription opens
    /// the submenu once `hover_started_at + 200 ms <= Instant::now()`,
    /// matching the standard Altium / Windows menu delay.
    pub pending_submenu: Option<(crate::app::ContextSubmenu, std::time::Instant)>,
    /// Which submenu launcher row the cursor is currently over, or
    /// `None`. Paired with `submenu_panel_hovered` to decide whether
    /// the open submenu should stay visible.
    pub submenu_launcher_hovered: Option<crate::app::ContextSubmenu>,
    /// Whether the cursor is currently over the opened submenu panel.
    pub submenu_panel_hovered: bool,
    /// Timestamp of when *both* the launcher and the panel became
    /// unhovered. The 50 ms tick closes the submenu once 150 ms has
    /// elapsed, giving the user time to cross the gap between the two
    /// zones without the menu collapsing mid-traversal.
    pub submenu_unhovered_since: Option<std::time::Instant>,
    pub last_mouse_pos: (f32, f32),
    /// Project-tree click memo for the home-grown double-click gate.
    /// First click on an openable leaf records `(path, instant)`; the
    /// second click on the same path within `TREE_DOUBLE_CLICK_WINDOW`
    /// opens the document. Anything else (different path, expired
    /// timer, intervening folder toggle) just refreshes the memo.
    /// Iced's `button` widget consumes mouse events before iced 0.14
    /// surfaces a built-in `on_double_click` for them, so we time the
    /// clicks ourselves at the app layer.
    pub last_tree_click: Option<(Vec<usize>, std::time::Instant)>,
    pub active_bar_menu: Option<crate::active_bar::ActiveBarMenu>,
    pub selection_filters: std::collections::HashSet<crate::active_bar::SelectionFilter>,
    /// User-defined custom filter presets (capped at
    /// `crate::active_bar::CUSTOM_FILTER_PRESET_LIMIT`). Loaded from
    /// `~/.config/signex/prefs.json` on launch and written back when
    /// edited from the Properties panel.
    pub custom_filter_presets: Vec<crate::active_bar::CustomFilterPreset>,
    /// Index of the active preset tab in the Properties-panel editor.
    /// Clamped to `0..custom_filter_presets.len()` whenever the list
    /// changes; ignored entirely when the list is empty.
    pub active_custom_filter_tab: usize,
    /// User-defined footprint-editor filter presets (capped at
    /// `crate::active_bar::CUSTOM_FILTER_PRESET_LIMIT`). Loaded from
    /// `~/.config/signex/prefs.json` on launch; parallel to
    /// `custom_filter_presets` but keyed on `SelectionFilterKind`
    /// (footprint categories) instead of the schematic
    /// `SelectionFilter` (Task 6).
    pub footprint_filter_presets: Vec<crate::active_bar::FootprintFilterPreset>,
    pub selection_slots: [Vec<signex_types::schematic::SelectedItem>; 8],
    pub last_tool: std::collections::HashMap<String, crate::active_bar::ActiveBarAction>,
    pub pending_power: Option<(String, String)>,
    pub pending_port: Option<(signex_types::schematic::LabelType, String)>,
    /// Uuid of the placed symbol the cursor is currently hovering over,
    /// if any. Set/cleared by the canvas `CursorAt` handler. Drives the
    /// hover tooltip overlay (designator + value + footprint + lib_id).
    pub hover_symbol_uuid: Option<uuid::Uuid>,
    /// Wall-clock timestamp at which `hover_symbol_uuid` was first set
    /// to its current value. Used by the view to gate the tooltip
    /// behind a 250 ms delay so hovering is a deliberate gesture.
    /// Resets when the hovered uuid changes.
    pub hover_started_at: Option<std::time::Instant>,
    /// Last-known window-relative cursor position while hovering a
    /// symbol — drives the tooltip's screen-space placement so the
    /// card tracks the cursor (offset to bottom-right by ~16 px so it
    /// doesn't obscure the symbol).
    pub hover_screen_pos: Option<(f32, f32)>,
}

impl InteractionState {
    pub fn active_canvas(&self) -> &SchematicCanvas {
        &self.canvas
    }

    pub fn active_canvas_mut(&mut self) -> &mut SchematicCanvas {
        &mut self.canvas
    }

    /// Per-window canvas lookup. Returns the per-window `SchematicCanvas`
    /// if one is registered (undocked windows), otherwise the main
    /// window's shared canvas. Writes from canvas events still go
    /// through the main-canvas slot; see the dispatch swap trick in
    /// `dispatch::ui::handle_canvas_event_in_window`.
    pub fn canvas_for_window(&self, window_id: iced::window::Id) -> &SchematicCanvas {
        self.canvases.get(&window_id).unwrap_or(&self.canvas)
    }

    #[allow(dead_code)]
    pub fn canvas_for_window_mut(&mut self, window_id: iced::window::Id) -> &mut SchematicCanvas {
        // `get_mut` returns `Option<&mut V>`. Match rather than
        // `contains_key` + `get_mut().unwrap()` to avoid the double
        // lookup and the unwrap.
        match self.canvases.get_mut(&window_id) {
            Some(canvas) => canvas,
            None => &mut self.canvas,
        }
    }
}
