//! Shared panel view-context (`PanelContext`) rendered by the panels.

use super::*;

pub struct PanelContext {
    /// Multi-project workspace — one entry per `LoadedProject`. Every
    /// project-aware panel reads from this Vec; the active project is
    /// the one with `is_active == true`.
    pub projects: Vec<ProjectPanelInfo>,
    pub sym_count: usize,
    pub wire_count: usize,
    pub label_count: usize,
    pub junction_count: usize,
    pub child_sheets: Vec<String>,
    pub has_schematic: bool,
    pub has_pcb: bool,
    pub paper_size: String,
    pub lib_symbol_count: usize,
    /// Library symbol names for Components panel.
    pub lib_symbol_names: Vec<String>,
    /// Placed symbols: (reference, value, footprint, lib_id).
    pub placed_symbols: Vec<(String, String, String, String)>,
    pub tokens: ThemeTokens,
    /// Active theme id. Feeds the icon registry so dock/panel SVGs tint
    /// to the theme's accent (see `crate::icons`).
    pub theme_id: signex_types::theme::ThemeId,
    // Live settings (synced from Signex on every update)
    pub unit: Unit,
    pub grid_visible: bool,
    pub snap_enabled: bool,
    pub grid_size_mm: f32,
    pub visible_grid_mm: f32,
    pub snap_hotspots: bool,
    /// UI font family name (shown in settings; applies on restart).
    pub ui_font_name: String,
    /// User-editable component-class registry mirrored from
    /// `UiState.component_classes`. Read by the New Component modal's
    /// class dropdown so the picker reflects whatever the user has
    /// added / renamed / removed in Preferences ▸ Component Classes.
    pub component_classes: Vec<crate::fonts::ComponentClassEntry>,
    /// Canvas font family name (Iosevka by default; applies immediately).
    pub canvas_font_name: String,
    /// Canvas font size in px (applied immediately as a global scale).
    pub canvas_font_size: f32,
    /// Canvas font bold style flag.
    pub canvas_font_bold: bool,
    /// Canvas font italic style flag.
    pub canvas_font_italic: bool,
    /// Whether the canvas font picker popup is open.
    pub canvas_font_popup_open: bool,
    pub properties_tab: usize, // 0=General, 1=Parameters
    // Components panel
    pub standard_libraries: Vec<String>,
    pub active_library: Option<String>,
    /// Browser entries from the selected library or aggregated catalog.
    pub library_symbols: Vec<LibrarySymbolEntry>,
    /// Selected component lib_id.
    pub selected_component: Option<String>,
    /// (pin_number, pin_name, pin_type) for the selected component.
    pub selected_pins: Vec<(String, String, String)>,
    /// Full LibSymbol data for canvas preview.
    pub selected_lib_symbol: Option<signex_types::schematic::LibSymbol>,
    /// Height in px for the Components list section (details gets the rest).
    pub components_split: f32,
    /// Persistent project tree — toggle state survives across renders.
    pub project_tree: Vec<TreeNode>,
    /// Path of the currently single-clicked tree row, used to drive
    /// the row highlight independently of which document is active.
    /// Set on `TreeMsg::Select`, cleared when the tree resets.
    pub project_tree_selected: Option<Vec<usize>>,
    /// F15 — detail for the row currently selected in the active
    /// Library Browser tab. `Some` when (a) the active tab is a
    /// `LibraryBrowser(path)` AND (b) `library_browsers[path]
    /// .selected_row` is `Some`. `view_properties` reads this and
    /// renders the row's metadata + Pick Symbol / Pick Footprint
    /// buttons; outside of a Library Browser tab the field stays
    /// `None` and the panel falls through to its existing branches.
    pub library_row_detail: Option<LibraryRowDetail>,
    // Selection info for Properties panel
    /// How many items are currently selected.
    pub selection_count: usize,
    /// UUID of the single selected item (for property editing).
    pub selected_uuid: Option<uuid::Uuid>,
    /// Kind of the single selected item.
    pub selected_kind: Option<signex_types::schematic::SelectedKind>,
    /// Description of the selected item (for single selection).
    pub selection_info: Vec<(String, String)>,
    /// Transient numeric-input buffers for the drawing properties
    /// panel — keyed on DrawingFieldId. Keeps half-typed strings
    /// ("", "-", "2.") alive between rerenders so fields can be
    /// fully erased and retyped. Reset when selection changes.
    pub drawing_edit_buf: std::collections::HashMap<DrawingFieldId, String>,
    /// UUID that owns the current drawing_edit_buf. When
    /// `selected_uuid` changes the handler clears the buffer.
    pub drawing_edit_buf_for: Option<uuid::Uuid>,
    /// The live SchDrawing matching `selected_uuid` when a single
    /// drawing is selected. Feeds the mini preview canvas in the
    /// Properties panel so the shape renders true-to-life.
    pub selected_drawing: Option<signex_types::schematic::SchDrawing>,
    /// The live ChildSheet matching `selected_uuid` when a single
    /// hierarchical sheet is selected. Powers the editable
    /// border/fill colour pickers and stroke-width input.
    pub selected_child_sheet: Option<signex_types::schematic::ChildSheet>,
    /// Whether the border-colour picker overlay is open for the
    /// currently-selected child sheet.
    pub child_sheet_border_picker_open: bool,
    /// Whether the fill-colour picker overlay is open for the
    /// currently-selected child sheet.
    pub child_sheet_fill_picker_open: bool,
    /// Whether the user expanded the border picker into the full
    /// HSV / RGB ColorPicker (vs the default preset palette).
    pub child_sheet_border_advanced_open: bool,
    /// Whether the user expanded the fill picker into the full
    /// HSV / RGB ColorPicker (vs the default preset palette).
    pub child_sheet_fill_advanced_open: bool,
    /// Transient text-input buffer for the child sheet's stroke-width
    /// numeric field. `None` means "show the live value formatted";
    /// `Some(s)` keeps half-typed strings between rerenders.
    pub child_sheet_stroke_width_buf: Option<String>,
    /// Component search filter text.
    pub component_filter: String,
    /// Which sections are collapsed (by section name key).
    pub collapsed_sections: CollapsedSections,
    /// Pre-placement configuration (shown when Tab pressed during placement tool).
    pub pre_placement: Option<PrePlacementData>,
    /// Current diagnostics level resolved from SIGNEX_LOG / RUST_LOG.
    /// Flattened ERC diagnostics from the most recent Run-ERC pass.
    pub erc_diagnostics: Vec<ErcDiagnosticEntry>,
    /// Focused ERC diagnostic index used by prev/next navigation arrows.
    pub erc_focus_index: Option<usize>,
    pub diagnostics_level: String,
    /// Recent application diagnostics shown in the Messages panel.
    pub diagnostics: Vec<crate::diagnostics::DiagnosticEntry>,
    /// Selection filter state, shared with the Active Bar.
    pub selection_filters: std::collections::HashSet<crate::active_bar::SelectionFilter>,
    /// User-defined custom filter presets (capped at
    /// `crate::active_bar::CUSTOM_FILTER_PRESET_LIMIT`). Edited from
    /// the Properties panel; surfaced as shortcut buttons in the Active
    /// Bar's filter dropdown.
    pub custom_filter_presets: Vec<crate::active_bar::CustomFilterPreset>,
    /// Index of the active preset tab in the Properties-panel editor
    /// (mirrored from `InteractionState`). Clamped on every sync.
    pub active_custom_filter_tab: usize,
    /// User-defined footprint-editor filter presets, mirrored from
    /// `InteractionState::footprint_filter_presets` (Task 6). Parallel
    /// to `custom_filter_presets` but keyed on `SelectionFilterKind`.
    pub footprint_filter_presets: Vec<crate::active_bar::FootprintFilterPreset>,
    /// Page formatting mode (Template / Standard / Custom).
    pub page_format_mode: PageFormatMode,
    /// Vertical page margin zones.
    pub margin_vertical: u32,
    /// Horizontal page margin zones.
    pub margin_horizontal: u32,
    /// Page coordinate origin.
    pub page_origin: PageOrigin,
    /// Custom paper width in mm (only used when page_format_mode == Custom).
    pub custom_paper_w_mm: f32,
    /// Custom paper height in mm (only used when page_format_mode == Custom).
    pub custom_paper_h_mm: f32,
    /// Sheet background colour.
    pub sheet_color: SheetColor,
    /// When the active tab is a `.snxsym` standalone editor, this carries
    /// the symbol's display data so the right-dock Properties panel and
    /// the left-dock SCH-Library panel can render context-aware content
    /// without the in-tab editor having to embed its own properties pane.
    /// `None` for any other tab kind.
    pub symbol_editor: Option<SymbolEditorPanelContext>,
    /// v0.14.2 — context for the right-dock Properties panel when the
    /// active tab is a `.snxfpt` standalone footprint editor.
    /// Surfaces editor mode (Pads / Sketch / 3D View) + the current
    /// selection so the panel can switch its body between pad
    /// properties, sketch entity properties, or footprint defaults.
    /// `None` for any other tab kind.
    pub footprint_editor: Option<FootprintEditorPanelContext>,
    /// Per-file Git history snapshot for the right-dock History
    /// panel. Mirrored from [`crate::app::DocumentState::history`]
    /// each refresh; the panel reads it directly without holding a
    /// borrow into the document state.
    pub history: history::HistoryPanelState,
}
