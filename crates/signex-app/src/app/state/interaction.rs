//! Canvas interaction state — active tool, per-window canvases, drag /
//! context-menu / text-edit sub-state. Split from `app/state.rs`.

use crate::canvas::CanvasSlot;
use crate::pcb_canvas::PcbCanvas;

use super::super::{ContextMenuState, DragTarget, TextEditState, Tool};

pub struct InteractionState {
    pub current_tool: Tool,
    /// The main-window schematic canvas. Every non-main window carries
    /// its own `CanvasSlot` inside `canvases`, keyed by that
    /// window's `iced::window::Id`. The event-dispatch layer swaps a
    /// per-window canvas into this slot while handling an event so the
    /// hundreds of `active_canvas_mut()` call sites don't need to know
    /// about per-window routing.
    pub canvas: CanvasSlot,
    /// Extra schematic canvases owned by non-main windows (undocked
    /// tabs). Populated on `Message::Window(WindowMsg::UndockedTabOpened)`; drained on
    /// `Message::Window(WindowMsg::SecondaryWindowClosed)`. Reads go through
    /// `canvas_for_window`; writes happen via the dispatch swap trick.
    pub canvases: std::collections::HashMap<iced::window::Id, CanvasSlot>,
    /// The main-window PCB canvas. Like `canvas`, the event-dispatch layer
    /// swaps an undocked window's entry from `pcb_canvases` into this slot
    /// while it handles that window's event.
    pub pcb_canvas: PcbCanvas,
    /// PCB canvases owned by undocked PCB tab windows, so each window keeps
    /// its own camera, viewport and caches (#494). Populated and drained
    /// alongside `canvases`. Reads go through `pcb_canvas_for_window`.
    pub pcb_canvases: std::collections::HashMap<iced::window::Id, PcbCanvas>,
    /// Canvases of the main window's unfocused tile panes (#2249).
    pub tile_panes: TilePaneCanvases,
    /// Hover, selection and the cached spatial index of the board in view
    /// (#2191). Like `pcb_canvas`, an undocked window's own state is
    /// swapped in from `pcb_selections` while it handles an event.
    pub pcb_selection: crate::pcb_canvas::PcbSelectionState,
    /// The selection of each undocked PCB window (#2240).
    pub pcb_selections:
        std::collections::HashMap<iced::window::Id, crate::pcb_canvas::PcbSelectionState>,
    /// Footprints being dragged or moved, with their preview (#2240).
    pub pcb_carry: Option<crate::pcb_canvas::PcbCarry>,
    /// The board placement tool armed and the shape it is drawing (#2373).
    pub pcb_tool: Option<crate::app::handlers::pcb_tool::BoardToolSession>,
    /// The values the board placement tools place with, set in Properties
    /// while one is armed (#2393); kept for the session.
    pub pcb_tool_defaults: crate::app::handlers::pcb_tool::BoardToolDefaults,
    /// A guide armed by the board panel's Place, waiting for the click on
    /// the main window's board that sets it (#2374): the board and the kind.
    pub pcb_guide_place: Option<(std::path::PathBuf, signex_types::pcb_options::GuideKind)>,
    /// Edit ▸ Origin ▸ Set armed (2026-11-18): the board whose next click on
    /// the main window sets its origin.
    pub pcb_origin_place: Option<std::path::PathBuf>,
    /// The board Measure tool and the highlighted board net (#2403).
    pub pcb_query: super::pcb_query::PcbQueryState,
    /// Arrange in Box armed on a board (#2409): the components it moves
    /// and the first corner once clicked.
    pub pcb_arrange_box: Option<crate::app::board_tools::ArrangeBox>,
    /// The active board's ratsnest lines and the nets hidden from them.
    pub pcb_ratsnest: super::pcb_ratsnest::PcbRatsnest,
    /// The board check the Messages panel lists (#2222).
    pub pcb_drc: super::pcb_drc::PcbDrc,
    /// The board Net Classes panel's draft and notice (#2400).
    pub board_net_classes: super::BoardNetClassPanelState,
    /// Cross Select Mode, what it last saw and a pending Cross Probe (#1312).
    pub cross_select: super::cross_select::CrossSelectState,
    /// The board in sketch mode and its working copy (#2234); `None` when
    /// no board is in sketch mode.
    pub board_sketch: Option<super::BoardSketchSession>,
    /// The board shown in 3D in the main window (#2390); `None` when the
    /// main window shows boards flat.
    pub board_3d: Option<super::Board3dSession>,
    /// Text typed into the selected board footprint's Designator or Value
    /// field and not yet applied with Enter (#1312).
    pub pcb_footprint_draft: Option<crate::app::handlers::pcb_footprint_text::PcbFootprintDraft>,
    /// The board Component view's search and unapplied field text (#2354).
    pub pcb_component: crate::app::handlers::pcb_component::PcbComponentPanelState,
    /// The track, via and pad panel's drafts, search, Show More and net
    /// pick (#2355).
    pub pcb_primitive: crate::app::handlers::pcb_primitive::PcbPrimitivePanel,
    /// The Layer Stack manager's selected row and unapplied cells (#2401).
    pub pcb_layer_stack: crate::app::handlers::pcb_layer_stack::LayerStackPanel,
    pub dragging: Option<DragTarget>,
    pub drag_start_pos: Option<f32>,
    pub drag_start_size: f32,
    pub tab_drag_origin: Option<(f32, f32)>,
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
    /// The last Copy or Cut: the items with the library bodies they use,
    /// anchored at the grid-snapped centre they were copied from.
    pub clipboard: Option<signex_engine::CopiedSelection>,
    /// The system-clipboard text that stands for `clipboard`: what this
    /// window last wrote for it, or the foreign fragment it adopted. A paste
    /// that reads the same text keeps `clipboard` as it is instead of
    /// parsing it back (which would re-anchor it to the current grid).
    pub clipboard_text: Option<String>,
    /// The last Copy or Cut on a board (#2361). One clipboard serves sheets
    /// and boards: a copy on one clears the other, so at most one is set.
    pub pcb_clipboard: Option<signex_engine::pcb::BoardClipboard>,
    /// How a wire or bus run bends at its corner. Kept across segments and
    /// tools until the user flips it or changes the mode (#2372).
    pub corner_rule: signex_types::corner::CornerRule,
    /// How the wires that follow a dragged pin bend (`cycle_wiring_mode`,
    /// #1669). Kept across drags; a move commit carries it as an operand.
    pub drag_wiring: signex_types::corner::DragWiringMode,
    /// How Interactive Routing treats other nets in its way (#2389): walk
    /// around them or push their tracks aside. Kept across routes.
    pub route_mode: signex_engine::pcb::route::RouteMode,
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
    /// clicked tab so per-tab actions ("Close \[filename\]") resolve
    /// against the correct entry. Mutually exclusive with
    /// `context_menu` and `project_tree_context_menu` — opening one
    /// dismisses the others.
    pub tab_context_menu: Option<crate::app::TabContextMenuState>,
    /// Right-click menu of a Start screen project row (#2305): which
    /// project, anchored where the click landed.
    pub start_row_menu: Option<crate::start_screen::StartRowMenu>,
    /// The Start screen navigation row last picked; back to Home once the
    /// Start screen goes away.
    pub start_section: crate::start_screen::StartSection,
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
    pub last_tree_click: crate::double_click::DoubleClick<signex_widgets::tree_view::TreeKey>,
    pub active_bar_menu: Option<crate::active_bar::ActiveBarMenu>,
    /// The undocked window `active_bar_menu` was opened in; `None` for the
    /// main window. Only that window paints the dropdown (#1910).
    pub active_bar_menu_window: Option<iced::window::Id>,
    /// The open dropdown of the main window's PCB Active Bar (#2352).
    pub pcb_bar_menu: Option<crate::app::PcbBarMenu>,
    /// The open right-click menu of a layer tab under a board canvas (#2379).
    pub pcb_layer_tab_menu: Option<crate::app::LayerTabMenu>,
    /// The open menu of the layer set button under a board canvas (#2394).
    pub pcb_layer_set_menu: Option<crate::app::LayerSetMenu>,
    /// The undocked window whose action is running inside `run_in_window`;
    /// `None` for the main window. Stamps what the action opens (a Components
    /// drag) and keeps the app-wide refreshes on the main window.
    pub acting_window: Option<iced::window::Id>,
    /// The window the pointer was last seen in during a pointer drag; `None`
    /// until one reports it. `last_mouse_pos` is in that window's coordinates.
    pub pointer_window: Option<iced::window::Id>,
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
    /// A library part attached to the cursor, waiting for its click.
    pub placement: Option<super::PlacementSession>,
    /// The values the current Tab pause began with; `None` when not paused.
    pub placement_pause: Option<super::SheetPlacementPause>,
    /// The last label or text note placed, for "Repeat last item" (#1253).
    pub last_placement: Option<super::LastPlacement>,
    /// Designator numbers for pasted parts (Next free), seeded when a paste
    /// starts.
    pub paste_counters: Option<super::PasteCounters>,
    /// Token for library reads. Every pick takes a new value and carries it,
    /// so only the LATEST pick may arm a session; Esc, a right-click and any
    /// tool change bump it too, so a read that finishes after a cancel arms
    /// nothing. Arming itself leaves it alone.
    pub placement_epoch: u64,
    pub pending_port: Option<(signex_types::schematic::LabelType, String)>,
    /// Orientation and size of the next bus entry (Space / Shift+Space / X / Y
    /// while the Bus Entry tool is armed). The signs are the orientation; see
    /// `signex_types::schematic::BusEntry`.
    pub bus_entry_size: (f64, f64),
    /// Uuid of the placed symbol the cursor is currently hovering over,
    /// if any. Set/cleared by the canvas `CursorAt` handler. Drives the
    /// hover tooltip overlay (designator + value + footprint + lib_id).
    pub hover_symbol_uuid: Option<uuid::Uuid>,
    /// The pin a click on a placed symbol landed on. Set by the click and
    /// kept by selection refreshes while that symbol, on that sheet in that
    /// placement, stays the selected part; it brings the pin forward in
    /// Properties.
    pub clicked_pin: Option<ClickedPin>,
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
    /// The zoom pill's preset menu is up (Graphite mockup #64): opened by a click on the
    /// pill's percentage, closed by a pick, Esc or a click outside it.
    pub zoom_presets_open: bool,
}

impl InteractionState {
    /// Forget a sheet Tab pause once placement is no longer paused, so a
    /// later Esc never restores its stale values. A footprint pause stays.
    pub fn end_sheet_placement_pause(&mut self) {
        if matches!(
            self.placement_pause,
            Some(super::SheetPlacementPause::Sheet { .. })
        ) {
            self.placement_pause = None;
        }
    }

    pub fn active_canvas(&self) -> &CanvasSlot {
        &self.canvas
    }

    pub fn active_canvas_mut(&mut self) -> &mut CanvasSlot {
        &mut self.canvas
    }

    /// Per-window canvas lookup. Returns the per-window `CanvasSlot`
    /// if one is registered (undocked windows), otherwise the main
    /// window's shared canvas. Writes from canvas events still go
    /// through the main-canvas slot; see the dispatch swap trick in
    /// `dispatch::ui::handle_canvas_event_in_window`.
    pub fn canvas_for_window(&self, window_id: iced::window::Id) -> &CanvasSlot {
        self.canvases.get(&window_id).unwrap_or(&self.canvas)
    }

    /// Per-window PCB canvas lookup, falling back to the main window's.
    pub fn pcb_canvas_for_window(&self, window_id: iced::window::Id) -> &PcbCanvas {
        self.pcb_canvases
            .get(&window_id)
            .unwrap_or(&self.pcb_canvas)
    }

    /// The main schematic canvas plus every undocked window's own slot, so
    /// an app-wide appearance change cannot leave a detached window stale.
    pub fn all_schematic_canvases_mut(&mut self) -> impl Iterator<Item = &mut CanvasSlot> {
        std::iter::once(&mut self.canvas)
            .chain(self.canvases.values_mut())
            .chain(self.tile_panes.schematic.values_mut())
    }

    /// Every PCB canvas, main window first. App-wide settings (theme, grid,
    /// the GPU-render flag) go to all of them.
    pub fn all_pcb_canvases_mut(&mut self) -> impl Iterator<Item = &mut PcbCanvas> {
        std::iter::once(&mut self.pcb_canvas)
            .chain(self.pcb_canvases.values_mut())
            .chain(self.tile_panes.board.values_mut())
    }

    pub fn canvas_for_window_mut(&mut self, window_id: iced::window::Id) -> &mut CanvasSlot {
        // `get_mut` returns `Option<&mut V>`. Match rather than
        // `contains_key` + `get_mut().unwrap()` to avoid the double
        // lookup and the unwrap.
        match self.canvases.get_mut(&window_id) {
            Some(canvas) => canvas,
            None => &mut self.canvas,
        }
    }
}

/// A pin of a placed symbol that a click landed on. The symbol uuid alone
/// does not name it: one child sheet opened through two placements has the
/// same uuids in both tabs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClickedPin {
    pub sheet: std::path::PathBuf,
    pub placement: Option<signex_types::schematic::InstancePath>,
    pub symbol: uuid::Uuid,
    pub pin: String,
}

/// Canvases of the documents in the main window's unfocused tile panes
/// (#2249), by tab path. The focused pane is the active tab and draws on the
/// main canvases; see `handlers::document_tiling`.
#[derive(Default)]
pub struct TilePaneCanvases {
    pub schematic: std::collections::HashMap<std::path::PathBuf, CanvasSlot>,
    pub board: std::collections::HashMap<std::path::PathBuf, PcbCanvas>,
}
