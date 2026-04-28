//! Panel implementations — uses signex-widgets for proper Altium-style content.

use iced::mouse;
use iced::widget::canvas;
use iced::widget::{Column, Row, Space, column, container, row, scrollable, svg, text};
use iced::{Background, Border, Color, Element, Length, Point, Rectangle, Renderer, Theme};
use iced_aw::{NumberInput, Wrap};
use signex_types::coord::Unit;
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;
use signex_widgets::tree_view::{TreeIcon, TreeMsg, TreeNode, TreeView};
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(dead_code)]
pub enum PanelKind {
    Projects,
    Components,
    Navigator,
    Properties,
    Filter,
    Erc,
    Messages,
    Signal,
    Drc,
    LayerStack,
    NetClasses,
    Variants,
    OutputJobs,
    // Additional Altium schematic panels
    SchFilter,
    SchList,
    BomStudio,
    Favorites,
    Snippets,
    Todo,
    Wiki,
    /// v0.9 Library panel — open `*.snxlib/` libraries, drill into
    /// their components, drag onto the canvas (Phase 2).
    Library,
    /// v0.9 phase 2.5 — Altium SCH Library panel. Lists symbols
    /// inside the active `.snxsym` container; clicking switches the
    /// editor's `active_idx`. Visible whenever a Symbol editor tab
    /// is focused.
    SchLibrary,
}

/// All available panel kinds for the panel list button.
pub const ALL_PANELS: &[PanelKind] = &[
    PanelKind::Projects,
    PanelKind::Components,
    PanelKind::Library,
    PanelKind::Navigator,
    PanelKind::Properties,
    PanelKind::Filter,
    PanelKind::SchFilter,
    PanelKind::SchList,
    PanelKind::Erc,
    PanelKind::Messages,
    PanelKind::Signal,
    PanelKind::Drc,
    PanelKind::BomStudio,
    PanelKind::Favorites,
    PanelKind::Snippets,
    PanelKind::LayerStack,
    PanelKind::NetClasses,
    PanelKind::Variants,
    PanelKind::OutputJobs,
    PanelKind::Todo,
    PanelKind::Wiki,
    PanelKind::SchLibrary,
];

impl PanelKind {
    /// Whether this panel requires a schematic document to be open.
    pub fn needs_schematic(self) -> bool {
        matches!(
            self,
            PanelKind::Navigator
                | PanelKind::Properties
                | PanelKind::Filter
                | PanelKind::Erc
                | PanelKind::SchFilter
                | PanelKind::SchList
                | PanelKind::Signal
                | PanelKind::Drc
                | PanelKind::BomStudio
                | PanelKind::Snippets
                | PanelKind::Variants
                | PanelKind::OutputJobs
        )
    }

    /// Whether this panel requires a PCB document to be open.
    pub fn needs_pcb(self) -> bool {
        matches!(self, PanelKind::LayerStack | PanelKind::NetClasses)
    }

    pub fn label(self) -> &'static str {
        match self {
            PanelKind::Projects => "Projects",
            PanelKind::Components => "Components",
            PanelKind::Navigator => "Navigator",
            PanelKind::Properties => "Properties",
            PanelKind::Filter => "Filter",
            PanelKind::Erc => "ERC",
            PanelKind::Messages => "Messages",
            PanelKind::Signal => "Signal",
            PanelKind::Drc => "DRC",
            PanelKind::LayerStack => "Layer Stack",
            PanelKind::NetClasses => "Net Classes",
            PanelKind::Variants => "Variants",
            PanelKind::SchFilter => "SCH Filter",
            PanelKind::SchList => "SCH List",
            PanelKind::BomStudio => "BOM Studio",
            PanelKind::Favorites => "Favorites",
            PanelKind::Snippets => "Snippets",
            PanelKind::Todo => "To-Do",
            PanelKind::Wiki => "Wiki",
            PanelKind::OutputJobs => "Output Jobs",
            PanelKind::Library => "Library",
            PanelKind::SchLibrary => "SCH Library",
        }
    }
}

/// Track which sections are collapsed (by section name).
pub type CollapsedSections = std::collections::HashSet<String>;

/// Per-sheet info for the project tree.
#[derive(Debug, Clone)]
pub struct SheetInfo {
    #[allow(dead_code)]
    pub name: String,
    pub filename: String,
    pub sym_count: usize,
    #[allow(dead_code)]
    pub wire_count: usize,
    #[allow(dead_code)]
    pub label_count: usize,
    /// True when this sheet is currently in `document_state.tabs`.
    /// Drives the small accent-coloured dot on the tree row (Altium parity).
    pub is_open: bool,
    /// True when the open tab for this sheet has unsaved edits.
    /// Drives the bright red dot on the tree row.
    pub is_dirty: bool,
    /// True when this sheet is the document the user is currently
    /// viewing (`document_state.tabs[active_tab].path == sheet path`).
    /// Drives the highlighted row background — Altium parity.
    pub is_active: bool,
}

/// Per-project bundle surfaced to the Projects panel. One entry per
/// `LoadedProject` in `DocumentState.projects`. `build_project_tree`
/// iterates this list to emit one tree root per project.
#[derive(Debug, Clone)]
pub struct ProjectPanelInfo {
    pub id: crate::app::ProjectId,
    /// Display name (project stem — "MyBoard" from "MyBoard.standard_pro").
    pub name: String,
    /// Root schematic filename shown as the "project file" under each
    /// root, when present.
    pub project_file: Option<String>,
    /// Open / dirty state for the root schematic, mirrors the same
    /// flags that `SheetInfo` carries for inner sheets.
    pub project_file_open: bool,
    pub project_file_dirty: bool,
    pub project_file_active: bool,
    /// Companion PCB filename, when present.
    pub pcb_file: Option<String>,
    pub pcb_file_open: bool,
    pub pcb_file_dirty: bool,
    pub pcb_file_active: bool,
    pub sheets: Vec<SheetInfo>,
    /// Component libraries attached to this project. One entry per
    /// `Project::libraries[]`. Drives the `Libraries` branch under
    /// the project root — each entry renders as a `*.snxlib` leaf
    /// and (when the library is mounted) a small list of cached
    /// components beneath it.
    pub libraries: Vec<LibraryNodeInfo>,
    /// Whether this is the currently-active project — drives accent
    /// styling on the root node.
    pub is_active: bool,
}

/// Per-library bundle for the project tree's `Libraries` group.
/// Mirrors what [`signex_types::project::LibraryEntry`] records on
/// the project, plus a couple of cached fields the panel pulls from
/// `LibraryState` so the view doesn't have to re-borrow the library
/// crate at render time.
///
/// The library renders as a single leaf in the project tree under
/// the v0.9 `.snxlib`-as-file model — symbols / footprints / sims
/// are not surfaced here. Browsing the library's contents is the
/// Library Browser tab's job; double-clicking the leaf opens it.
#[derive(Debug, Clone)]
pub struct LibraryNodeInfo {
    /// Display name for the row — `<entry.path>.file_name()` or the
    /// manifest name when the library is mounted.
    pub display_name: String,
    /// Absolute on-disk path of the `.snxlib` file — feeds the
    /// right-click menu (Add New ▸ Component pre-selects this path)
    /// and the double-click → Library Browser open dispatch.
    pub root: std::path::PathBuf,
    /// True when the library is currently mounted in
    /// `LibraryState::open_libraries`. Drives the icon tint —
    /// unmounted entries render in the muted "missing" colour.
    pub mounted: bool,
}

/// Context passed to panels — owned data to avoid lifetime issues.
#[derive(Debug, Clone)]
pub struct LibrarySymbolEntry {
    pub lib_id: String,
    pub symbol_name: String,
    pub library_name: String,
    pub pin_count: usize,
}

/// Flattened ERC diagnostic row for the ERC panel.
///
/// The app flattens per-sheet ERC caches into this list so the panel can
/// present one navigable table across the entire project.
#[derive(Debug, Clone)]
pub struct ErcDiagnosticEntry {
    pub global_index: usize,
    pub sheet_name: String,
    pub sheet_path: std::path::PathBuf,
    pub severity: ErcSeverityLite,
    pub rule_label: &'static str,
    pub message: String,
    pub world_x: f64,
    pub world_y: f64,
    pub select: Option<signex_types::schematic::SelectedItem>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErcSeverityLite {
    Error,
    Warning,
    Info,
}

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
}

/// Context handed to the right-dock Properties panel and the SCH-Library
/// left-dock panel when the active tab is a `.snxsym` standalone editor.
/// Mirrors the live `SymbolEditorState` but contains only the fields the
/// panels render — keeps `panel_ctx` cloneable and decouples panel code
/// from the canvas state struct.
#[derive(Debug, Clone)]
pub struct SymbolEditorPanelContext {
    /// File path of the open `.snxsym` (used as the tab key).
    pub path: std::path::PathBuf,
    /// The active symbol's `name` field (Altium "Design Item ID").
    pub symbol_name: String,
    /// Altium "Designator" (e.g. `U?`).
    pub symbol_designator: String,
    /// Altium "Comment" (e.g. `*` or a fixed value).
    pub symbol_comment: String,
    /// Free-text description.
    pub symbol_description: String,
    /// Altium "Component Type" — `Standard / Mechanical / Graphical
    /// / Net Tie / Standard (No BOM) / Jumper`.
    pub symbol_component_type: signex_library::ComponentType,
    /// Altium "Mirrored" toggle.
    pub symbol_mirrored: bool,
    /// Optional per-symbol Local Colors override (Fills / Lines /
    /// Pins). `None` = inherit from sheet palette.
    pub symbol_local_fill_color: Option<[u8; 4]>,
    pub symbol_local_line_color: Option<[u8; 4]>,
    pub symbol_local_pin_color: Option<[u8; 4]>,
    /// UUID of the active symbol — surfaced read-only on the panel.
    pub symbol_uuid: uuid::Uuid,
    /// Pin summaries for the active symbol — drives Properties panel
    /// pin selection.
    pub pins: Vec<SymbolPinSummary>,
    /// Graphic summaries for the active symbol — drives the SCH
    /// Library Graphics sub-list (click to select, populates the
    /// Properties panel).
    pub graphics: Vec<GraphicSummary>,
    /// What's currently selected on the canvas. Drives the right-dock
    /// Properties panel's mode (Pin / Field / Component default).
    pub selected: SymbolEditorSelection,
    /// All symbols in the open `.snxsym` container — feeds the SCH
    /// Library left-dock panel's components list.
    pub symbols_in_file: Vec<SymbolFileEntry>,
    /// Index into `symbols_in_file` for the symbol currently being
    /// edited. The SCH Library panel's row click rewrites this.
    pub active_idx: usize,
    /// Active sub-part of the currently-edited symbol — surfaced on
    /// the in-tab toolbar's `Part X / N` picker and the SCH Library
    /// panel's part tree-expander.
    pub active_part: u8,
    /// Highest declared `part_number` across the active symbol's
    /// pins (`1` for single-part components). Drives the tree-
    /// expander row count under the active symbol and clamps the
    /// in-tab toolbar's right arrow.
    pub active_max_part: u8,
    /// Whether any pin on the active symbol carries `part_number == 0`
    /// (Altium "Part Zero" — shared across every part). Drives the
    /// optional "Part 0 (shared)" tree-expander row.
    pub active_has_part_zero: bool,
    /// Per-`.snxlib` display settings (sheet color, grid, unit) —
    /// rendered on the Properties panel's "None" branch as Altium
    /// Document Options. Resolved by `runtime.rs` from the
    /// containing library; falls back to defaults for lone-file
    /// edits.
    pub display: SymbolDisplayOptions,
}

/// Sheet / grid / unit + library identity surfaced on the
/// Properties panel as Altium "Document Options" when nothing is
/// selected on the symbol canvas. Mirrors
/// [`crate::library::state::LibraryDisplaySettings`] but lives in
/// the panels crate so view code doesn't pull
/// `crate::library::state` directly.
#[derive(Debug, Clone, PartialEq)]
pub struct SymbolDisplayOptions {
    pub sheet_color: SheetColor,
    pub grid_visible: bool,
    pub grid_size_mm: f32,
    pub unit: signex_types::coord::Unit,
    /// Display name of the containing `.snxlib` (or the file stem
    /// when the symbol lives outside any mounted library).
    pub library_name: String,
    /// Total number of symbols across every `.snxsym` in the
    /// containing library — surfaced as "Symbols in library: N".
    /// `None` for lone-file edits.
    pub library_symbol_count: Option<usize>,
}

impl Default for SymbolDisplayOptions {
    fn default() -> Self {
        Self {
            sheet_color: SheetColor::default(),
            grid_visible: true,
            grid_size_mm: 2.54,
            unit: signex_types::coord::Unit::Mm,
            library_name: "(lone file)".to_string(),
            library_symbol_count: None,
        }
    }
}

/// One symbol's row entry in the SCH Library panel's components list.
#[derive(Debug, Clone, PartialEq)]
pub struct SymbolFileEntry {
    pub idx: usize,
    pub name: String,
    pub uuid: uuid::Uuid,
    pub pin_count: usize,
    /// Free-text description — surfaced as the second tree column.
    pub description: String,
}

/// Extended pin fields surfaced on the Properties panel — flows
/// alongside [`SymbolPinSummary`] so the panel doesn't have to
/// reach back into the editor state to read them. Mirrors the
/// Altium SchLib Pin Properties layout.
#[derive(Debug, Clone, PartialEq)]
pub struct SymbolPinDetails {
    pub description: String,
    pub function: Vec<String>,
    pub pin_package_length: Option<f64>,
    pub propagation_delay_ns: Option<f64>,
    pub designator_visible: bool,
    pub name_visible: bool,
    pub inside_symbol: signex_library::PinSymbolKind,
    pub inside_edge_symbol: signex_library::PinSymbolKind,
    pub outside_edge_symbol: signex_library::PinSymbolKind,
    pub outside_symbol: signex_library::PinSymbolKind,
    pub hidden: bool,
    pub locked: bool,
    /// Multi-part scoping: 1 = single-part default, 0 = "Part Zero"
    /// (pin appears on every part), 2..N = scoped to that part.
    pub part_number: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SymbolPinSummary {
    pub idx: usize,
    pub number: String,
    pub name: String,
    pub electrical: String,
    pub position: [f64; 2],
    pub orientation: String,
    pub length: f64,
    /// Extended fields — surfaced on the Properties panel only when
    /// the pin is selected. Kept in a sub-struct so the SCH Library
    /// Pins sub-list (which only needs number/name/electrical) can
    /// stay terse.
    pub details: SymbolPinDetails,
}

/// What the canvas currently has selected — drives Properties panel
/// content. `None` = nothing selected, panel shows the symbol's defaults.
#[derive(Debug, Clone, PartialEq)]
pub enum SymbolEditorSelection {
    None,
    Pin(SymbolPinSummary),
    FieldReference,
    FieldValue,
    /// A placed graphic — drives the per-shape Properties branch
    /// (corners / endpoints / centre + radius / arc angles / text +
    /// stroke).
    Graphic(GraphicSummary),
}

/// Per-shape Properties-panel summary for a placed `SymbolGraphic`.
/// Cloned out of the live `Symbol::graphics` vector each refresh so
/// the panel doesn't hold a borrow into the editor state.
#[derive(Debug, Clone, PartialEq)]
pub struct GraphicSummary {
    pub idx: usize,
    pub kind: GraphicKindSummary,
    pub stroke_width: f64,
}

/// Per-variant geometry for [`GraphicSummary`] — mirrors
/// `signex_library::SymbolGraphicKind` so the panel can render each
/// shape's editable fields without depending on the library type.
#[derive(Debug, Clone, PartialEq)]
pub enum GraphicKindSummary {
    Rectangle {
        from: [f64; 2],
        to: [f64; 2],
    },
    Line {
        from: [f64; 2],
        to: [f64; 2],
    },
    Circle {
        center: [f64; 2],
        radius: f64,
    },
    Arc {
        center: [f64; 2],
        radius: f64,
        start_deg: f64,
        end_deg: f64,
    },
    Text {
        position: [f64; 2],
        content: String,
        size: f64,
    },
}

/// Identifier for one numeric field on a graphic — carried by
/// [`PanelMsg::SymEditorSetGraphicField`] so the dispatcher knows which
/// scalar to mutate. The dispatcher silently ignores (idx, field)
/// pairs whose field doesn't apply to the graphic's kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GraphicFieldId {
    /// `Rectangle`/`Line` `from.x`.
    FromX,
    /// `Rectangle`/`Line` `from.y`.
    FromY,
    /// `Rectangle`/`Line` `to.x`.
    ToX,
    /// `Rectangle`/`Line` `to.y`.
    ToY,
    /// `Circle`/`Arc` `center.x`.
    CenterX,
    /// `Circle`/`Arc` `center.y`.
    CenterY,
    /// `Circle`/`Arc` `radius`.
    Radius,
    /// `Arc` `start_deg`.
    StartDeg,
    /// `Arc` `end_deg`.
    EndDeg,
    /// `Text` `position.x`.
    PositionX,
    /// `Text` `position.y`.
    PositionY,
    /// `Text` `size`.
    TextSize,
    /// All variants — `SymbolGraphic.stroke_width`.
    StrokeWidth,
}

/// Sheet background colour presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SheetColor {
    #[default]
    Black,
    White,
    DarkGray,
    LightGray,
    Cream,
}

impl SheetColor {
    pub fn to_color(self) -> iced::Color {
        match self {
            SheetColor::Black => iced::Color::from_rgb8(0x14, 0x14, 0x14),
            SheetColor::White => iced::Color::WHITE,
            SheetColor::DarkGray => iced::Color::from_rgb8(0x2A, 0x2A, 0x2A),
            SheetColor::LightGray => iced::Color::from_rgb8(0xD0, 0xD0, 0xD0),
            SheetColor::Cream => iced::Color::from_rgb8(0xFB, 0xF4, 0xE0),
        }
    }
    pub const ALL: &'static [SheetColor] = &[
        SheetColor::Black,
        SheetColor::White,
        SheetColor::DarkGray,
        SheetColor::LightGray,
        SheetColor::Cream,
    ];
}

impl std::fmt::Display for SheetColor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            SheetColor::Black => "Black",
            SheetColor::White => "White",
            SheetColor::DarkGray => "Dark Gray",
            SheetColor::LightGray => "Light Gray",
            SheetColor::Cream => "Cream",
        })
    }
}

/// Altium-style page formatting mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PageFormatMode {
    Template,
    #[default]
    Standard,
    Custom,
}

/// Altium-style page coordinate origin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PageOrigin {
    UpperLeft,
    #[default]
    LowerLeft,
}

impl std::fmt::Display for PageOrigin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            PageOrigin::UpperLeft => "Upper Left",
            PageOrigin::LowerLeft => "Lower Left",
        })
    }
}

/// Supported paper sizes (Altium-compatible subset).
pub const PAPER_SIZES: &[&str] = &[
    "A0", "A1", "A2", "A3", "A4", "A5", "B5", "Letter", "Legal", "Tabloid",
];

/// (width_mm, height_mm) for a paper size string.
pub fn paper_dimensions(size: &str) -> (f32, f32) {
    match size {
        "A0" => (1189.0, 841.0),
        "A1" => (841.0, 594.0),
        "A2" => (594.0, 420.0),
        "A3" => (420.0, 297.0),
        "A4" => (297.0, 210.0),
        "A5" => (210.0, 148.0),
        "B5" => (257.0, 182.0),
        "Letter" => (279.4, 215.9),
        "Legal" => (355.6, 215.9),
        "Tabloid" => (431.8, 279.4),
        _ => (297.0, 210.0),
    }
}

/// Pre-placement configuration data — shown in Properties panel when Tab pressed.
#[derive(Debug, Clone)]
pub struct PrePlacementData {
    /// Which tool is being configured.
    pub tool_name: String,
    /// Semantic kind so the panel can render the right field set.
    pub kind: PrePlacementKind,
    /// Net label / text note text.
    pub label_text: String,
    /// Component designator override.
    pub designator: String,
    /// Rotation (degrees).
    pub rotation: f64,
    /// Font family (cosmetic until font switching ships).
    pub font: String,
    /// Font size in points (10 pt = Altium default).
    pub font_size_pt: u32,
    /// Horizontal justification.
    pub justify_h: signex_types::schematic::HAlign,
    /// Vertical justification (TextNote / Component fields).
    pub justify_v: signex_types::schematic::VAlign,
    /// Style toggles (currently cosmetic — engine wiring tracks v0.7+).
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    /// Most-recent cursor world position (for the X/Y readout).
    pub cursor_x_mm: f64,
    pub cursor_y_mm: f64,
    /// Stroke width for the shape tools (Line / Rect / Circle / Arc /
    /// Polygon). 0 = Standard default ≈ 0.15 mm.
    pub shape_width_mm: f64,
    /// Fill style for shapes that support it (Rect / Circle / Polygon).
    pub shape_fill: signex_types::schematic::FillType,
}

/// Stable identifiers for every numeric drawing-field editor so the
/// panel keeps a transient string buffer per field across rerenders.
/// Erasing a text_input leaves an empty string in the buffer until
/// the user types a valid f64, at which point UpdateDrawingEdit fires.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DrawingFieldId {
    LineStartX,
    LineStartY,
    LineEndX,
    LineEndY,
    LineWidth,
    RectStartX,
    RectStartY,
    RectWidth,
    RectHeight,
    RectBorder,
    CircleCenterX,
    CircleCenterY,
    CircleRadius,
    CircleBorder,
    ArcCenterX,
    ArcCenterY,
    ArcRadius,
    ArcStartAngle,
    ArcEndAngle,
    ArcWidth,
    PolyBorder,
}

/// Distinguishes placement flavors so the pre-placement form only shows
/// fields relevant to what the user is about to drop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrePlacementKind {
    Wire,
    Bus,
    BusEntry,
    NoConnect,
    NetLabel,
    GlobalPort,
    HierPort,
    PowerPort,
    TextNote,
    Component,
    Line,
    Rectangle,
    Circle,
    Arc,
    Polygon,
    Other,
}

/// Panel-level message wrapping widget messages.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum PanelMsg {
    Tree(TreeMsg),
    SetUnit(Unit),
    RunErc,
    /// Clear the current ERC violations list and canvas markers.
    ClearErc,
    /// Focus a specific ERC diagnostic row from the global flattened list.
    FocusErcViolation(usize),
    /// Focus previous ERC diagnostic row in the global list.
    FocusPrevErcDiagnostic,
    /// Focus next ERC diagnostic row in the global list.
    FocusNextErcDiagnostic,
    ToggleGrid,
    ToggleSnap,
    PropertiesTab(usize),
    SelectLibrary(String),
    SelectComponent(String),
    DragComponentsSplit,
    ComponentFilter(String),
    /// Toggle a collapsible section (by section key).
    ToggleSection(String),
    /// Edit a symbol's designator (committed on submit).
    EditSymbolDesignator(uuid::Uuid, String),
    /// Edit a symbol's value (committed on submit).
    EditSymbolValue(uuid::Uuid, String),
    /// Edit a symbol's footprint (committed on submit).
    EditSymbolFootprint(uuid::Uuid, String),
    /// Toggle a symbol's mirror_x.
    ToggleSymbolMirrorX(uuid::Uuid),
    /// Toggle a symbol's mirror_y.
    ToggleSymbolMirrorY(uuid::Uuid),
    /// Toggle a symbol's locked state.
    ToggleSymbolLocked(uuid::Uuid),
    /// Toggle a symbol's DNP state.
    ToggleSymbolDnp(uuid::Uuid),
    /// Set absolute rotation on a symbol (degrees).
    EditSymbolRotation(uuid::Uuid, f64),
    /// Set font size (Altium pt) on a symbol's value text property.
    EditSymbolValueFontSizePt(uuid::Uuid, u32),
    /// Change the lib_id of a symbol (used by power-port Style dropdown).
    EditSymbolLibId(uuid::Uuid, String),
    /// Swap a power-port's style: change lib_id and preserve visual direction
    /// by setting rotation accordingly.
    EditPowerPortStyle {
        symbol_id: uuid::Uuid,
        new_lib_id: String,
        rotation_degrees: f64,
    },
    /// Edit a label's text (committed on submit).
    EditLabelText(uuid::Uuid, String),
    /// Edit a label's horizontal justification.
    EditLabelJustifyH(uuid::Uuid, signex_types::schematic::HAlign),
    /// Edit a label direction preset (rotation + horizontal justify).
    EditLabelDirection(uuid::Uuid, f64, signex_types::schematic::HAlign),
    /// Edit a label's rotation (degrees).
    EditLabelRotation(uuid::Uuid, f64),
    /// Edit a label's font size in Altium pt (10 = 2.54 mm).
    EditLabelFontSizePt(uuid::Uuid, u32),
    /// Edit a text note's text (committed on submit).
    EditTextNoteText(uuid::Uuid, String),
    /// Pre-placement: update label/text field.
    SetPrePlacementText(String),
    /// Pre-placement: update designator field.
    SetPrePlacementDesignator(String),
    /// Pre-placement: update rotation.
    SetPrePlacementRotation(f64),
    /// Pre-placement: update font family.
    SetPrePlacementFont(String),
    /// Pre-placement: update font size (pt).
    SetPrePlacementFontSize(u32),
    /// Pre-placement: set horizontal justification.
    SetPrePlacementJustifyH(signex_types::schematic::HAlign),
    /// Pre-placement: set vertical justification.
    SetPrePlacementJustifyV(signex_types::schematic::VAlign),
    /// Pre-placement: toggle bold / italic / underline.
    TogglePrePlacementBold,
    TogglePrePlacementItalic,
    TogglePrePlacementUnderline,
    SetPrePlacementShapeWidth(f64),
    SetPrePlacementShapeFill(signex_types::schematic::FillType),
    /// Properties panel — edit a pin's designator (number) on the
    /// active Symbol editor tab. Routed through `handle_dock_sch_library_message`
    /// because the symbol editor's lifecycle owns the pin edits.
    SymEditorSetPinNumber {
        pin_idx: usize,
        value: String,
    },
    /// Properties panel — edit a pin's display name.
    SymEditorSetPinName {
        pin_idx: usize,
        value: String,
    },
    /// Properties panel — edit a pin's stub length in mm.
    SymEditorSetPinLength {
        pin_idx: usize,
        value: f64,
    },
    /// Properties panel — set a pin's electrical type from the
    /// Altium-spec dropdown (Input / Output / Bidirectional / Power
    /// / Passive / Open Collector / Open Emitter / Tri-state /
    /// Not Connected / Unspecified).
    SymEditorSetPinElectrical {
        pin_idx: usize,
        value: signex_library::PinElectricalType,
    },
    /// Properties panel — set a pin's orientation (Right / Up /
    /// Left / Down). Also updates the canvas cache so the pin
    /// re-renders.
    SymEditorSetPinOrientation {
        pin_idx: usize,
        value: signex_library::PinOrientation,
    },
    /// Properties panel — set a pin's X coordinate in mm.
    SymEditorSetPinX {
        pin_idx: usize,
        value: f64,
    },
    /// Properties panel — set a pin's Y coordinate in mm.
    SymEditorSetPinY {
        pin_idx: usize,
        value: f64,
    },
    /// SCH Library panel — click on a row in the Pins sub-list.
    /// Selects the pin on the canvas (Properties panel switches
    /// to pin-mode automatically via the next refresh_panel_ctx).
    SymEditorSelectPin(usize),
    /// Properties panel — edit a pin's free-text Description.
    SymEditorSetPinDescription {
        pin_idx: usize,
        value: String,
    },
    /// Properties panel — edit a pin's Function list (alt-names) as
    /// a single comma-separated string. Persisted as `Vec<String>`
    /// after splitting + trimming on commit.
    SymEditorSetPinFunctionCsv {
        pin_idx: usize,
        value: String,
    },
    /// Properties panel — toggle a pin's designator visibility.
    SymEditorTogglePinDesignatorVisible(usize),
    /// Properties panel — toggle a pin's name visibility.
    SymEditorTogglePinNameVisible(usize),
    /// Properties panel — toggle Pin Hide.
    SymEditorTogglePinHidden(usize),
    /// Properties panel — toggle Pin Locked.
    SymEditorTogglePinLocked(usize),
    /// Properties panel — set one of the four IEEE-symbol slots on
    /// a pin. `slot` is 0=Inside, 1=InsideEdge, 2=OutsideEdge, 3=Outside.
    SymEditorSetPinSymbol {
        pin_idx: usize,
        slot: u8,
        value: signex_library::PinSymbolKind,
    },
    /// Properties panel — set a pin's multi-part scope (Altium
    /// "Part Number" spinner). `0` is the special Part Zero (pin
    /// appears on every part); `1..=N` scopes to a specific part.
    SymEditorSetPinPartNumber {
        pin_idx: usize,
        value: u8,
    },
    /// SCH Library panel: select a placed graphic from the active
    /// symbol's `graphics` vector. Fires the same selection state as
    /// a canvas click on a graphic body.
    SymEditorSelectGraphic(usize),
    /// SCH Library panel: switch the editor's `active_part` to the
    /// given value. `0` selects Part Zero (shared pins). The
    /// dispatcher clamps to `[0, active_max_part]` so a stale tree
    /// click can't park `active_part` outside the symbol's range.
    SymEditorSelectPart(u8),
    /// Properties panel — set one numeric field of the placed graphic
    /// at `idx`. The dispatcher routes `field` to the matching
    /// `SymbolGraphicKind` variant; mismatched (idx, field) pairs
    /// silently no-op so an out-of-date Properties panel can't
    /// corrupt geometry.
    SymEditorSetGraphicField {
        idx: usize,
        field: GraphicFieldId,
        value: f64,
    },
    /// Properties panel — set the text content of a placed
    /// `SymbolGraphicKind::Text` at `idx`. No-op for other kinds.
    SymEditorSetGraphicText {
        idx: usize,
        value: String,
    },
    /// Properties panel — edit the active symbol's name (Altium
    /// "Design Item ID"). Affects the SCH Library panel row label
    /// + the on-disk container's `display_name` when the active
    /// symbol is the only one in the file.
    SymEditorSetSymbolName(String),
    /// Properties panel — edit the active symbol's designator
    /// template (Altium "Designator", e.g. `U?`).
    SymEditorSetSymbolDesignator(String),
    /// Properties panel — edit the active symbol's comment
    /// passthrough.
    SymEditorSetSymbolComment(String),
    /// Properties panel — edit the active symbol's free-text
    /// description.
    SymEditorSetSymbolDescription(String),
    /// Properties panel — pick the active symbol's Component Type.
    SymEditorSetSymbolType(signex_library::ComponentType),
    /// Properties panel — toggle the active symbol's mirrored flag.
    SymEditorToggleSymbolMirrored,
    /// Properties panel — cycle the active symbol's local fill
    /// colour through preset palette → None → preset palette.
    SymEditorCycleLocalFillColor,
    /// Properties panel — cycle the active symbol's local line
    /// colour.
    SymEditorCycleLocalLineColor,
    /// Properties panel — cycle the active symbol's local pin
    /// colour.
    SymEditorCycleLocalPinColor,
    /// Document Options (Properties pane when nothing is selected)
    /// — set the sheet background color preset on the containing
    /// `.snxlib`. All `.snxsym` tabs from the same library share.
    SymEditorSetDisplaySheetColor(SheetColor),
    /// Document Options — toggle the visible dot grid on the
    /// containing `.snxlib`.
    SymEditorToggleDisplayGrid,
    /// Document Options — cycle the visible grid spacing through
    /// `crate::canvas::grid::GRID_SIZES_MM`.
    SymEditorCycleDisplayGridSize,
    /// Document Options — cycle the coordinate display unit on
    /// the containing `.snxlib` (mm → mil → inch → um → mm).
    SymEditorCycleDisplayUnit,
    /// SCH Library panel: switch the active symbol within the open
    /// `.snxsym` container to the given index.
    SchLibrarySelectSymbol(usize),
    /// SCH Library panel: append a new empty symbol to the open
    /// container and make it active. Caller emits a default name —
    /// the user renames via the Properties panel.
    SchLibraryAddSymbol,
    /// SCH Library panel: delete the symbol at the given index from
    /// the open container. Refuses to delete the last remaining
    /// symbol — the file would be empty otherwise.
    SchLibraryDeleteSymbol(usize),
    UpdateDrawingEdit(crate::app::contracts::DrawingFieldEdit),
    /// Numeric text_input keystroke for a drawing field. The string
    /// is stored verbatim in panel_ctx.drawing_edit_buf so empty /
    /// partial input survives between frames; the handler parses
    /// best-effort and fires UpdateDrawingEdit when the value is a
    /// valid f64.
    DrawingFieldTyping(DrawingFieldId, String),
    /// Open / close the border-colour picker overlay for a child sheet.
    ToggleChildSheetBorderPicker(uuid::Uuid),
    /// Open / close the fill-colour picker overlay for a child sheet.
    ToggleChildSheetFillPicker(uuid::Uuid),
    /// Expand the currently-open child-sheet picker dropdown into the
    /// full HSV / RGB ColorPicker overlay. `is_border` selects which
    /// channel (border vs fill).
    OpenChildSheetAdvancedPicker(uuid::Uuid, bool),
    /// Cancel the currently-open child-sheet colour picker without
    /// committing a new value.
    CancelChildSheetColorPicker,
    /// Commit a new border colour for a child sheet (closes the picker).
    EditChildSheetBorderColor(uuid::Uuid, iced::Color),
    /// Commit a new fill colour for a child sheet (closes the picker).
    EditChildSheetFillColor(uuid::Uuid, iced::Color),
    /// Buffered keystroke for the child sheet stroke-width input.
    ChildSheetStrokeWidthTyping(uuid::Uuid, String),
    /// Commit the currently-buffered child sheet stroke width.
    CommitChildSheetStrokeWidth(uuid::Uuid),
    /// Reset child sheet styling (border / fill colour, line width)
    /// back to theme defaults.
    ResetChildSheetStyle(uuid::Uuid),
    /// Pre-placement: confirm and close.
    ConfirmPrePlacement,
    /// Set snap grid size (mm).
    SetGridSize(f32),
    /// Set visible grid size (mm) — independent of snap grid.
    SetVisibleGridSize(f32),
    /// Toggle snap to electrical object hotspots.
    ToggleSnapHotspots,
    /// Change the UI font (saved to prefs; applies on next restart).
    #[allow(dead_code)]
    SetUiFont(String),
    /// Change the canvas font (applied immediately to schematic/PCB text).
    SetCanvasFont(String),
    /// Change canvas font size (px) applied to canvas text rendering.
    SetCanvasFontSize(f32),
    /// Toggle canvas font bold style.
    SetCanvasFontBold(bool),
    /// Toggle canvas font italic style.
    SetCanvasFontItalic(bool),
    /// Open canvas font popup.
    OpenCanvasFontPopup,
    /// Close canvas font popup.
    CloseCanvasFontPopup,
    /// Set page margin vertical zones.
    SetMarginVertical(u32),
    /// Set page margin horizontal zones.
    SetMarginHorizontal(u32),
    /// Toggle a single selection filter — shared with the Active Bar.
    ToggleSelectionFilter(crate::active_bar::SelectionFilter),
    /// Toggle all selection filters on/off — shared with the Active Bar.
    ToggleAllSelectionFilters,
    /// Append a new empty custom filter preset (no-op when at the cap).
    AddCustomFilterPreset,
    /// Remove the preset at this index.
    RemoveCustomFilterPreset(usize),
    /// Rename the preset at this index.
    RenameCustomFilterPreset(usize, String),
    /// Toggle whether the preset at `idx` includes `filter`.
    ToggleCustomFilterPresetMember(usize, crate::active_bar::SelectionFilter),
    /// Snapshot the active selection filter set into the preset at `idx`.
    CaptureCustomFilterPreset(usize),
    /// Switch the Properties-panel preset editor to the given tab.
    SelectCustomFilterTab(usize),
    /// Page Options: choose formatting mode.
    SetPageFormatMode(PageFormatMode),
    /// Page Options: choose paper size.
    SetPaperSize(String),
    /// Page Options: choose origin corner.
    SetPageOrigin(PageOrigin),
    /// Page Options: set custom paper width (mm).
    SetCustomPaperWidth(f32),
    /// Page Options: set custom paper height (mm).
    SetCustomPaperHeight(f32),
    /// Page Options: choose sheet background colour.
    SetSheetColor(SheetColor),
    /// No-op placeholder for unimplemented UI controls.
    Noop,
}

/// Render a panel's content.
pub fn view_panel<'a>(kind: PanelKind, ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    // Components has its own split scrollables — don't wrap again
    if kind == PanelKind::Components {
        return view_components(ctx);
    }

    let content = match kind {
        PanelKind::Components => return view_components(ctx),
        PanelKind::Projects => view_projects(ctx),
        PanelKind::Navigator => view_navigator(ctx),
        PanelKind::Properties => view_properties(ctx),
        PanelKind::Filter => view_stub("Selection Filter", "All object types enabled", ctx),
        PanelKind::Erc => view_erc(ctx),
        PanelKind::Messages => view_messages(ctx),
        PanelKind::Signal => view_stub("Signal AI", "Pro feature — AI design review", ctx),
        PanelKind::Drc => view_stub("DRC", "Run DRC to check PCB design rules", ctx),
        PanelKind::LayerStack => view_stub("Layer Stack", "PCB mode only", ctx),
        PanelKind::NetClasses => view_stub("Net Classes", "Define net classes and rules", ctx),
        PanelKind::Variants => view_stub("Variants", "Design variant management", ctx),
        PanelKind::OutputJobs => view_stub("Output Jobs", "Manufacturing output config", ctx),
        PanelKind::SchFilter => view_stub("SCH Filter", "Schematic object filter", ctx),
        PanelKind::SchList => view_stub("SCH List", "Schematic object list inspector", ctx),
        PanelKind::BomStudio => view_stub("BOM Studio", "Bill of Materials management", ctx),
        PanelKind::Favorites => view_stub("Favorites", "Favorite components and snippets", ctx),
        PanelKind::Snippets => view_stub("Snippets", "Reusable schematic snippets", ctx),
        PanelKind::Todo => view_stub("To-Do", "Task and issue tracking", ctx),
        PanelKind::Wiki => view_stub("Wiki", "Project documentation wiki", ctx),
        PanelKind::Library => view_stub(
            "Library",
            "Library panel — see Signex.library state. Use the dock host's Library panel \
             rendering path; this stub fires only if Library is mounted via PanelMsg \
             instead of LibraryMessage routing.",
            ctx,
        ),
        PanelKind::SchLibrary => view_sch_library(ctx),
    };

    scrollable(content).width(Length::Fill).into()
}

// ─── Helpers ──────────────────────────────────────────────────

// SVG chevrons (same as tree_view for consistency)
const SVG_CHEVRON_RIGHT: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 10 10"><path d="M3 1l5 4-5 4z" fill="currentColor"/></svg>"#;
const SVG_CHEVRON_DOWN: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 10 10"><path d="M1 3l4 5 4-5z" fill="currentColor"/></svg>"#;

fn chevron_right() -> svg::Handle {
    static H: OnceLock<svg::Handle> = OnceLock::new();
    H.get_or_init(|| svg::Handle::from_memory(SVG_CHEVRON_RIGHT))
        .clone()
}
fn chevron_down() -> svg::Handle {
    static H: OnceLock<svg::Handle> = OnceLock::new();
    H.get_or_init(|| svg::Handle::from_memory(SVG_CHEVRON_DOWN))
        .clone()
}

fn shape_icon_handle(elem_type: &str, theme: signex_types::theme::ThemeId) -> Option<svg::Handle> {
    match elem_type {
        "Line" => Some(crate::icons::icon_shape_line(theme)),
        "Rectangle" => Some(crate::icons::icon_shape_rect(theme)),
        "Circle" => Some(crate::icons::icon_shape_circle(theme)),
        "Arc" => Some(crate::icons::icon_shape_arc(theme)),
        "Polygon" => Some(crate::icons::icon_shape_polygon(theme)),
        _ => None,
    }
}

/// Collapsible section: clickable header with SVG chevron, hides content when collapsed.
fn collapsible_section<'a>(
    key: &str,
    title: &str,
    collapsed: &CollapsedSections,
    header_color: Color,
    border_c: Color,
    content: impl FnOnce() -> Column<'a, PanelMsg>,
) -> Column<'a, PanelMsg> {
    let is_collapsed = collapsed.contains(key);
    let chevron_handle = if is_collapsed {
        chevron_right()
    } else {
        chevron_down()
    };
    let key_owned = key.to_string();

    let mut col: Column<'a, PanelMsg> = Column::new().spacing(0).width(Length::Fill);

    // Clickable header with SVG chevron
    col = col.push(
        iced::widget::button(
            container(
                row![
                    svg(chevron_handle)
                        .width(10)
                        .height(10)
                        .style(move |_: &Theme, _| iced::widget::svg::Style {
                            color: Some(header_color),
                        }),
                    text(title.to_string()).size(10).color(header_color),
                ]
                .spacing(4)
                .align_y(iced::Alignment::Center),
            )
            .padding([4, 8])
            .width(Length::Fill),
        )
        .padding(0)
        .width(Length::Fill)
        .on_press(PanelMsg::ToggleSection(key_owned))
        .style(move |_: &Theme, status: iced::widget::button::Status| {
            let bg = match status {
                iced::widget::button::Status::Hovered => Some(iced::Background::Color(border_c)),
                _ => None,
            };
            iced::widget::button::Style {
                background: bg,
                border: Border::default(),
                ..iced::widget::button::Style::default()
            }
        }),
    );
    col = col.push(thin_sep(border_c));

    // Content (only when expanded)
    if !is_collapsed {
        col = col.push(content());
    }

    col
}

/// Property key-value row (owned strings to avoid lifetime issues in closures).
fn prop_kv_row<'a>(key: &str, value: &str, key_c: Color, val_c: Color) -> Element<'a, PanelMsg> {
    container(
        row![
            text(key.to_string()).size(10).color(key_c).width(84),
            text(value.to_string()).size(10).color(val_c),
        ]
        .spacing(4),
    )
    .padding([4, 8])
    .width(Length::Fill)
    .into()
}

fn section_title<'a>(title: &str, tokens: &ThemeTokens) -> iced::widget::Text<'a> {
    text(title.to_uppercase())
        .size(9)
        .color(theme_ext::text_secondary(tokens))
}

fn separator<'a>(tokens: &ThemeTokens) -> iced::widget::Text<'a> {
    // Use a cached static string instead of allocating on every render
    const SEP: &str = "──────────────────────────────";
    text(SEP).size(4).color(theme_ext::border_color(tokens))
}

fn view_stub<'a>(title: &str, desc: &str, ctx: &PanelContext) -> Element<'a, PanelMsg> {
    container(
        column![
            section_title(title, &ctx.tokens),
            separator(&ctx.tokens),
            text(desc.to_string())
                .size(10)
                .color(theme_ext::text_secondary(&ctx.tokens)),
        ]
        .spacing(4)
        .padding(6),
    )
    .width(Length::Fill)
    .into()
}

// ─── SCH Library Panel (Altium parity) ───────────────────────────────
//
// When a `.snxsym` standalone editor tab is active, this panel lists
// the symbols in the open `SymbolFile` container. Click switches the
// active symbol; "Add Symbol" appends a fresh empty Symbol to the
// container and makes it active. Read-only on Symbol metadata for now —
// rename / designator-prefix edits go through the right-dock Properties
// panel.
//
// When no Symbol editor is open the panel renders a hint pointing the
// user at the project tree's `Add New ▸ Symbol` flow.

/// Render one indented part row inside the SCH Library tree-expander.
/// Active part gets the selection background; otherwise the row hovers
/// like the symbol rows above it.
fn part_tree_row<'a>(
    label: &str,
    part: u8,
    is_active: bool,
    primary: Color,
    muted: Color,
    bg_active: Color,
) -> Element<'a, PanelMsg> {
    let label_color = if is_active { primary } else { muted };
    iced::widget::button(
        row![
            // Tree-expander indent: 18 px gutter + a faint glyph so
            // the part rows visually nest under the symbol.
            text("\u{2514}")
                .size(10)
                .color(muted)
                .width(Length::Fixed(18.0)),
            text(label.to_string())
                .size(10)
                .color(label_color)
                .width(Length::Fill),
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center),
    )
    .padding([3, 8])
    .width(Length::Fill)
    .on_press(PanelMsg::SymEditorSelectPart(part))
    .style(
        move |_: &iced::Theme, status: iced::widget::button::Status| iced::widget::button::Style {
            background: if is_active {
                Some(iced::Background::Color(bg_active))
            } else if matches!(status, iced::widget::button::Status::Hovered) {
                Some(iced::Background::Color(iced::Color::from_rgba(
                    1.0, 1.0, 1.0, 0.04,
                )))
            } else {
                None
            },
            border: iced::Border::default(),
            text_color: label_color,
            ..iced::widget::button::Style::default()
        },
    )
    .into()
}

fn view_sch_library<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let muted = theme_ext::text_secondary(&ctx.tokens);
    let primary = theme_ext::text_primary(&ctx.tokens);
    let border_c = theme_ext::border_color(&ctx.tokens);

    let mut col: Column<'a, PanelMsg> = Column::new().spacing(0).width(Length::Fill);
    col = col.push(
        container(text("SCH Library").size(11).color(primary))
            .padding([6, 8])
            .width(Length::Fill),
    );
    col = col.push(thin_sep(border_c));

    let Some(sym) = ctx.symbol_editor.as_ref() else {
        col = col.push(
            container(
                text(
                    "Open a `.snxsym` to see its symbols here. Right-click a library node \
                     in the project tree and pick `Add New ▸ Symbol` to create one.",
                )
                .size(10)
                .color(muted),
            )
            .padding([6, 8])
            .width(Length::Fill),
        );
        return scrollable(col).width(Length::Fill).into();
    };

    // ── File header ──
    col = col.push(
        container(
            text(format!(
                "{} ({} symbols)",
                sym.path
                    .file_name()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "<untitled>".to_string()),
                sym.symbols_in_file.len(),
            ))
            .size(10)
            .color(muted),
        )
        .padding([4, 8]),
    );
    col = col.push(thin_sep(border_c));

    // ── Column header — Altium SCH Library parity. Two columns:
    //     Design Item ID | Description.
    col = col.push(
        container(
            row![
                text("Design Item ID")
                    .size(10)
                    .color(muted)
                    .width(Length::FillPortion(3)),
                text("Description")
                    .size(10)
                    .color(muted)
                    .width(Length::FillPortion(4)),
            ]
            .spacing(6),
        )
        .padding([4, 8])
        .width(Length::Fill),
    );
    col = col.push(thin_sep(border_c));

    // ── Symbols tree ──
    // Each symbol is a row showing Design Item ID + Description.
    // Multi-part symbols (or symbols with Part Zero pins) expand
    // under the active symbol with one row per part — click to
    // switch active_part.
    for entry in &sym.symbols_in_file {
        let is_active = entry.idx == sym.active_idx;
        let label_color = if is_active { primary } else { muted };
        let bg_active = crate::styles::ti(ctx.tokens.selection);
        let row_msg = PanelMsg::SchLibrarySelectSymbol(entry.idx);
        let row_btn = iced::widget::button(
            row![
                text(entry.name.clone())
                    .size(11)
                    .color(label_color)
                    .width(Length::FillPortion(3)),
                text(entry.description.clone())
                    .size(10)
                    .color(muted)
                    .width(Length::FillPortion(4)),
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center),
        )
        .padding([4, 8])
        .width(Length::Fill)
        .on_press(row_msg)
        .style(
            move |_: &iced::Theme, status: iced::widget::button::Status| {
                iced::widget::button::Style {
                    background: if is_active {
                        Some(iced::Background::Color(bg_active))
                    } else if matches!(status, iced::widget::button::Status::Hovered) {
                        Some(iced::Background::Color(iced::Color::from_rgba(
                            1.0, 1.0, 1.0, 0.04,
                        )))
                    } else {
                        None
                    },
                    border: iced::Border::default(),
                    text_color: label_color,
                    ..iced::widget::button::Style::default()
                }
            },
        );
        col = col.push(row_btn);

        // Part tree-expander under the active multi-part symbol.
        if is_active && (sym.active_max_part > 1 || sym.active_has_part_zero) {
            if sym.active_has_part_zero {
                col = col.push(part_tree_row(
                    "Part 0 (shared)",
                    0,
                    sym.active_part == 0,
                    primary,
                    muted,
                    bg_active,
                ));
            }
            for part in 1..=sym.active_max_part {
                col = col.push(part_tree_row(
                    &format!("Part {part}"),
                    part,
                    sym.active_part == part,
                    primary,
                    muted,
                    bg_active,
                ));
            }
        }
    }

    col = col.push(thin_sep(border_c));

    // Pins / Graphics sub-lists used to live here; per Altium SCH
    // Library panel parity they belong on dedicated panels (Pins
    // already shows in the Properties panel's Pin selection branch;
    // a dedicated SCHLIB Filter / SCHLIB List pair lives on the
    // bottom panel-tabs strip when that ships).
    col = col.push(thin_sep(border_c));

    // ── Action row: Place / Add / Delete / Edit (Altium parity) ──
    let action_btn_style = move |is_primary: bool, enabled: bool| {
        let text_color = if enabled { primary } else { muted };
        move |_: &iced::Theme, status: iced::widget::button::Status| {
            let bg_alpha = if !enabled {
                0.02
            } else if is_primary {
                if matches!(status, iced::widget::button::Status::Hovered) {
                    0.10
                } else {
                    0.04
                }
            } else if matches!(status, iced::widget::button::Status::Hovered) {
                0.08
            } else {
                0.03
            };
            iced::widget::button::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgba(
                    1.0, 1.0, 1.0, bg_alpha,
                ))),
                border: iced::Border {
                    width: 1.0,
                    radius: 3.0.into(),
                    color: border_c,
                },
                text_color,
                ..iced::widget::button::Style::default()
            }
        }
    };

    // Place — fires on the active symbol (would dispatch to schematic
    // place flow when wired). Stub for now: greyed-out.
    let place_btn = iced::widget::button(text("Place").size(11).color(muted))
        .padding([4, 10])
        .style(action_btn_style(true, false));

    let add_btn = iced::widget::button(text("Add").size(11).color(primary))
        .padding([4, 10])
        .on_press(PanelMsg::SchLibraryAddSymbol)
        .style(action_btn_style(false, true));

    let can_delete = sym.symbols_in_file.len() > 1;
    let delete_color = if can_delete { primary } else { muted };
    let mut delete_btn = iced::widget::button(text("Delete").size(11).color(delete_color))
        .padding([4, 10])
        .style(action_btn_style(false, can_delete));
    if can_delete {
        delete_btn = delete_btn.on_press(PanelMsg::SchLibraryDeleteSymbol(sym.active_idx));
    }

    // Edit — opens the active symbol in the standalone editor (for
    // when the SCH Library panel is the only visible surface).
    // Stub for now since the symbol is already open in its tab.
    let edit_btn = iced::widget::button(text("Edit").size(11).color(muted))
        .padding([4, 10])
        .style(action_btn_style(false, false));

    col = col.push(
        container(
            row![
                place_btn,
                Space::new().width(4),
                add_btn,
                Space::new().width(4),
                delete_btn,
                Space::new().width(4),
                edit_btn,
                Space::new().width(Length::Fill),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding([6, 8]),
    );

    scrollable(col).width(Length::Fill).into()
}

// ─── Projects Panel (TreeView) ────────────────────────────────

/// Build the project tree from panel context data. Produces one root
/// per loaded project so multi-project workspaces show all their
/// projects side by side. Single-project users see the same shape as
/// before (one root). `PanelContext::projects` is the source of truth;
/// the legacy `project_name` / `sheets` singletons are ignored here so
/// we never emit a duplicate root for the active project.
pub fn build_project_tree(ctx: &PanelContext) -> Vec<TreeNode> {
    if ctx.projects.is_empty() {
        return vec![];
    }

    ctx.projects
        .iter()
        .map(project_root_node)
        .collect()
}

/// One project root — "Source Documents" / "Libraries" / "Settings".
/// The Libraries branch lists this project's mounted `*.snxlib`
/// entries (right-click → Add New ▸ Component Library to add one);
/// it renders empty when the project has no libraries rather than
/// inheriting a workspace-wide symbol count.
fn project_root_node(project: &ProjectPanelInfo) -> TreeNode {
    let mut source_docs: Vec<TreeNode> = Vec::new();

    if !project.sheets.is_empty() {
        for sheet in &project.sheets {
            let icon = TreeIcon::for_path(&sheet.filename);
            source_docs.push(
                TreeNode::leaf(sheet.filename.clone(), icon)
                    .with_open(sheet.is_open)
                    .with_dirty(sheet.is_dirty)
                    .with_active(sheet.is_active),
            );
        }
    } else if let Some(file) = &project.project_file {
        let icon = TreeIcon::for_path(file);
        source_docs.push(
            TreeNode::leaf(file.clone(), icon)
                .with_open(project.project_file_open)
                .with_dirty(project.project_file_dirty)
                .with_active(project.project_file_active),
        );
    }

    if let Some(pcb) = &project.pcb_file {
        let icon = TreeIcon::for_path(pcb);
        source_docs.push(
            TreeNode::leaf(pcb.clone(), icon)
                .with_open(project.pcb_file_open)
                .with_dirty(project.pcb_file_dirty)
                .with_active(project.pcb_file_active),
        );
    }

    // Render each `Project::libraries[i]` entry as a single `.snxlib`
    // leaf — no children, no chevron. Under the v0.9 `.snxlib`-as-file
    // model a library is one thing the user opens, not a folder they
    // browse. Symbols / footprints / sims are siblings on disk, but
    // surfacing them in the project tree confuses the mental model:
    // the user's contract is the `.snxlib` file, not its private
    // working directory. Double-click opens the Library Browser tab,
    // which is the proper surface for browsing the library's contents.
    //
    // When the project carries no library entries the Libraries branch
    // renders empty (matching Settings) — the user can right-click →
    // Add New ▸ Component Library to create one. We deliberately do
    // NOT mint a synthetic "N symbols loaded" child from the
    // workspace-wide library count: that pre-DBLib placeholder
    // advertised symbols the project hadn't actually mounted, which
    // caused real confusion (a project with nothing saved would show
    // "222 symbols loaded" sourced from globally-mounted libraries).
    let lib_children: Vec<TreeNode> = project
        .libraries
        .iter()
        .map(|lib| {
            let display = format!("{}.snxlib", lib.display_name);
            TreeNode::leaf(display, TreeIcon::SnxLibrary)
        })
        .collect();

    // Settings holds nothing today — gated until a project actually
    // carries per-project preferences. Showing an empty branch reads
    // as "this project has settings hidden behind a toggle"; the
    // honest UI is to omit the heading.
    let settings_children: Vec<TreeNode> = Vec::new();

    // Build the project's child list, skipping any heading whose
    // children list is empty so the tree never renders a bare
    // "(empty)" placeholder under Libraries / Settings.
    let mut children: Vec<TreeNode> = Vec::new();
    if !source_docs.is_empty() {
        children.push(TreeNode::branch(
            "Source Documents".to_string(),
            TreeIcon::Folder,
            source_docs,
        ));
    }
    if !lib_children.is_empty() {
        children.push(TreeNode::branch(
            "Libraries".to_string(),
            TreeIcon::Library,
            lib_children,
        ));
    }
    if !settings_children.is_empty() {
        let mut settings =
            TreeNode::branch("Settings".to_string(), TreeIcon::File, settings_children);
        settings.expanded = false;
        children.push(settings);
    }

    TreeNode::branch(project.name.clone(), TreeIcon::Folder, children)
        .with_accent(project.is_active)
}

fn view_projects<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    if ctx.project_tree.is_empty() {
        let muted = theme_ext::text_secondary(&ctx.tokens);
        column![
            text("No project open").size(11).color(muted),
            text("").size(4),
            text("File > Open to begin").size(10).color(muted),
        ]
        .spacing(2)
        .padding(6)
        .width(Length::Fill)
        .into()
    } else {
        // Render the persistent tree — toggle state is preserved.
        // Wrap in a container with a small top inset so the tree's
        // first row doesn't sit flush against the panel's tab-strip
        // border (matches the breathing room Altium leaves below its
        // panel tabs).
        container(
            TreeView::new(&ctx.project_tree, &ctx.tokens)
                .view()
                .map(PanelMsg::Tree),
        )
        .padding(iced::Padding {
            top: 6.0,
            right: 0.0,
            bottom: 0.0,
            left: 0.0,
        })
        .width(Length::Fill)
        .into()
    }
}

// ─── Components Panel (matched to Altium Designer) ───────────

fn view_components<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let muted = theme_ext::text_secondary(&ctx.tokens);
    let primary = theme_ext::text_primary(&ctx.tokens);
    let border_c = theme_ext::border_color(&ctx.tokens);
    let hover_c = crate::styles::ti(ctx.tokens.hover);
    let panel_bg_c = crate::styles::ti(ctx.tokens.panel_bg);
    let input_bg = crate::styles::ti(ctx.tokens.selection);
    let input_bdr = crate::styles::ti(ctx.tokens.accent);

    // ── TOP: Library selector + component list (scrollable) ──
    let mut list_col: Column<'a, PanelMsg> = Column::new().spacing(0).width(Length::Fill);

    list_col = list_col.push(
        container(
            iced::widget::pick_list(
                ctx.standard_libraries.clone(),
                ctx.active_library.clone(),
                PanelMsg::SelectLibrary,
            )
            .placeholder("Select a library...")
            .text_size(11)
            .width(Length::Fill),
        )
        .padding([4, 8]),
    );

    // Search filter input
    list_col = list_col.push(
        container(
            iced::widget::text_input("Search components...", &ctx.component_filter)
                .on_input(PanelMsg::ComponentFilter)
                .size(11)
                .width(Length::Fill),
        )
        .padding([4, 8]),
    );

    list_col = list_col.push(thin_sep(border_c));
    list_col = list_col.push(
        container(
            row![
                text("Name")
                    .size(10)
                    .color(muted)
                    .width(Length::FillPortion(3)),
                text("Library")
                    .size(10)
                    .color(muted)
                    .width(Length::FillPortion(2)),
                text("Pins")
                    .size(10)
                    .color(muted)
                    .width(Length::FillPortion(1)),
            ]
            .spacing(4.0),
        )
        .padding([4, 8])
        .width(Length::Fill),
    );
    list_col = list_col.push(thin_sep(border_c));

    // Filter the symbol list
    let filter = ctx.component_filter.to_ascii_lowercase();
    let filtered_symbols: Vec<&LibrarySymbolEntry> = if filter.is_empty() {
        ctx.library_symbols.iter().collect()
    } else {
        ctx.library_symbols
            .iter()
            .filter(|entry| {
                entry.symbol_name.to_ascii_lowercase().contains(&filter)
                    || entry.library_name.to_ascii_lowercase().contains(&filter)
                    || entry.lib_id.to_ascii_lowercase().contains(&filter)
            })
            .collect()
    };

    if filtered_symbols.is_empty() {
        let msg = if ctx.active_library.is_some() {
            if filter.is_empty() {
                "Loading..."
            } else {
                "No matches"
            }
        } else {
            "Select a library above"
        };
        list_col = list_col.push(container(text(msg).size(10).color(muted)).padding([8, 8]));
    } else {
        let sel = &ctx.selected_component;
        for entry in &filtered_symbols {
            let is_sel = sel.as_deref() == Some(entry.lib_id.as_str());
            let row_bg = if is_sel {
                theme_ext::selection_color(&ctx.tokens)
            } else {
                Color::TRANSPARENT
            };
            let name_c = if is_sel { Color::WHITE } else { primary };
            let lib_id = entry.lib_id.clone();
            list_col = list_col.push(
                column![
                    iced::widget::button(
                        row![
                            text(entry.symbol_name.clone())
                                .size(10)
                                .color(name_c)
                                .width(Length::FillPortion(3))
                                .wrapping(iced::widget::text::Wrapping::None),
                            text(entry.library_name.clone())
                                .size(10)
                                .color(muted)
                                .width(Length::FillPortion(2))
                                .wrapping(iced::widget::text::Wrapping::None),
                            text(entry.pin_count.to_string())
                                .size(10)
                                .color(muted)
                                .width(Length::FillPortion(1)),
                        ]
                        .spacing(4.0),
                    )
                    .padding([3, 8])
                    .width(Length::Fill)
                    .on_press(PanelMsg::SelectComponent(lib_id))
                    .style(
                        move |_: &Theme, status: iced::widget::button::Status| {
                            let bg = match (is_sel, status) {
                                (true, _) => Some(Background::Color(row_bg)),
                                (false, iced::widget::button::Status::Hovered) => {
                                    Some(Background::Color(hover_c))
                                }
                                _ => None,
                            };
                            iced::widget::button::Style {
                                background: bg,
                                border: Border::default(),
                                ..iced::widget::button::Style::default()
                            }
                        }
                    ),
                    thin_sep(border_c),
                ]
                .spacing(0),
            );
        }
    }

    list_col = list_col.push(
        container(
            text(format!("Results: {}", filtered_symbols.len()))
                .size(10)
                .color(muted),
        )
        .padding([4, 8]),
    );

    // ── BOTTOM: Details panel (scrollable) ──
    let mut detail_col: Column<'a, PanelMsg> = Column::new().spacing(0).width(Length::Fill);

    if let Some(comp_id) = &ctx.selected_component {
        let selected_entry = ctx
            .library_symbols
            .iter()
            .find(|entry| &entry.lib_id == comp_id);
        let comp_name = selected_entry
            .map(|entry| entry.symbol_name.as_str())
            .unwrap_or(comp_id.as_str());
        detail_col = detail_col.push(section_hdr(
            &format!("\u{25BC} Details  {comp_name}"),
            primary,
            border_c,
        ));
        let pin_count = ctx
            .library_symbols
            .iter()
            .find(|entry| entry.lib_id == *comp_id)
            .map(|entry| entry.pin_count)
            .unwrap_or(0);
        detail_col = detail_col.push(form_input_row(
            "Symbol", comp_name, muted, input_bg, input_bdr,
        ));
        detail_col = detail_col.push(form_input_row(
            "Pins",
            &pin_count.to_string(),
            muted,
            input_bg,
            input_bdr,
        ));
        detail_col = detail_col.push(form_input_row(
            "Library",
            selected_entry
                .map(|entry| entry.library_name.as_str())
                .or(ctx.active_library.as_deref())
                .unwrap_or(""),
            muted,
            input_bg,
            input_bdr,
        ));

        // Symbol preview canvas
        detail_col = detail_col.push(Space::new().height(4.0));
        detail_col = detail_col.push(section_hdr("\u{25BC} Models", primary, border_c));
        if let Some(lib_sym) = &ctx.selected_lib_symbol {
            // Symbol preview
            detail_col = detail_col.push(
                container(
                    container(
                        signex_widgets::symbol_preview::symbol_preview(lib_sym.clone(), 120.0)
                            .map(|_: ()| PanelMsg::ToggleGrid),
                    )
                    .width(Length::Fill)
                    .style(move |_: &Theme| container::Style {
                        background: Some(Background::Color(panel_bg_c)),
                        border: Border {
                            width: 1.0,
                            radius: 2.0.into(),
                            color: border_c,
                        },
                        ..container::Style::default()
                    }),
                )
                .padding([4, 8]),
            );

            // Footprint preview placeholder
            detail_col = detail_col.push(
                container(
                    container(
                        text("Footprint preview")
                            .size(10)
                            .color(muted)
                            .align_x(iced::alignment::Horizontal::Center),
                    )
                    .width(Length::Fill)
                    .padding([30, 8])
                    .style(move |_: &Theme| container::Style {
                        background: Some(Background::Color(panel_bg_c)),
                        border: Border {
                            width: 1.0,
                            radius: 2.0.into(),
                            color: border_c,
                        },
                        ..container::Style::default()
                    }),
                )
                .padding([4, 8]),
            );
        }

        for section in &["References", "Part Choices", "Where Used"] {
            detail_col = detail_col.push(Space::new().height(2.0));
            detail_col = detail_col.push(section_hdr(
                &format!("\u{25BC} {section}"),
                primary,
                border_c,
            ));
        }
    } else {
        detail_col = detail_col.push(
            container(text("Select a component").size(10).color(muted))
                .padding([12, 8])
                .width(Length::Fill),
        );
    }

    // Split view: list (fixed height) | handle | details (fill)
    column![
        container(scrollable(list_col).width(Length::Fill))
            .height(ctx.components_split)
            .width(Length::Fill),
        // Drag handle
        iced::widget::mouse_area(
            container(Space::new())
                .height(5.0)
                .width(Length::Fill)
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(border_c)),
                    ..container::Style::default()
                }),
        )
        .interaction(iced::mouse::Interaction::ResizingVertically)
        .on_press(PanelMsg::DragComponentsSplit),
        container(scrollable(detail_col).width(Length::Fill))
            .height(Length::Fill)
            .width(Length::Fill),
    ]
    .spacing(0)
    .into()
}

// ─── Navigator Panel ──────────────────────────────────────────

fn view_navigator<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let mut col: Column<'a, PanelMsg> = Column::new().spacing(2).padding(6).width(Length::Fill);
    col = col.push(section_title("Sheets", &ctx.tokens));
    col = col.push(separator(&ctx.tokens));

    // Resolve the active project from the multi-project Vec — replaces
    // the legacy `ctx.project_name` singleton (#54 phase 2.5).
    let active_project = ctx.projects.iter().find(|p| p.is_active);
    if let Some(project) = active_project {
        let mut sheets = vec![];
        for cs in &ctx.child_sheets {
            sheets.push(TreeNode::leaf(cs.clone(), TreeIcon::Sheet));
        }
        let roots = vec![TreeNode::branch(
            project.name.clone(),
            TreeIcon::Schematic,
            sheets,
        )];
        col = col.push(
            TreeView::new(&roots, &ctx.tokens)
                .view()
                .map(PanelMsg::Tree),
        );
    } else {
        col = col.push(
            text("No project")
                .size(10)
                .color(theme_ext::text_secondary(&ctx.tokens)),
        );
    }
    container(col).width(Length::Fill).into()
}

// ─── Properties Panel (matched to Altium Designer) ───────────

const LABEL_W: f32 = 76.0;
const PROPERTY_LABEL_PORTION: u16 = 2;
const PROPERTY_CONTROL_PORTION: u16 = 5;
const PROPERTY_ROW_PAD_X: u16 = 6;

fn view_properties<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let muted = theme_ext::text_secondary(&ctx.tokens);
    let primary = theme_ext::text_primary(&ctx.tokens);
    let border_c = theme_ext::border_color(&ctx.tokens);
    let input_bg = crate::styles::ti(ctx.tokens.selection);
    let input_bdr = crate::styles::ti(ctx.tokens.accent);

    // Symbol-editor tab takes precedence — when the user is editing a
    // `.snxsym` the right-dock Properties panel shows symbol/pin
    // properties driven by `panel_ctx.symbol_editor`. Matches Altium's
    // SchLib editor flow where the same Properties panel switches mode
    // based on selection. (#62 / v0.9 phase 1)
    if let Some(sym) = ctx.symbol_editor.as_ref() {
        return view_symbol_editor_properties(sym, muted, primary, border_c);
    }

    if !ctx.has_schematic {
        // Don't mislead the user into thinking nothing is loaded when
        // they've just switched to a PCB tab — distinguish "no project
        // yet" from "project open, but the active tab isn't a
        // schematic". GitHub issue #51.
        let hint = if ctx.has_pcb {
            "Properties are available when a schematic is active"
        } else {
            "Open a project"
        };
        return container(
            column![
                text("Properties").size(12).color(primary),
                Space::new().height(12.0),
                text(hint).size(11).color(muted),
            ]
            .spacing(4)
            .padding(8),
        )
        .width(Length::Fill)
        .into();
    }

    // ── Pre-placement properties (TAB pressed during tool) ──
    // TAB pauses placement and edits the properties the NEXT click will
    // commit with. We render a full Altium-style Location + Properties
    // form bound to the pre_placement data — not the live engine.
    if let Some(ref pp) = ctx.pre_placement {
        return view_pre_placement(pp, ctx, muted, primary, border_c, input_bg, input_bdr);
    }

    // ── Context-aware: if something is selected, show element properties (Altium style) ──
    if ctx.selection_count == 1 && !ctx.selection_info.is_empty() {
        return view_selected_element_properties(ctx, muted, primary, border_c);
    }
    if ctx.selection_count > 1 {
        let mut col: Column<'a, PanelMsg> = Column::new().spacing(0).width(Length::Fill);
        col = col.push(
            container(text("Multi-Selection").size(11).color(primary))
                .padding([6, 8])
                .width(Length::Fill),
        );
        col = col.push(thin_sep(border_c));
        col = col.push(
            container(
                text(format!("{} objects selected", ctx.selection_count))
                    .size(10)
                    .color(muted),
            )
            .padding([4, 8]),
        );
        for (key, value) in &ctx.selection_info {
            col = col.push(
                container(
                    row![
                        text(key)
                            .size(10)
                            .color(muted)
                            .width(Length::FillPortion(2)),
                        text(value)
                            .size(10)
                            .color(primary)
                            .width(Length::FillPortion(3)),
                    ]
                    .spacing(4),
                )
                .padding([3, 8])
                .width(Length::Fill),
            );
        }
        return scrollable(col).width(Length::Fill).into();
    }

    // ── Nothing selected: show Document Options (Altium default) ──
    let mut col: Column<'a, PanelMsg> = Column::new().spacing(0).width(Length::Fill);

    col = col.push(
        container(text("Document Options").size(11).color(primary))
            .padding([6, 8])
            .width(Length::Fill),
    );

    // ── General | Parameters tab bar ──
    let tab = ctx.properties_tab;
    let tab_hover = crate::styles::ti(ctx.tokens.hover);
    let text_inactive = crate::styles::ti(ctx.tokens.text_secondary);
    col = col.push(
        container(
            row![
                props_tab_btn(
                    "General",
                    tab == 0,
                    PanelMsg::PropertiesTab(0),
                    primary,
                    text_inactive,
                    tab_hover,
                    border_c
                ),
                props_tab_btn(
                    "Parameters",
                    tab == 1,
                    PanelMsg::PropertiesTab(1),
                    primary,
                    text_inactive,
                    tab_hover,
                    border_c
                ),
            ]
            .spacing(2.0),
        )
        .padding([4, 8]),
    );
    col = col.push(thin_sep(border_c));

    // ── Tab content ──
    if tab == 0 {
        col = col.push(view_properties_general(ctx, muted, primary, border_c));
    } else {
        col = col.push(view_properties_parameters(
            muted,
            primary,
            border_c,
            crate::styles::ti(ctx.tokens.selection),
            crate::styles::ti(ctx.tokens.accent),
            crate::styles::ti(ctx.tokens.hover),
        ));
    }

    // ── Status: Nothing selected ──
    col = col.push(Space::new().height(8.0));
    col = col.push(thin_sep(border_c));
    col = col.push(
        container(text("Nothing selected").size(10).color(muted))
            .padding([6, 8])
            .width(Length::Fill),
    );

    scrollable(col).width(Length::Fill).into()
}

/// IEEE-symbol pick_list row used four times (Inside / Inside Edge /
/// Outside Edge / Outside) on the pin Properties surface. `slot`
/// matches the `SymEditorSetPinSymbol::slot` numbering: 0 / 1 / 2 / 3.
fn view_pin_symbol_picker<'a>(
    label: &str,
    current: signex_library::PinSymbolKind,
    pin_idx: usize,
    slot: u8,
    muted: Color,
) -> Element<'a, PanelMsg> {
    use signex_library::PinSymbolKind as K;
    let options = [
        ("None", K::None),
        ("Dot (active-low bubble)", K::Dot),
        ("Clock edge", K::ClockEdge),
        ("Active-low input", K::ActiveLowInput),
        ("Active-low output", K::ActiveLowOutput),
        ("Schmitt trigger", K::SchmittTrigger),
        ("Analog (≈)", K::Analog),
        ("Digital (square wave)", K::Digital),
        ("Shift-right (▷)", K::ShiftRight),
        ("Shift-left (◁)", K::ShiftLeft),
        ("Pi (π)", K::Pi),
        ("Sigma (Σ)", K::Sigma),
        ("Open collector", K::OpenCollector),
        ("Open emitter", K::OpenEmitter),
        ("Hi-Z (tri-state)", K::HiZ),
    ];
    let labels: Vec<String> = options.iter().map(|(l, _)| l.to_string()).collect();
    let lookup: Vec<(String, K)> = options.iter().map(|(l, v)| (l.to_string(), *v)).collect();
    let current_label = options
        .iter()
        .find(|(_, v)| *v == current)
        .map(|(l, _)| l.to_string())
        .unwrap_or_else(|| "None".to_string());
    let picker = iced::widget::pick_list(labels, Some(current_label), move |chosen: String| {
        let value = lookup
            .iter()
            .find(|(l, _)| l == &chosen)
            .map(|(_, v)| *v)
            .unwrap_or(K::None);
        PanelMsg::SymEditorSetPinSymbol {
            pin_idx,
            slot,
            value,
        }
    })
    .padding([2, 4])
    .text_size(10);
    container(
        row![
            text(label.to_string())
                .size(10)
                .color(muted)
                .width(Length::FillPortion(2)),
            container(picker).width(Length::FillPortion(3)),
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center),
    )
    .padding([3, 8])
    .width(Length::Fill)
    .into()
}

/// Properties panel content for the active `.snxsym` standalone editor
/// tab. Mirrors Altium SchLib's right-dock Properties: pin selected →
/// pin properties (editable Designator / Name / Length, read-only
/// Electrical / Position / Orientation), field selected → field
/// properties, nothing selected → symbol-level defaults (Name /
/// UUID / pin count) with Name editable.
/// Render one Local Colors row — three click-to-cycle swatches
/// (Fills / Lines / Pins). Each swatch shows the current override
/// colour or a striped "inherit" pattern when `None`. Clicking
/// cycles through a small preset palette + back to None.
fn local_colors_row<'a>(
    label: &'a str,
    fill: Option<[u8; 4]>,
    line: Option<[u8; 4]>,
    pin: Option<[u8; 4]>,
    muted: Color,
) -> Element<'a, PanelMsg> {
    let swatch = |slot_label: &'a str, c: Option<[u8; 4]>, msg: PanelMsg| {
        let bg = match c {
            Some([r, g, b, a]) => iced::Color::from_rgba8(r, g, b, (a as f32) / 255.0),
            None => iced::Color::from_rgba(0.5, 0.5, 0.5, 0.25),
        };
        let border = if c.is_some() {
            iced::Color::from_rgba(0.0, 0.0, 0.0, 0.35)
        } else {
            iced::Color::from_rgba(1.0, 1.0, 1.0, 0.30)
        };
        column![
            text(slot_label).size(9).color(muted),
            iced::widget::button(iced::widget::Space::new())
                .padding(0)
                .width(Length::Fixed(28.0))
                .height(Length::Fixed(16.0))
                .on_press(msg)
                .style(
                    move |_: &iced::Theme, _status: iced::widget::button::Status| {
                        iced::widget::button::Style {
                            background: Some(iced::Background::Color(bg)),
                            border: iced::Border {
                                width: 1.0,
                                radius: 2.0.into(),
                                color: border,
                            },
                            ..iced::widget::button::Style::default()
                        }
                    }
                ),
        ]
        .spacing(2)
        .align_x(iced::Alignment::Center)
    };

    container(
        row![
            text(label.to_string())
                .size(10)
                .color(muted)
                .width(Length::FillPortion(2)),
            row![
                swatch("Fills", fill, PanelMsg::SymEditorCycleLocalFillColor),
                Space::new().width(8),
                swatch("Lines", line, PanelMsg::SymEditorCycleLocalLineColor),
                Space::new().width(8),
                swatch("Pins", pin, PanelMsg::SymEditorCycleLocalPinColor),
            ]
            .width(Length::FillPortion(3)),
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center),
    )
    .padding([3, 8])
    .width(Length::Fill)
    .into()
}

fn view_symbol_editor_properties<'a>(
    sym: &'a SymbolEditorPanelContext,
    muted: Color,
    primary: Color,
    border_c: Color,
) -> Element<'a, PanelMsg> {
    let mut col: Column<'a, PanelMsg> = Column::new().spacing(0).width(Length::Fill);

    let header_label = match &sym.selected {
        SymbolEditorSelection::None => "Symbol",
        SymbolEditorSelection::Pin(_) => "Pin",
        SymbolEditorSelection::FieldReference => "Field — Reference",
        SymbolEditorSelection::FieldValue => "Field — Value",
        SymbolEditorSelection::Graphic(g) => match &g.kind {
            GraphicKindSummary::Rectangle { .. } => "Graphic — Rectangle",
            GraphicKindSummary::Line { .. } => "Graphic — Line",
            GraphicKindSummary::Circle { .. } => "Graphic — Circle",
            GraphicKindSummary::Arc { .. } => "Graphic — Arc",
            GraphicKindSummary::Text { .. } => "Graphic — Text",
        },
    };
    col = col.push(
        container(text(header_label).size(11).color(primary))
            .padding([6, 8])
            .width(Length::Fill),
    );
    col = col.push(thin_sep(border_c));

    let prop_row_static = |label: &str, value: String| {
        container(
            row![
                text(label.to_string())
                    .size(10)
                    .color(muted)
                    .width(Length::FillPortion(2)),
                text(value)
                    .size(10)
                    .color(primary)
                    .width(Length::FillPortion(3)),
            ]
            .spacing(4),
        )
        .padding([3, 8])
        .width(Length::Fill)
    };

    match &sym.selected {
        SymbolEditorSelection::None => {
            // The .snxsym editor is a SYMBOL editor — symbol-level
            // visual / geometric properties only. Component metadata
            // (Designator / Comment / Description / Type / Parameters)
            // lives on the host ComponentRow in the component library
            // and is edited from the Library Browser / Component
            // Editor, not here.

            // Helper — labelled text_input row.
            let text_field = |label: &'a str,
                              value: &'a str,
                              placeholder: &'a str,
                              on_input: fn(String) -> PanelMsg|
             -> Element<'a, PanelMsg> {
                container(
                    row![
                        text(label)
                            .size(10)
                            .color(muted)
                            .width(Length::FillPortion(2)),
                        iced::widget::text_input(placeholder, value)
                            .padding([2, 4])
                            .size(11)
                            .on_input(on_input)
                            .width(Length::FillPortion(3)),
                    ]
                    .spacing(4)
                    .align_y(iced::Alignment::Center),
                )
                .padding([3, 8])
                .width(Length::Fill)
                .into()
            };

            // ── ▾ Symbol ──
            col = col.push(thin_sep(border_c));
            col = col.push(container(text("Symbol").size(11).color(primary)).padding([6, 8]));
            col = col.push(text_field(
                "Design Item ID",
                sym.symbol_name.as_str(),
                "Symbol name",
                PanelMsg::SymEditorSetSymbolName,
            ));
            col = col.push(prop_row_static("UUID", sym.symbol_uuid.to_string()));
            col = col.push(prop_row_static("Pins", sym.pins.len().to_string()));
            col = col.push(prop_row_static("Graphics", sym.graphics.len().to_string()));

            // Part of Parts (Altium "Part B / of Parts: 2") — surfaces
            // the multi-part picker the user already drives via the
            // toolbar arrows or the SCH Library tree-expander.
            let part_label = if sym.active_max_part > 1 {
                format!(
                    "Part {} / of Parts {}",
                    sym.active_part, sym.active_max_part
                )
            } else {
                "Single-part".to_string()
            };
            col = col.push(prop_row_static("Part", part_label));

            // ── ▾ Graphical ──
            col = col.push(thin_sep(border_c));
            col = col.push(container(text("Graphical").size(11).color(primary)).padding([6, 8]));
            let mirrored_row: Element<'a, PanelMsg> = container(
                row![
                    text("Mirrored")
                        .size(10)
                        .color(muted)
                        .width(Length::FillPortion(2)),
                    iced::widget::checkbox(sym.symbol_mirrored)
                        .size(14)
                        .on_toggle(|_| PanelMsg::SymEditorToggleSymbolMirrored),
                ]
                .spacing(4)
                .align_y(iced::Alignment::Center),
            )
            .padding([3, 8])
            .width(Length::Fill)
            .into();
            col = col.push(mirrored_row);

            // ── Local Colors (Fills / Lines / Pins) ──
            // Three click-to-cycle swatches. None = inherit from
            // sheet palette; clicking cycles through 4 preset
            // overrides + back to None. Each slot has its own
            // Set message so the dispatcher can clear / set
            // independently.
            col = col.push(local_colors_row(
                "Local Colors",
                sym.symbol_local_fill_color,
                sym.symbol_local_line_color,
                sym.symbol_local_pin_color,
                muted,
            ));
        }
        SymbolEditorSelection::Pin(pin) => {
            let pin_idx = pin.idx;

            // ── Designator (text) ──
            let designator_row: Element<'a, PanelMsg> = container(
                row![
                    text("Designator")
                        .size(10)
                        .color(muted)
                        .width(Length::FillPortion(2)),
                    iced::widget::text_input("number", pin.number.as_str())
                        .padding([2, 4])
                        .size(11)
                        .on_input(move |s| PanelMsg::SymEditorSetPinNumber { pin_idx, value: s })
                        .width(Length::FillPortion(3)),
                ]
                .spacing(4)
                .align_y(iced::Alignment::Center),
            )
            .padding([3, 8])
            .width(Length::Fill)
            .into();
            col = col.push(designator_row);

            // ── Name (text) ──
            let name_row: Element<'a, PanelMsg> = container(
                row![
                    text("Name")
                        .size(10)
                        .color(muted)
                        .width(Length::FillPortion(2)),
                    iced::widget::text_input("name", pin.name.as_str())
                        .padding([2, 4])
                        .size(11)
                        .on_input(move |s| PanelMsg::SymEditorSetPinName { pin_idx, value: s })
                        .width(Length::FillPortion(3)),
                ]
                .spacing(4)
                .align_y(iced::Alignment::Center),
            )
            .padding([3, 8])
            .width(Length::Fill)
            .into();
            col = col.push(name_row);

            // ── Electrical Type (pick_list) ──
            let electrical_options = [
                ("Input", signex_library::PinElectricalType::Input),
                ("I/O", signex_library::PinElectricalType::Bidirectional),
                ("Output", signex_library::PinElectricalType::Output),
                (
                    "Open Collector",
                    signex_library::PinElectricalType::OpenCollector,
                ),
                ("Passive", signex_library::PinElectricalType::Passive),
                ("HiZ", signex_library::PinElectricalType::Tristate),
                (
                    "Open Emitter",
                    signex_library::PinElectricalType::OpenEmitter,
                ),
                ("Power", signex_library::PinElectricalType::Power),
                (
                    "Not Connected",
                    signex_library::PinElectricalType::NotConnected,
                ),
                (
                    "Unspecified",
                    signex_library::PinElectricalType::Unspecified,
                ),
            ];
            let current_label = electrical_options
                .iter()
                .find(|(_, v)| format!("{:?}", v) == pin.electrical)
                .map(|(label, _)| label.to_string())
                .unwrap_or_else(|| pin.electrical.clone());
            let labels: Vec<String> = electrical_options
                .iter()
                .map(|(label, _)| label.to_string())
                .collect();
            let labels_for_msg: Vec<(String, signex_library::PinElectricalType)> =
                electrical_options
                    .iter()
                    .map(|(label, v)| (label.to_string(), *v))
                    .collect();
            let electrical_picker =
                iced::widget::pick_list(labels, Some(current_label), move |chosen: String| {
                    let value = labels_for_msg
                        .iter()
                        .find(|(label, _)| label == &chosen)
                        .map(|(_, v)| *v)
                        .unwrap_or(signex_library::PinElectricalType::Unspecified);
                    PanelMsg::SymEditorSetPinElectrical { pin_idx, value }
                })
                .padding([2, 4])
                .text_size(11);
            let electrical_row: Element<'a, PanelMsg> = container(
                row![
                    text("Electrical")
                        .size(10)
                        .color(muted)
                        .width(Length::FillPortion(2)),
                    container(electrical_picker).width(Length::FillPortion(3)),
                ]
                .spacing(4)
                .align_y(iced::Alignment::Center),
            )
            .padding([3, 8])
            .width(Length::Fill)
            .into();
            col = col.push(electrical_row);

            // ── Position (X, Y) ──
            let pos_x = pin.position[0];
            let pos_y = pin.position[1];
            let pos_x_row: Element<'a, PanelMsg> = container(
                row![
                    text("X")
                        .size(10)
                        .color(muted)
                        .width(Length::FillPortion(2)),
                    iced::widget::text_input("mm", &format!("{:.3}", pos_x))
                        .padding([2, 4])
                        .size(11)
                        .on_input(move |s| {
                            let parsed = s.trim().parse::<f64>().unwrap_or(pos_x);
                            PanelMsg::SymEditorSetPinX {
                                pin_idx,
                                value: parsed,
                            }
                        })
                        .width(Length::FillPortion(3)),
                ]
                .spacing(4)
                .align_y(iced::Alignment::Center),
            )
            .padding([3, 8])
            .width(Length::Fill)
            .into();
            col = col.push(pos_x_row);

            let pos_y_row: Element<'a, PanelMsg> = container(
                row![
                    text("Y")
                        .size(10)
                        .color(muted)
                        .width(Length::FillPortion(2)),
                    iced::widget::text_input("mm", &format!("{:.3}", pos_y))
                        .padding([2, 4])
                        .size(11)
                        .on_input(move |s| {
                            let parsed = s.trim().parse::<f64>().unwrap_or(pos_y);
                            PanelMsg::SymEditorSetPinY {
                                pin_idx,
                                value: parsed,
                            }
                        })
                        .width(Length::FillPortion(3)),
                ]
                .spacing(4)
                .align_y(iced::Alignment::Center),
            )
            .padding([3, 8])
            .width(Length::Fill)
            .into();
            col = col.push(pos_y_row);

            // ── Orientation (pick_list) ──
            let orientation_options = [
                ("Right", signex_library::PinOrientation::Right),
                ("Up", signex_library::PinOrientation::Up),
                ("Left", signex_library::PinOrientation::Left),
                ("Down", signex_library::PinOrientation::Down),
            ];
            let current_orient = orientation_options
                .iter()
                .find(|(_, v)| format!("{:?}", v) == pin.orientation)
                .map(|(label, _)| label.to_string())
                .unwrap_or_else(|| pin.orientation.clone());
            let orient_labels: Vec<String> = orientation_options
                .iter()
                .map(|(label, _)| label.to_string())
                .collect();
            let orient_msg_lookup: Vec<(String, signex_library::PinOrientation)> =
                orientation_options
                    .iter()
                    .map(|(label, v)| (label.to_string(), *v))
                    .collect();
            let orientation_picker = iced::widget::pick_list(
                orient_labels,
                Some(current_orient),
                move |chosen: String| {
                    let value = orient_msg_lookup
                        .iter()
                        .find(|(label, _)| label == &chosen)
                        .map(|(_, v)| *v)
                        .unwrap_or(signex_library::PinOrientation::Right);
                    PanelMsg::SymEditorSetPinOrientation { pin_idx, value }
                },
            )
            .padding([2, 4])
            .text_size(11);
            let orientation_row: Element<'a, PanelMsg> = container(
                row![
                    text("Orientation")
                        .size(10)
                        .color(muted)
                        .width(Length::FillPortion(2)),
                    container(orientation_picker).width(Length::FillPortion(3)),
                ]
                .spacing(4)
                .align_y(iced::Alignment::Center),
            )
            .padding([3, 8])
            .width(Length::Fill)
            .into();
            col = col.push(orientation_row);

            // ── Length (numeric) ──
            let length_row: Element<'a, PanelMsg> = container(
                row![
                    text("Length")
                        .size(10)
                        .color(muted)
                        .width(Length::FillPortion(2)),
                    iced::widget::text_input("mm", &format!("{:.3}", pin.length))
                        .padding([2, 4])
                        .size(11)
                        .on_input(move |s| {
                            let parsed = s.trim().parse::<f64>().unwrap_or(0.0);
                            PanelMsg::SymEditorSetPinLength {
                                pin_idx,
                                value: parsed,
                            }
                        })
                        .width(Length::FillPortion(3)),
                ]
                .spacing(4)
                .align_y(iced::Alignment::Center),
            )
            .padding([3, 8])
            .width(Length::Fill)
            .into();
            col = col.push(length_row);

            // ── Part Number (multi-part components) ──
            let part_now = pin.details.part_number;
            let part_row: Element<'a, PanelMsg> = container(
                row![
                    text("Part Number")
                        .size(10)
                        .color(muted)
                        .width(Length::FillPortion(2)),
                    iced::widget::text_input("1, or 0 for shared", &part_now.to_string(),)
                        .padding([2, 4])
                        .size(11)
                        .on_input(move |s| {
                            let parsed = s.trim().parse::<u8>().unwrap_or(part_now);
                            PanelMsg::SymEditorSetPinPartNumber {
                                pin_idx,
                                value: parsed,
                            }
                        })
                        .width(Length::FillPortion(3)),
                ]
                .spacing(4)
                .align_y(iced::Alignment::Center),
            )
            .padding([3, 8])
            .width(Length::Fill)
            .into();
            col = col.push(part_row);

            // ── Description (text) ──
            let description_row: Element<'a, PanelMsg> = container(
                row![
                    text("Description")
                        .size(10)
                        .color(muted)
                        .width(Length::FillPortion(2)),
                    iced::widget::text_input("free text", pin.details.description.as_str())
                        .padding([2, 4])
                        .size(11)
                        .on_input(move |s| PanelMsg::SymEditorSetPinDescription {
                            pin_idx,
                            value: s,
                        })
                        .width(Length::FillPortion(3)),
                ]
                .spacing(4)
                .align_y(iced::Alignment::Center),
            )
            .padding([3, 8])
            .width(Length::Fill)
            .into();
            col = col.push(description_row);

            // ── Function (comma-separated alt-names) ──
            let function_csv = pin.details.function.join(", ");
            let function_row: Element<'a, PanelMsg> = container(
                row![
                    text("Function")
                        .size(10)
                        .color(muted)
                        .width(Length::FillPortion(2)),
                    iced::widget::text_input("alt-names, comma-separated", &function_csv)
                        .padding([2, 4])
                        .size(11)
                        .on_input(move |s| PanelMsg::SymEditorSetPinFunctionCsv {
                            pin_idx,
                            value: s,
                        })
                        .width(Length::FillPortion(3)),
                ]
                .spacing(4)
                .align_y(iced::Alignment::Center),
            )
            .padding([3, 8])
            .width(Length::Fill)
            .into();
            col = col.push(function_row);

            // ── Visibility / state toggles ──
            let toggle_row =
                |label: &'static str, value: bool, msg: PanelMsg| -> Element<'a, PanelMsg> {
                    row![
                        iced::widget::checkbox(value)
                            .size(14)
                            .on_toggle(move |_| msg.clone()),
                        text(label.to_string()).size(10).color(muted),
                    ]
                    .spacing(6)
                    .align_y(iced::Alignment::Center)
                    .into()
                };
            let toggles_row: Element<'a, PanelMsg> = container(
                column![
                    toggle_row(
                        "Designator visible",
                        pin.details.designator_visible,
                        PanelMsg::SymEditorTogglePinDesignatorVisible(pin_idx),
                    ),
                    toggle_row(
                        "Name visible",
                        pin.details.name_visible,
                        PanelMsg::SymEditorTogglePinNameVisible(pin_idx),
                    ),
                    toggle_row(
                        "Hidden (Pin Hide)",
                        pin.details.hidden,
                        PanelMsg::SymEditorTogglePinHidden(pin_idx),
                    ),
                    toggle_row(
                        "Locked",
                        pin.details.locked,
                        PanelMsg::SymEditorTogglePinLocked(pin_idx),
                    ),
                ]
                .spacing(2),
            )
            .padding([6, 8])
            .width(Length::Fill)
            .into();
            col = col.push(toggles_row);

            // ── IEEE Symbols (Inside / Inside Edge / Outside Edge / Outside) ──
            col = col.push(thin_sep(border_c));
            col = col.push(container(text("Symbols").size(10).color(primary)).padding([4, 8]));
            col = col.push(view_pin_symbol_picker(
                "Inside",
                pin.details.inside_symbol,
                pin_idx,
                0,
                muted,
            ));
            col = col.push(view_pin_symbol_picker(
                "Inside Edge",
                pin.details.inside_edge_symbol,
                pin_idx,
                1,
                muted,
            ));
            col = col.push(view_pin_symbol_picker(
                "Outside Edge",
                pin.details.outside_edge_symbol,
                pin_idx,
                2,
                muted,
            ));
            col = col.push(view_pin_symbol_picker(
                "Outside",
                pin.details.outside_symbol,
                pin_idx,
                3,
                muted,
            ));
        }
        SymbolEditorSelection::Graphic(g) => {
            let g_idx = g.idx;
            // Per-shape numeric fields. Each invokes
            // `PanelMsg::SymEditorSetGraphicField`; mismatched fields
            // (e.g. CircleRadius on a Line) silently no-op in the
            // dispatcher so a stale Properties pane can't corrupt
            // geometry.
            let num_field =
                |label: &'static str, field: GraphicFieldId, value: f64| -> Element<'a, PanelMsg> {
                    container(
                        row![
                            text(label.to_string())
                                .size(10)
                                .color(muted)
                                .width(Length::FillPortion(2)),
                            iced::widget::text_input("mm", &format!("{:.3}", value))
                                .padding([2, 4])
                                .size(11)
                                .on_input(move |s| {
                                    let parsed = s.trim().parse::<f64>().unwrap_or(value);
                                    PanelMsg::SymEditorSetGraphicField {
                                        idx: g_idx,
                                        field,
                                        value: parsed,
                                    }
                                })
                                .width(Length::FillPortion(3)),
                        ]
                        .spacing(4)
                        .align_y(iced::Alignment::Center),
                    )
                    .padding([3, 8])
                    .width(Length::Fill)
                    .into()
                };

            match &g.kind {
                GraphicKindSummary::Rectangle { from, to } => {
                    col = col.push(num_field("From X", GraphicFieldId::FromX, from[0]));
                    col = col.push(num_field("From Y", GraphicFieldId::FromY, from[1]));
                    col = col.push(num_field("To X", GraphicFieldId::ToX, to[0]));
                    col = col.push(num_field("To Y", GraphicFieldId::ToY, to[1]));
                }
                GraphicKindSummary::Line { from, to } => {
                    col = col.push(num_field("Start X", GraphicFieldId::FromX, from[0]));
                    col = col.push(num_field("Start Y", GraphicFieldId::FromY, from[1]));
                    col = col.push(num_field("End X", GraphicFieldId::ToX, to[0]));
                    col = col.push(num_field("End Y", GraphicFieldId::ToY, to[1]));
                }
                GraphicKindSummary::Circle { center, radius } => {
                    col = col.push(num_field("Center X", GraphicFieldId::CenterX, center[0]));
                    col = col.push(num_field("Center Y", GraphicFieldId::CenterY, center[1]));
                    col = col.push(num_field("Radius", GraphicFieldId::Radius, *radius));
                }
                GraphicKindSummary::Arc {
                    center,
                    radius,
                    start_deg,
                    end_deg,
                } => {
                    col = col.push(num_field("Center X", GraphicFieldId::CenterX, center[0]));
                    col = col.push(num_field("Center Y", GraphicFieldId::CenterY, center[1]));
                    col = col.push(num_field("Radius", GraphicFieldId::Radius, *radius));
                    col = col.push(num_field(
                        "Start \u{00B0}",
                        GraphicFieldId::StartDeg,
                        *start_deg,
                    ));
                    col = col.push(num_field("End \u{00B0}", GraphicFieldId::EndDeg, *end_deg));
                }
                GraphicKindSummary::Text {
                    position,
                    content,
                    size: text_size,
                } => {
                    col = col.push(num_field("X", GraphicFieldId::PositionX, position[0]));
                    col = col.push(num_field("Y", GraphicFieldId::PositionY, position[1]));
                    col = col.push(num_field("Size", GraphicFieldId::TextSize, *text_size));
                    let content_row: Element<'a, PanelMsg> = container(
                        row![
                            text("Content")
                                .size(10)
                                .color(muted)
                                .width(Length::FillPortion(2)),
                            iced::widget::text_input("text", content.as_str())
                                .padding([2, 4])
                                .size(11)
                                .on_input(move |s| PanelMsg::SymEditorSetGraphicText {
                                    idx: g_idx,
                                    value: s,
                                })
                                .width(Length::FillPortion(3)),
                        ]
                        .spacing(4)
                        .align_y(iced::Alignment::Center),
                    )
                    .padding([3, 8])
                    .width(Length::Fill)
                    .into();
                    col = col.push(content_row);
                }
            }
            // Stroke width — common to every variant.
            col = col.push(num_field(
                "Stroke (mm)",
                GraphicFieldId::StrokeWidth,
                g.stroke_width,
            ));
        }
        SymbolEditorSelection::FieldReference => {
            col = col.push(prop_row_static(
                "Field",
                "Reference (designator)".to_string(),
            ));
            col = col.push(
                container(
                    text("Bound to the host Component's designator at place-time.")
                        .size(10)
                        .color(muted),
                )
                .padding([4, 8]),
            );
        }
        SymbolEditorSelection::FieldValue => {
            col = col.push(prop_row_static("Field", "Value".to_string()));
            col = col.push(
                container(
                    text("Bound to the host Component's value at place-time.")
                        .size(10)
                        .color(muted),
                )
                .padding([4, 8]),
            );
        }
    }

    col = col.push(thin_sep(border_c));
    col = col.push(
        container(
            text(
                "Click on the canvas to select. Drawing tools (rectangle, line, arc) land in v0.9 phase 3c.",
            )
            .size(10)
            .color(muted),
        )
        .padding([6, 8]),
    );

    scrollable(col).width(Length::Fill).into()
}

/// Altium-style context-aware properties for a single selected element.
/// Shows EDITABLE fields for symbols, labels, and text notes.
fn view_selected_element_properties<'a>(
    ctx: &'a PanelContext,
    muted: Color,
    primary: Color,
    border_c: Color,
) -> Element<'a, PanelMsg> {
    let input_bg = crate::styles::ti(ctx.tokens.selection);
    let input_bdr = crate::styles::ti(ctx.tokens.accent);
    let mut col: Column<'a, PanelMsg> = Column::new().spacing(0).width(Length::Fill);

    let elem_type = ctx
        .selection_info
        .iter()
        .find(|(k, _)| k == "Type")
        .map(|(_, v)| v.as_str())
        .unwrap_or("Object");

    let uuid = ctx.selected_uuid;
    let selected_kind = ctx.selected_kind;
    let get = |key: &str| -> String {
        ctx.selection_info
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    };

    // ── Header ──
    col = col.push(
        container(text(elem_type.to_owned()).size(11).color(primary))
            .padding([6, 8])
            .width(Length::Fill),
    );
    col = col.push(thin_sep(border_c));

    // ── Editable properties based on element type ──
    // Power ports get their own Altium-style panel; regular symbols keep the
    // existing designator/value/footprint layout.
    let is_power_port = elem_type == "Power Port";

    if is_power_port && let Some(id) = uuid {
        let value = get("Value");
        let position = get("Position");
        let rotation_str = get("Rotation");
        let lib_id = get("Library ID");
        let rotation_deg = rotation_str
            .trim_end_matches('°')
            .trim()
            .parse::<f64>()
            .unwrap_or(0.0);
        // Pick a Style token by looking inside the lib_id (same logic the
        // built-in power renderer uses).
        let lid = lib_id.to_lowercase();
        let current_style =
            if lid.contains("gnd") && !lid.contains("earth") && !lid.contains("gndref") {
                "Power Ground"
            } else if lid.contains("gndref") {
                "Signal Ground"
            } else if lid.contains("earth") {
                "Earth"
            } else if lid.contains("arrow") {
                "Arrow"
            } else if lid.contains("wave") {
                "Wave"
            } else if lid.contains("circle") {
                "Circle"
            } else {
                "Bar"
            };
        let style_options: Vec<String> = [
            "Bar",
            "Arrow",
            "Wave",
            "Circle",
            "Power Ground",
            "Signal Ground",
            "Earth",
        ]
        .iter()
        .map(|s| (*s).to_string())
        .collect();

        // ── Location ──
        let pos_loc = position.clone();
        let rot_current = rotation_deg;
        col = col.push(collapsible_section(
            "sel_location",
            "Location",
            &ctx.collapsed_sections,
            primary,
            border_c,
            move || {
                let mut c = Column::new().spacing(0).width(Length::Fill);
                c = c.push(form_input_row(
                    "(X/Y)", &pos_loc, muted, input_bg, input_bdr,
                ));
                let rotation_opts: Vec<String> = vec![
                    "0 Degrees".into(),
                    "90 Degrees".into(),
                    "180 Degrees".into(),
                    "270 Degrees".into(),
                ];
                let rot_label = format!("{:.0} Degrees", rot_current);
                c = c.push(form_pick_row(
                    "Rotation",
                    rotation_opts,
                    rot_label,
                    move |s| {
                        let deg = s
                            .split_whitespace()
                            .next()
                            .and_then(|n| n.parse::<f64>().ok())
                            .unwrap_or(0.0);
                        PanelMsg::EditSymbolRotation(id, deg)
                    },
                    muted,
                ));
                c
            },
        ));

        // ── Properties (Name, Style) ──
        let name_val = value.clone();
        let base_lib = lib_id.clone();
        col = col.push(collapsible_section(
            "sel_props",
            "Properties",
            &ctx.collapsed_sections,
            primary,
            border_c,
            move || {
                let mut c = Column::new().spacing(0).width(Length::Fill);
                c = c.push(form_edit_row("Name", &name_val, muted, move |s| {
                    PanelMsg::EditSymbolValue(id, s)
                }));
                let base_lib = base_lib.clone();
                let current_rot = rot_current;
                c = c.push(form_pick_row(
                    "Style",
                    style_options,
                    current_style.to_string(),
                    move |style_label| {
                        // Map Altium Style label → lib_id keyword the built-in
                        // power renderer recognizes.
                        let tag = match style_label.as_str() {
                            "Bar" => "bar",
                            "Arrow" => "arrow",
                            "Wave" => "wave",
                            "Circle" => "circle",
                            "Power Ground" => "GND",
                            "Signal Ground" => "GNDREF",
                            "Earth" => "Earth",
                            _ => "bar",
                        };
                        let new_lib = format!("power:{tag}");
                        // The built-in renderer flips body direction when
                        // lib_id contains "gnd". Compensate rotation so the
                        // port keeps its current visual orientation: if we
                        // are switching between gnd-like and non-gnd-like,
                        // rotate by +180° from current, else keep current.
                        let old_gnd = base_lib.to_lowercase().contains("gnd")
                            && !base_lib.to_lowercase().contains("earth");
                        let new_gnd = new_lib.to_lowercase().contains("gnd")
                            && !new_lib.to_lowercase().contains("earth");
                        let target_rot = if old_gnd != new_gnd {
                            (current_rot + 180.0).rem_euclid(360.0)
                        } else {
                            current_rot
                        };
                        PanelMsg::EditPowerPortStyle {
                            symbol_id: id,
                            new_lib_id: new_lib,
                            rotation_degrees: target_rot,
                        }
                    },
                    muted,
                ));
                c
            },
        ));

        // Add Font + B/I/U/T row to Properties section via a second collapsible
        col = col.push(collapsible_section(
            "sel_props_font",
            "Font",
            &ctx.collapsed_sections,
            primary,
            border_c,
            move || {
                let mut c = Column::new().spacing(0).width(Length::Fill);
                let font_opts: Vec<String> = vec![
                    "Iosevka Fixed SS03".into(),
                    "Roboto".into(),
                    "Fira Code".into(),
                    "Arial".into(),
                    "Times New Roman".into(),
                ];
                c = c.push(form_pick_row(
                    "Font",
                    font_opts,
                    "Iosevka Fixed SS03".to_string(),
                    |_| PanelMsg::Noop,
                    muted,
                ));
                let size_opts: Vec<String> = [6, 8, 10, 12, 14, 16, 18, 20, 24, 28, 36, 48, 72]
                    .iter()
                    .map(|n| n.to_string())
                    .collect();
                c = c.push(form_pick_row(
                    "Size",
                    size_opts,
                    "10".to_string(),
                    move |s| {
                        let pt: u32 = s.parse().unwrap_or(10);
                        PanelMsg::EditSymbolValueFontSizePt(id, pt)
                    },
                    muted,
                ));
                c = c.push(font_style_row(muted, primary, input_bg, input_bdr));
                c
            },
        ));

        // ── General (Net) — informational ──
        let phys_name = value.clone();
        col = col.push(collapsible_section(
            "sel_net",
            "General (Net)",
            &ctx.collapsed_sections,
            primary,
            border_c,
            move || {
                let mut c = Column::new().spacing(0).width(Length::Fill);
                c = c.push(form_input_row(
                    "Physical Name",
                    &phys_name,
                    muted,
                    input_bg,
                    input_bdr,
                ));
                c = c.push(form_input_row(
                    "Net Name", &phys_name, muted, input_bg, input_bdr,
                ));
                c = c.push(net_numeric_row(
                    "Power Net",
                    "0.000",
                    "V",
                    muted,
                    input_bg,
                    input_bdr,
                ));
                c = c.push(net_numeric_row(
                    "High Speed",
                    "0.000",
                    "Hz",
                    muted,
                    input_bg,
                    input_bdr,
                ));
                let dp_opts: Vec<String> = vec!["None".into()];
                c = c.push(form_pick_row(
                    "Differential Pair",
                    dp_opts,
                    "None".to_string(),
                    |_| PanelMsg::Noop,
                    muted,
                ));
                c
            },
        ));

        // ── Parameters (Net) — placeholder (no parameters/rules/classes yet) ──
        col = col.push(collapsible_section(
            "sel_net_params",
            "Parameters (Net)",
            &ctx.collapsed_sections,
            primary,
            border_c,
            move || {
                let mut c = Column::new().spacing(0).width(Length::Fill);
                c = c.push(net_params_tabs(primary, muted, input_bg, input_bdr));
                c = c.push(net_params_header(muted, border_c));
                c = c.push(empty_section_row("No Parameters", muted, border_c));
                c = c.push(empty_section_row("No Rules", muted, border_c));
                c = c.push(empty_section_row("No Classes", muted, border_c));
                c = c.push(net_params_add_bar(muted, input_bg, input_bdr));
                c
            },
        ));

        return scrollable(col).width(Length::Fill).into();
    }

    match selected_kind {
        Some(signex_types::schematic::SelectedKind::Symbol) => {
            let reference = get("Reference");
            let value = get("Value");
            let description = get("Description");
            let datasheet = get("Datasheet");
            let footprint = get("Footprint");
            let lib_id = get("Library ID");
            let position = get("Position");
            let rotation = get("Rotation");
            let locked = get("Locked") == "Yes";
            let dnp = get("DNP") == "Yes";
            let has_mirror_x = ctx
                .selection_info
                .iter()
                .any(|(k, v)| k == "Mirror" && v == "X");
            let has_mirror_y = ctx
                .selection_info
                .iter()
                .any(|(k, v)| k == "Mirror" && v == "Y");
            // Custom parameters: every ("Param: NAME", value) tuple.
            let params: Vec<(String, String)> = ctx
                .selection_info
                .iter()
                .filter_map(|(k, v)| {
                    k.strip_prefix("Param: ")
                        .map(|name| (name.to_string(), v.clone()))
                })
                .collect();

            if let Some(id) = uuid {
                // General section — editable
                col = col.push(collapsible_section(
                    "sel_general",
                    "General",
                    &ctx.collapsed_sections,
                    muted,
                    border_c,
                    move || {
                        let mut c = Column::new().spacing(0).width(Length::Fill);
                        c = c.push(form_edit_row("Designator", &reference, muted, move |s| {
                            PanelMsg::EditSymbolDesignator(id, s)
                        }));
                        c = c.push(form_edit_row("Value", &value, muted, move |s| {
                            PanelMsg::EditSymbolValue(id, s)
                        }));
                        if !description.is_empty() {
                            c = c.push(form_input_row(
                                "Description",
                                &description,
                                muted,
                                input_bg,
                                input_bdr,
                            ));
                        }
                        c = c.push(form_edit_row("Footprint", &footprint, muted, move |s| {
                            PanelMsg::EditSymbolFootprint(id, s)
                        }));
                        if !datasheet.is_empty() {
                            c = c.push(form_input_row(
                                "Datasheet",
                                &datasheet,
                                muted,
                                input_bg,
                                input_bdr,
                            ));
                        }
                        c = c.push(form_input_row(
                            "Library ID",
                            &lib_id,
                            muted,
                            input_bg,
                            input_bdr,
                        ));
                        c
                    },
                ));

                // Location section — read-only for now
                col = col.push(collapsible_section(
                    "sel_location",
                    "Location",
                    &ctx.collapsed_sections,
                    muted,
                    border_c,
                    move || {
                        let mut c = Column::new().spacing(0).width(Length::Fill);
                        c = c.push(form_input_row(
                            "Position", &position, muted, input_bg, input_bdr,
                        ));
                        c = c.push(form_input_row(
                            "Rotation", &rotation, muted, input_bg, input_bdr,
                        ));
                        c
                    },
                ));

                // Graphical section — checkboxes
                col = col.push(collapsible_section(
                    "sel_graphical",
                    "Graphical",
                    &ctx.collapsed_sections,
                    muted,
                    border_c,
                    move || {
                        let mut c = Column::new().spacing(0).width(Length::Fill);
                        c = c.push(form_check_row(
                            "Mirror X",
                            has_mirror_x,
                            PanelMsg::ToggleSymbolMirrorX(id),
                            muted,
                        ));
                        c = c.push(form_check_row(
                            "Mirror Y",
                            has_mirror_y,
                            PanelMsg::ToggleSymbolMirrorY(id),
                            muted,
                        ));
                        c = c.push(form_check_row(
                            "Locked",
                            locked,
                            PanelMsg::ToggleSymbolLocked(id),
                            muted,
                        ));
                        c = c.push(form_check_row(
                            "DNP",
                            dnp,
                            PanelMsg::ToggleSymbolDnp(id),
                            muted,
                        ));
                        c
                    },
                ));

                // Parameters section — custom fields carried on the symbol
                // instance. Read-only for v0.6; editing per-field lands in
                // v0.7 with the parameter-manager dialog.
                let header_label = if params.is_empty() {
                    "Parameters (none)".to_string()
                } else {
                    format!("Parameters ({})", params.len())
                };
                let section_params = params.clone();
                col = col.push(collapsible_section(
                    "sel_parameters",
                    &header_label,
                    &ctx.collapsed_sections,
                    muted,
                    border_c,
                    move || {
                        let mut c = Column::new().spacing(0).width(Length::Fill);
                        if section_params.is_empty() {
                            c = c.push(
                                container(
                                    text("No custom parameters".to_string())
                                        .size(11)
                                        .color(muted),
                                )
                                .padding([6, 8]),
                            );
                        } else {
                            for (name, value) in &section_params {
                                c = c.push(form_input_row(name, value, muted, input_bg, input_bdr));
                            }
                        }
                        c
                    },
                ));
            }
        }
        Some(signex_types::schematic::SelectedKind::SymbolRefField)
        | Some(signex_types::schematic::SelectedKind::SymbolValField) => {
            let text_value = get("Text");
            let position = get("Position");
            let rotation = get("Rotation");
            let text_size = get("Text Size");
            let justify_h = get("Justify H");
            let justify_v = get("Justify V");
            let visible = get("Visible");
            let fields_autoplaced = get("Fields Autoplaced");
            let is_reference = matches!(
                selected_kind,
                Some(signex_types::schematic::SelectedKind::SymbolRefField)
            );

            if let Some(id) = uuid {
                col = col.push(collapsible_section(
                    "sel_basic",
                    "Basic Properties",
                    &ctx.collapsed_sections,
                    muted,
                    border_c,
                    move || {
                        let mut c = Column::new().spacing(0).width(Length::Fill);
                        c = c.push(form_input_row(
                            "Field",
                            if is_reference { "Reference" } else { "Value" },
                            muted,
                            input_bg,
                            input_bdr,
                        ));
                        c = c.push(form_edit_row("Text", &text_value, muted, move |s| {
                            if is_reference {
                                PanelMsg::EditSymbolDesignator(id, s)
                            } else {
                                PanelMsg::EditSymbolValue(id, s)
                            }
                        }));
                        c = c.push(form_input_row(
                            "Visible", &visible, muted, input_bg, input_bdr,
                        ));
                        c = c.push(form_input_row(
                            "Fields Autoplaced",
                            &fields_autoplaced,
                            muted,
                            input_bg,
                            input_bdr,
                        ));
                        c
                    },
                ));

                col = col.push(collapsible_section(
                    "sel_text",
                    "Text Properties",
                    &ctx.collapsed_sections,
                    muted,
                    border_c,
                    move || {
                        let mut c = Column::new().spacing(0).width(Length::Fill);
                        c = c.push(form_input_row(
                            "Position", &position, muted, input_bg, input_bdr,
                        ));
                        c = c.push(form_input_row(
                            "Rotation", &rotation, muted, input_bg, input_bdr,
                        ));
                        c = c.push(form_input_row(
                            "Justify H",
                            &justify_h,
                            muted,
                            input_bg,
                            input_bdr,
                        ));
                        c = c.push(form_input_row(
                            "Justify V",
                            &justify_v,
                            muted,
                            input_bg,
                            input_bdr,
                        ));
                        c = c.push(form_input_row(
                            "Text Size",
                            &text_size,
                            muted,
                            input_bg,
                            input_bdr,
                        ));
                        c
                    },
                ));
            }
        }
        Some(signex_types::schematic::SelectedKind::Label) => {
            // Net Name stored in Standard escapes `/` as `{slash}`. Show the
            // visible form in the panel; the edit handler re-escapes on save.
            let label_text = signex_render::schematic::text::expand_char_escapes(&get("Text"));
            let position = get("Position");
            let rotation_str = get("Rotation");
            let text_size_str = get("Text Size");
            let justify_h_str = get("Justify H");

            // Parse numeric values for edit controls (with fallbacks).
            let rotation_deg = rotation_str
                .trim_end_matches('°')
                .trim()
                .parse::<f64>()
                .unwrap_or(0.0);
            let text_size_pt = text_size_str.parse::<u32>().unwrap_or(10);
            let justify_h = match justify_h_str.as_str() {
                "Left" => signex_types::schematic::HAlign::Left,
                "Right" => signex_types::schematic::HAlign::Right,
                _ => signex_types::schematic::HAlign::Center,
            };

            if let Some(id) = uuid {
                // ── Location ──
                let pos_clone = position.clone();
                let rot_current = rotation_deg;
                col = col.push(collapsible_section(
                    "sel_location",
                    "Location",
                    &ctx.collapsed_sections,
                    primary,
                    border_c,
                    move || {
                        let mut c = Column::new().spacing(0).width(Length::Fill);
                        c = c.push(form_input_row(
                            "(X/Y)", &pos_clone, muted, input_bg, input_bdr,
                        ));
                        let rotation_opts: Vec<String> = vec![
                            "0 Degrees".into(),
                            "90 Degrees".into(),
                            "180 Degrees".into(),
                            "270 Degrees".into(),
                        ];
                        let rot_label = format!("{:.0} Degrees", rot_current);
                        c = c.push(form_pick_row(
                            "Rotation",
                            rotation_opts,
                            rot_label,
                            move |s| {
                                let deg = s
                                    .split_whitespace()
                                    .next()
                                    .and_then(|n| n.parse::<f64>().ok())
                                    .unwrap_or(0.0);
                                PanelMsg::EditLabelRotation(id, deg)
                            },
                            muted,
                        ));
                        c
                    },
                ));

                // ── Properties (Net Name, Font, Justification) ──
                let net_name = label_text.clone();
                col = col.push(collapsible_section(
                    "sel_props",
                    "Properties",
                    &ctx.collapsed_sections,
                    primary,
                    border_c,
                    move || {
                        let mut c = Column::new().spacing(0).width(Length::Fill);
                        c = c.push(form_edit_row("Net Name", &net_name, muted, move |s| {
                            PanelMsg::EditLabelText(id, s)
                        }));
                        // Font family + size + color (color + family are cosmetic for now)
                        let font_opts: Vec<String> = crate::fonts::system_font_families().clone();
                        let default_font = font_opts
                            .iter()
                            .find(|f| f.to_lowercase().contains("iosevka"))
                            .cloned()
                            .unwrap_or_else(|| font_opts.first().cloned().unwrap_or_default());
                        c = c.push(form_pick_row(
                            "Font",
                            font_opts,
                            default_font,
                            |_| PanelMsg::Noop,
                            muted,
                        ));
                        let size_opts: Vec<String> =
                            [6, 8, 10, 12, 14, 16, 18, 20, 24, 28, 36, 48, 72]
                                .iter()
                                .map(|n| n.to_string())
                                .collect();
                        c = c.push(form_pick_row(
                            "Font Size",
                            size_opts,
                            text_size_pt.to_string(),
                            move |s| {
                                let pt: u32 = s.parse().unwrap_or(10);
                                PanelMsg::EditLabelFontSizePt(id, pt)
                            },
                            muted,
                        ));
                        // B/I/U/T row — cosmetic for now.
                        c = c.push(font_style_row(muted, primary, input_bg, input_bdr));
                        // 3x3 Justification grid — Altium's 9-point anchor picker.
                        c = c.push(form_label("Justification", muted));
                        c = c.push(
                            container(justification_grid(
                                id,
                                rotation_deg,
                                justify_h,
                                input_bg,
                                input_bdr,
                                primary,
                                muted,
                                ctx.theme_id,
                            ))
                            .padding([4, 8]),
                        );
                        c
                    },
                ));
            }
        }
        Some(signex_types::schematic::SelectedKind::TextNote) => {
            let note_text = get("Text");
            let position = get("Position");
            let rotation = get("Rotation");
            let text_size = get("Text Size");
            let justify_h = get("Justify H");
            let justify_v = get("Justify V");

            if let Some(id) = uuid {
                col = col.push(collapsible_section(
                    "sel_basic",
                    "Basic Properties",
                    &ctx.collapsed_sections,
                    muted,
                    border_c,
                    move || {
                        let mut c = Column::new().spacing(0).width(Length::Fill);
                        c = c.push(form_edit_row("Text", &note_text, muted, move |s| {
                            PanelMsg::EditTextNoteText(id, s)
                        }));
                        c
                    },
                ));

                col = col.push(collapsible_section(
                    "sel_text",
                    "Text Properties",
                    &ctx.collapsed_sections,
                    muted,
                    border_c,
                    move || {
                        let mut c = Column::new().spacing(0).width(Length::Fill);
                        c = c.push(form_input_row(
                            "Position", &position, muted, input_bg, input_bdr,
                        ));
                        c = c.push(form_input_row(
                            "Rotation", &rotation, muted, input_bg, input_bdr,
                        ));
                        c = c.push(form_input_row(
                            "Justify H",
                            &justify_h,
                            muted,
                            input_bg,
                            input_bdr,
                        ));
                        c = c.push(form_input_row(
                            "Justify V",
                            &justify_v,
                            muted,
                            input_bg,
                            input_bdr,
                        ));
                        c = c.push(form_input_row(
                            "Text Size",
                            &text_size,
                            muted,
                            input_bg,
                            input_bdr,
                        ));
                        c
                    },
                ));
            }
        }
        Some(signex_types::schematic::SelectedKind::Drawing) => {
            col = col.push(view_drawing_properties(ctx, muted, primary, border_c));
        }
        Some(signex_types::schematic::SelectedKind::ChildSheet) => {
            col = col.push(view_child_sheet_properties(ctx, muted, primary, border_c));
        }
        _ => {
            // Generic read-only properties for other types
            let info: Vec<(String, String)> = ctx
                .selection_info
                .iter()
                .filter(|(k, _)| k != "Type")
                .cloned()
                .collect();
            col = col.push(collapsible_section(
                "sel_general",
                "Properties",
                &ctx.collapsed_sections,
                muted,
                border_c,
                || {
                    let mut c = Column::new().spacing(0).width(Length::Fill);
                    for (key, value) in &info {
                        c = c.push(prop_kv_row(key, value, muted, primary));
                    }
                    c
                },
            ));
        }
    }

    // ── Status bar ──
    col = col.push(Space::new().height(8.0));
    col = col.push(thin_sep(border_c));
    col = col.push(container(text("1 object selected").size(10).color(muted)).padding([4, 8]));

    scrollable(col).width(Length::Fill).into()
}

/// Pre-placement properties — shown when TAB pressed during a placement tool.
fn view_pre_placement<'a>(
    pp: &PrePlacementData,
    ctx: &'a PanelContext,
    muted: Color,
    primary: Color,
    border_c: Color,
    input_bg: Color,
    input_bdr: Color,
) -> Element<'a, PanelMsg> {
    let label_text = pp.label_text.clone();
    let designator = pp.designator.clone();
    let rotation = pp.rotation;
    let tool_name = pp.tool_name.clone();
    let kind = pp.kind;
    let pos_str = format!("{:.2}, {:.2}", pp.cursor_x_mm, pp.cursor_y_mm);
    let rot_label = format!("{:.0} Degrees", rotation);
    let font = pp.font.clone();
    let font_size_pt = pp.font_size_pt;
    let justify_h = pp.justify_h;

    let mut col: Column<'a, PanelMsg> = Column::new().spacing(0).width(Length::Fill);

    // Header — kind-labelled, Altium style.
    col = col.push(
        container(
            row![
                text(tool_name.clone()).size(12).color(primary),
                Space::new().width(Length::Fill),
                iced::widget::button(text("OK").size(10).color(Color::WHITE))
                    .padding([2, 10])
                    .on_press(PanelMsg::ConfirmPrePlacement)
                    .style(iced::widget::button::primary),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding([6, 8])
        .width(Length::Fill),
    );

    col = col.push(container(Space::new()).height(1).width(Length::Fill).style(
        move |_: &Theme| container::Style {
            background: Some(Background::Color(border_c)),
            ..container::Style::default()
        },
    ));

    // ── Location ──
    col = col.push(collapsible_section(
        "preplace_location",
        "Location",
        &ctx.collapsed_sections,
        primary,
        border_c,
        move || {
            let mut c = Column::new().spacing(0).width(Length::Fill);
            c = c.push(form_input_row(
                "(X/Y)", &pos_str, muted, input_bg, input_bdr,
            ));
            let rotation_opts: Vec<String> = vec![
                "0 Degrees".into(),
                "90 Degrees".into(),
                "180 Degrees".into(),
                "270 Degrees".into(),
            ];
            c = c.push(form_pick_row(
                "Rotation",
                rotation_opts,
                rot_label.clone(),
                |s| {
                    let deg = s
                        .split_whitespace()
                        .next()
                        .and_then(|n| n.parse::<f64>().ok())
                        .unwrap_or(0.0);
                    PanelMsg::SetPrePlacementRotation(deg)
                },
                muted,
            ));
            c
        },
    ));

    // ── Properties (kind-specific) ──
    let text_label_for_kind = match kind {
        PrePlacementKind::NetLabel => "Net Name",
        PrePlacementKind::GlobalPort => "Port Name",
        PrePlacementKind::HierPort => "Sheet Name",
        PrePlacementKind::PowerPort => "Net Name",
        PrePlacementKind::TextNote => "Text",
        PrePlacementKind::Component => "Value",
        _ => "",
    };

    let show_text_field = !text_label_for_kind.is_empty();
    let show_designator = matches!(kind, PrePlacementKind::Component);
    let show_text_styling = matches!(
        kind,
        PrePlacementKind::NetLabel
            | PrePlacementKind::GlobalPort
            | PrePlacementKind::HierPort
            | PrePlacementKind::PowerPort
            | PrePlacementKind::TextNote
            | PrePlacementKind::Component
    );

    if show_text_field || show_text_styling {
        col = col.push(collapsible_section(
            "preplace_props",
            "Properties",
            &ctx.collapsed_sections,
            primary,
            border_c,
            move || {
                let mut c = Column::new().spacing(0).width(Length::Fill);
                if show_text_field {
                    c = c.push(form_edit_row(
                        text_label_for_kind,
                        &label_text,
                        muted,
                        PanelMsg::SetPrePlacementText,
                    ));
                }
                if show_designator {
                    c = c.push(form_edit_row(
                        "Designator",
                        &designator,
                        muted,
                        PanelMsg::SetPrePlacementDesignator,
                    ));
                }
                if show_text_styling {
                    let font_opts: Vec<String> = crate::fonts::system_font_families().clone();
                    c = c.push(form_pick_row(
                        "Font",
                        font_opts,
                        font.clone(),
                        PanelMsg::SetPrePlacementFont,
                        muted,
                    ));
                    let size_opts: Vec<String> = [6, 8, 10, 12, 14, 16, 18, 20, 24, 28, 36, 48, 72]
                        .iter()
                        .map(|n| n.to_string())
                        .collect();
                    c = c.push(form_pick_row(
                        "Font Size",
                        size_opts,
                        font_size_pt.to_string(),
                        |s| PanelMsg::SetPrePlacementFontSize(s.parse().unwrap_or(10)),
                        muted,
                    ));
                    c = c.push(font_style_row(muted, primary, input_bg, input_bdr));
                    c = c.push(form_label("Justification", muted));
                    c = c.push(
                        container(preplacement_justification_grid(
                            justify_h,
                            input_bg,
                            input_bdr,
                            primary,
                            muted,
                            ctx.theme_id,
                        ))
                        .padding([4, 8]),
                    );
                }
                c
            },
        ));
    } else if matches!(
        kind,
        PrePlacementKind::Line
            | PrePlacementKind::Rectangle
            | PrePlacementKind::Circle
            | PrePlacementKind::Arc
            | PrePlacementKind::Polygon
    ) {
        // Shape tools — Altium-style Width + Fill so users can
        // preconfigure the next placement via TAB.
        let width = pp.shape_width_mm;
        let fill = pp.shape_fill;
        let show_fill = !matches!(kind, PrePlacementKind::Line | PrePlacementKind::Arc);
        col = col.push(collapsible_section(
            "preplace_shape",
            "Properties",
            &ctx.collapsed_sections,
            primary,
            border_c,
            move || {
                let mut c = Column::new().spacing(0).width(Length::Fill);
                c = c.push(form_edit_row_f64(
                    "Width (mm)",
                    width,
                    muted,
                    PanelMsg::SetPrePlacementShapeWidth,
                ));
                if show_fill {
                    c = c.push(shape_fill_row(fill, muted, border_c));
                }
                c
            },
        ));
    } else {
        col = col.push(
            container(
                text("Click to place. No per-instance options.")
                    .size(10)
                    .color(muted),
            )
            .padding([8, PROPERTY_ROW_PAD_X]),
        );
    }

    container(scrollable(col).width(Length::Fill))
        .width(Length::Fill)
        .into()
}

/// Numeric edit row used by the shape pre-placement form. Writes on
/// submit — partial text mid-type doesn't panic via parse failure.
fn form_edit_row_f64<'a>(
    label: &'a str,
    value: f64,
    muted: Color,
    on_submit: impl Fn(f64) -> PanelMsg + 'a + Clone,
) -> Element<'a, PanelMsg> {
    use iced::widget::{row, text, text_input};
    let buf = format!("{value:.3}");
    let on_submit_cb = on_submit.clone();
    row![
        text(label)
            .size(10)
            .color(muted)
            .width(Length::FillPortion(2)),
        text_input("", &buf)
            .size(11)
            .on_input(move |s| {
                if let Ok(v) = s.parse::<f64>() {
                    on_submit_cb(v)
                } else {
                    PanelMsg::Noop
                }
            })
            .width(Length::FillPortion(3)),
    ]
    .padding([4, PROPERTY_ROW_PAD_X])
    .spacing(6)
    .align_y(iced::Alignment::Center)
    .into()
}

fn shape_fill_row<'a>(
    current: signex_types::schematic::FillType,
    muted: Color,
    _border_c: Color,
) -> Element<'a, PanelMsg> {
    use iced::widget::{button, row, text};
    use signex_types::schematic::FillType;
    let tile = |label: &'static str, ft: FillType, active: bool| -> Element<'a, PanelMsg> {
        button(text(label).size(10))
            .padding([3, 8])
            .on_press(PanelMsg::SetPrePlacementShapeFill(ft))
            .style(move |_: &iced::Theme, _| iced::widget::button::Style {
                background: Some(iced::Background::Color(if active {
                    Color::from_rgb(0.20, 0.36, 0.58)
                } else {
                    Color::from_rgba(0.25, 0.25, 0.28, 0.4)
                })),
                border: iced::Border {
                    width: 1.0,
                    radius: 3.0.into(),
                    color: Color::from_rgb(0.28, 0.28, 0.32),
                },
                text_color: if active {
                    Color::from_rgb(1.0, 1.0, 1.0)
                } else {
                    muted
                },
                ..iced::widget::button::Style::default()
            })
            .into()
    };
    row![
        text("Fill")
            .size(10)
            .color(muted)
            .width(Length::FillPortion(2)),
        row![
            tile("None", FillType::None, current == FillType::None),
            tile("Outline", FillType::Outline, current == FillType::Outline),
            tile(
                "Background",
                FillType::Background,
                current == FillType::Background
            ),
        ]
        .spacing(4)
        .width(Length::FillPortion(3)),
    ]
    .padding([4, PROPERTY_ROW_PAD_X])
    .spacing(6)
    .align_y(iced::Alignment::Center)
    .into()
}

fn view_properties_general<'a>(
    ctx: &'a PanelContext,
    muted: Color,
    primary: Color,
    border_c: Color,
) -> Column<'a, PanelMsg> {
    // Derive button/input colors from tokens (Copy values captured in closures)
    let input_bg = crate::styles::ti(ctx.tokens.selection); // deep blue tint
    let input_bdr = crate::styles::ti(ctx.tokens.accent);
    let tag_hover = {
        let c = crate::styles::ti(ctx.tokens.accent);
        Color {
            r: (c.r * 1.3).min(1.0),
            g: (c.g * 1.3).min(1.0),
            b: (c.b * 1.3).min(1.0),
            ..c
        }
    };
    let seg_hover = crate::styles::ti(ctx.tokens.hover);

    let mut col: Column<'a, PanelMsg> = Column::new().spacing(0).width(Length::Fill);

    // Custom Selection Filters (collapsible) — tabbed editor for up to
    // CUSTOM_FILTER_PRESET_LIMIT named presets that also surface as
    // shortcut buttons in the Active Bar's filter dropdown.
    {
        use crate::active_bar::{CUSTOM_FILTER_PRESET_LIMIT, SelectionFilter};
        let presets = ctx.custom_filter_presets.clone();
        let active_tab = ctx
            .active_custom_filter_tab
            .min(presets.len().saturating_sub(1));
        let muted_c = muted;
        let primary_c = primary;
        // Border colour for tabs and member chips — matches the Active
        // Bar Filter dropdown's chip border treatment so the section
        // reads as one cohesive piece.
        let accent_c = crate::styles::ti(ctx.tokens.accent);
        col = col.push(collapsible_section(
            "prop_sel_filter",
            "Custom Selection Filters",
            &ctx.collapsed_sections,
            primary,
            border_c,
            move || {
                let mut c = Column::new().spacing(6).width(Length::Fill);
                if presets.is_empty() {
                    c = c.push(
                        container(
                            text("No presets yet. Click + to define one.")
                                .size(11)
                                .color(muted_c),
                        )
                        .padding([4, 8]),
                    );
                    c = c.push(
                        container(
                            iced::widget::button(text("+ Add Filter").size(11).color(primary_c))
                                .padding([4, 10])
                                .on_press(PanelMsg::AddCustomFilterPreset),
                        )
                        .padding([4, 8]),
                    );
                    return c;
                }
                // Tab strip: one tab per preset + a trailing "+" tab
                // when room remains. Each tab is its own button — the
                // active one gets the accent border, others a muted one.
                let mut tabs = iced::widget::Row::new()
                    .spacing(2)
                    .align_y(iced::Alignment::Center);
                for (idx, preset) in presets.iter().enumerate() {
                    let label = if preset.name.trim().is_empty() {
                        format!("Filter {}", idx + 1)
                    } else {
                        preset.name.clone()
                    };
                    tabs = tabs.push(custom_filter_tab(
                        label,
                        idx == active_tab,
                        idx,
                        tag_hover,
                        accent_c,
                    ));
                }
                if presets.len() < CUSTOM_FILTER_PRESET_LIMIT {
                    tabs = tabs.push(
                        iced::widget::button(text("+").size(12).color(primary_c))
                            .padding([3, 10])
                            .on_press(PanelMsg::AddCustomFilterPreset),
                    );
                }
                c = c.push(container(tabs).padding([4, 8]));
                // Active tab body — name input + chips + delete.
                let preset = &presets[active_tab];
                let included: std::collections::HashSet<SelectionFilter> =
                    preset.filters.iter().copied().collect();
                let header = row![
                    iced::widget::text_input("Preset name", &preset.name)
                        .size(11)
                        .padding([3, 6])
                        .on_input(move |s| PanelMsg::RenameCustomFilterPreset(active_tab, s))
                        .width(Length::Fill),
                    iced::widget::button(text("Delete").size(10).color(primary_c))
                        .padding([3, 8])
                        .on_press(PanelMsg::RemoveCustomFilterPreset(active_tab)),
                ]
                .spacing(4)
                .align_y(iced::Alignment::Center);
                let mut wrap = Wrap::new().spacing(4.0).line_spacing(4.0);
                for &f in SelectionFilter::ALL {
                    wrap = wrap.push(preset_chip(
                        f.label(),
                        active_tab,
                        f,
                        included.contains(&f),
                        tag_hover,
                        accent_c,
                    ));
                }
                c = c.push(
                    container(column![header, container(wrap).padding([4, 0])].spacing(4))
                        .padding([6, 8]),
                );
                c
            },
        ));
    }

    // General (collapsible)
    {
        let unit = ctx.unit;
        let grid_size_mm = ctx.grid_size_mm;
        let visible_grid_mm = ctx.visible_grid_mm;
        let snap_enabled = ctx.snap_enabled;
        let snap_hotspots = ctx.snap_hotspots;
        let grid_visible = ctx.grid_visible;
        let canvas_font_name = ctx.canvas_font_name.clone();
        let canvas_font_size = ctx.canvas_font_size;
        let canvas_font_bold = ctx.canvas_font_bold;
        let canvas_font_italic = ctx.canvas_font_italic;
        let canvas_font_popup_open = ctx.canvas_font_popup_open;
        let sheet_color = ctx.sheet_color;
        col = col.push(collapsible_section(
            "prop_general",
            "General",
            &ctx.collapsed_sections,
            primary,
            border_c,
            move || {
                let mut c = Column::new().spacing(0).width(Length::Fill);
                c = c.push(form_label("Units", muted));
                c = c.push(
                    container(
                        row![
                            seg_btn(
                                "mm",
                                unit == Unit::Mm,
                                PanelMsg::SetUnit(Unit::Mm),
                                input_bg,
                                primary,
                                muted,
                                seg_hover,
                                input_bdr
                            ),
                            seg_btn(
                                "mils",
                                unit == Unit::Mil,
                                PanelMsg::SetUnit(Unit::Mil),
                                input_bg,
                                primary,
                                muted,
                                seg_hover,
                                input_bdr
                            ),
                        ]
                        .spacing(0.0)
                        .width(Length::Fill),
                    )
                    .padding([2, 8]),
                );
                // Altium-style: Visible Grid and Snap Grid are independent
                c = c.push(form_grid_row(
                    "Visible Grid",
                    visible_grid_mm,
                    unit,
                    false,
                    PanelMsg::SetVisibleGridSize,
                    muted,
                    grid_visible,
                    PanelMsg::ToggleGrid,
                ));
                c = c.push(form_grid_row(
                    "Snap Grid",
                    grid_size_mm,
                    unit,
                    true,
                    PanelMsg::SetGridSize,
                    muted,
                    snap_enabled,
                    PanelMsg::ToggleSnap,
                ));
                c = c.push(form_check_row_shortcut(
                    "Snap to Hotspots",
                    snap_hotspots,
                    PanelMsg::ToggleSnapHotspots,
                    "Shift+E",
                    muted,
                ));
                c = c.push(form_font_link_row(
                    "Canvas Font",
                    &canvas_font_name,
                    canvas_font_size,
                    canvas_font_bold,
                    canvas_font_italic,
                    muted,
                ));
                if canvas_font_popup_open {
                    c = c.push(
                        container(canvas_font_popup(
                            &canvas_font_name,
                            canvas_font_size,
                            canvas_font_bold,
                            canvas_font_italic,
                            muted,
                            input_bg,
                            input_bdr,
                        ))
                        .padding(iced::Padding {
                            top: 0.0,
                            right: 16.0,
                            bottom: 4.0,
                            left: 8.0,
                        }),
                    );
                }
                let sheet_colors: Vec<SheetColor> = SheetColor::ALL.to_vec();
                c = c.push(form_pick_row(
                    "Sheet Color",
                    sheet_colors,
                    sheet_color,
                    PanelMsg::SetSheetColor,
                    muted,
                ));
                c
            },
        ));
    }

    // Page Options (collapsible)
    {
        let paper_size = ctx.paper_size.clone();
        let format_mode = ctx.page_format_mode;
        let margin_v = ctx.margin_vertical;
        let margin_h = ctx.margin_horizontal;
        let origin = ctx.page_origin;
        let custom_w = ctx.custom_paper_w_mm;
        let custom_h = ctx.custom_paper_h_mm;
        col = col.push(collapsible_section(
            "prop_page_opts",
            "Page Options",
            &ctx.collapsed_sections,
            primary,
            border_c,
            move || {
                let mut c = Column::new().spacing(0).width(Length::Fill);
                c = c.push(form_label("Formatting and Size", muted));
                c = c.push(
                    container(
                        row![
                            seg_btn(
                                "Template",
                                format_mode == PageFormatMode::Template,
                                PanelMsg::SetPageFormatMode(PageFormatMode::Template),
                                input_bg,
                                primary,
                                muted,
                                seg_hover,
                                input_bdr,
                            ),
                            seg_btn(
                                "Standard",
                                format_mode == PageFormatMode::Standard,
                                PanelMsg::SetPageFormatMode(PageFormatMode::Standard),
                                input_bg,
                                primary,
                                muted,
                                seg_hover,
                                input_bdr,
                            ),
                            seg_btn(
                                "Custom",
                                format_mode == PageFormatMode::Custom,
                                PanelMsg::SetPageFormatMode(PageFormatMode::Custom),
                                input_bg,
                                primary,
                                muted,
                                seg_hover,
                                input_bdr,
                            ),
                        ]
                        .spacing(0.0)
                        .width(Length::Fill),
                    )
                    .padding([2, 8]),
                );
                // Standard + Template modes share the size picker. Custom mode replaces
                // it with width/height inputs.
                match format_mode {
                    PageFormatMode::Standard | PageFormatMode::Template => {
                        let paper_options: Vec<String> =
                            PAPER_SIZES.iter().map(|s| (*s).to_string()).collect();
                        c = c.push(form_pick_row(
                            "Paper",
                            paper_options,
                            paper_size.clone(),
                            PanelMsg::SetPaperSize,
                            muted,
                        ));
                        let (w, h) = paper_dimensions(&paper_size);
                        let dims = format!("Width: {w:.0}mm  Height: {h:.0}mm");
                        c = c.push(container(text(dims).size(10).color(muted)).padding([3, 8]));
                        if matches!(format_mode, PageFormatMode::Template) {
                            c = c.push(
                                container(
                                    text("Template: using A-series defaults")
                                        .size(10)
                                        .color(muted),
                                )
                                .padding([0, 8]),
                            );
                        }
                    }
                    PageFormatMode::Custom => {
                        c = c.push(form_mm_edit_row(
                            "Width",
                            custom_w,
                            PanelMsg::SetCustomPaperWidth,
                            muted,
                            input_bg,
                            input_bdr,
                        ));
                        c = c.push(form_mm_edit_row(
                            "Height",
                            custom_h,
                            PanelMsg::SetCustomPaperHeight,
                            muted,
                            input_bg,
                            input_bdr,
                        ));
                    }
                }
                c = c.push(form_label("Margin and Zones", muted));
                c = c.push(form_int_edit_row(
                    "Vertical",
                    margin_v,
                    PanelMsg::SetMarginVertical,
                    muted,
                    input_bg,
                    input_bdr,
                ));
                c = c.push(form_int_edit_row(
                    "Horizontal",
                    margin_h,
                    PanelMsg::SetMarginHorizontal,
                    muted,
                    input_bg,
                    input_bdr,
                ));
                let origin_opts: Vec<PageOrigin> =
                    vec![PageOrigin::UpperLeft, PageOrigin::LowerLeft];
                c = c.push(form_pick_row(
                    "Origin",
                    origin_opts,
                    origin,
                    PanelMsg::SetPageOrigin,
                    muted,
                ));
                c
            },
        ));
    }

    col
}

fn view_properties_parameters<'a>(
    muted: Color,
    primary: Color,
    border_c: Color,
    input_bg: Color,
    input_bdr: Color,
    seg_hover: Color,
) -> Column<'a, PanelMsg> {
    let mut col: Column<'a, PanelMsg> = Column::new().spacing(0).width(Length::Fill);

    col = col.push(section_hdr("\u{25BC} Parameters", primary, border_c));

    // Sub-tabs: All | Parameters | Rules
    col = col.push(
        container(
            row![
                seg_btn(
                    "All",
                    false,
                    PanelMsg::PropertiesTab(1),
                    input_bg,
                    primary,
                    muted,
                    seg_hover,
                    input_bdr
                ),
                seg_btn(
                    "Parameters",
                    true,
                    PanelMsg::PropertiesTab(1),
                    input_bg,
                    primary,
                    muted,
                    seg_hover,
                    input_bdr
                ),
                seg_btn(
                    "Rules",
                    false,
                    PanelMsg::PropertiesTab(1),
                    input_bg,
                    primary,
                    muted,
                    seg_hover,
                    input_bdr
                ),
            ]
            .spacing(0.0)
            .width(Length::Fill),
        )
        .padding([4, 8]),
    );

    // Table header
    col = col.push(thin_sep(border_c));
    col = col.push(
        container(
            row![
                text("Name")
                    .size(10)
                    .color(muted)
                    .width(Length::FillPortion(3)),
                text("Value")
                    .size(10)
                    .color(muted)
                    .width(Length::FillPortion(2)),
            ]
            .spacing(4.0),
        )
        .padding([4, 8])
        .width(Length::Fill),
    );
    col = col.push(thin_sep(border_c));

    // Parameter rows (standard Altium document parameters)
    let params: &[(&str, &str)] = &[
        ("CurrentTime", "*"),
        ("CurrentDate", "*"),
        ("Time", "*"),
        ("Date", "*"),
        ("DocumentFullPathAndName", "*"),
        ("DocumentName", "*"),
        ("ModifiedDate", "*"),
        ("ApprovedBy", "*"),
        ("CheckedBy", "*"),
        ("Author", "*"),
        ("CompanyName", "*"),
        ("DrawnBy", "*"),
        ("Engineer", "*"),
        ("Organization", "*"),
        ("Title", "*"),
        ("Address1", "*"),
        ("Address2", "*"),
        ("Address3", "*"),
        ("Address4", "*"),
    ];

    for (name, val) in params {
        col = col.push(param_table_row(name, val, primary, muted, border_c));
    }

    col
}

/// Parameter table row with subtle bottom border.
fn param_table_row<'a, M: 'a>(
    name: &str,
    value: &str,
    name_c: Color,
    val_c: Color,
    border_c: Color,
) -> Element<'a, M> {
    column![
        container(
            row![
                text(name.to_string())
                    .size(11)
                    .color(name_c)
                    .width(Length::FillPortion(3))
                    .wrapping(iced::widget::text::Wrapping::None),
                text(value.to_string())
                    .size(11)
                    .color(val_c)
                    .width(Length::FillPortion(2)),
            ]
            .spacing(4.0),
        )
        .padding([4, 8])
        .width(Length::Fill),
        thin_sep(border_c),
    ]
    .spacing(0)
    .into()
}

/// Properties panel tab button (General / Parameters).
fn props_tab_btn(
    label: &str,
    active: bool,
    msg: PanelMsg,
    text_active: Color,
    text_inactive: Color,
    hover_bg: Color,
    border_c: Color,
) -> Element<'static, PanelMsg> {
    let text_c = if active { text_active } else { text_inactive };
    iced::widget::button(text(label.to_string()).size(11).color(text_c))
        .padding([4, 12])
        .on_press(msg)
        .style(move |_: &Theme, status: iced::widget::button::Status| {
            let hover = matches!(status, iced::widget::button::Status::Hovered);
            iced::widget::button::Style {
                background: if active || hover {
                    Some(Background::Color(hover_bg))
                } else {
                    None
                },
                border: Border {
                    width: 1.0,
                    radius: 2.0.into(),
                    color: border_c,
                },
                ..iced::widget::button::Style::default()
            }
        })
        .into()
}

/// Thin 1px separator line.
fn thin_sep<'a, M: 'a>(border_c: Color) -> Element<'a, M> {
    container(Space::new())
        .height(1.0)
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(border_c)),
            ..container::Style::default()
        })
        .into()
}

/// Section header: bold label + separator line.
fn section_hdr<'a, M: 'a>(title: &str, text_c: Color, border_c: Color) -> Column<'a, M> {
    column![
        container(text(title.to_string()).size(12).color(text_c))
            .padding([6, 8])
            .width(Length::Fill),
        container(Space::new())
            .height(1.0)
            .width(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(border_c)),
                ..container::Style::default()
            }),
    ]
    .spacing(0)
}

/// Form row: label | styled input-like value display.
fn form_input_row<'a, M: 'a>(
    label: &str,
    value: &str,
    label_c: Color,
    input_bg: Color,
    input_border: Color,
) -> Element<'a, M> {
    container(
        row![
            text(label.to_string())
                .size(11)
                .color(label_c)
                .width(Length::FillPortion(PROPERTY_LABEL_PORTION))
                .wrapping(iced::widget::text::Wrapping::None),
            container(
                text(value.to_string())
                    .size(11)
                    .color(Color::WHITE)
                    .wrapping(iced::widget::text::Wrapping::None),
            )
            .padding([3, 6])
            .width(Length::FillPortion(PROPERTY_CONTROL_PORTION))
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(input_bg)),
                border: Border {
                    width: 1.0,
                    radius: 2.0.into(),
                    color: input_border,
                },
                ..container::Style::default()
            }),
        ]
        .spacing(8.0)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .into()
}

/// Form row: label | integer text_input (no spinner buttons).
fn form_int_edit_row<'a>(
    label: &str,
    value: u32,
    on_change: impl Fn(u32) -> PanelMsg + 'a + Clone,
    label_c: Color,
    input_bg: Color,
    input_border: Color,
) -> Element<'a, PanelMsg> {
    let text_value = value.to_string();
    let on_change_cl = on_change.clone();
    container(
        row![
            text(label.to_string())
                .size(11)
                .color(label_c)
                .width(Length::FillPortion(PROPERTY_LABEL_PORTION))
                .wrapping(iced::widget::text::Wrapping::None),
            iced::widget::text_input("", &text_value)
                .on_input(move |s| {
                    let parsed: u32 = s.trim().parse().unwrap_or(0);
                    (on_change_cl)(parsed.min(99))
                })
                .size(11)
                .padding([3, 6])
                .width(Length::FillPortion(PROPERTY_CONTROL_PORTION))
                .style(move |_: &Theme, _| iced::widget::text_input::Style {
                    background: Background::Color(input_bg),
                    border: Border {
                        width: 1.0,
                        radius: 2.0.into(),
                        color: input_border,
                    },
                    icon: Color::TRANSPARENT,
                    placeholder: Color::from_rgba8(0x66, 0x6A, 0x7E, 1.0),
                    value: Color::WHITE,
                    selection: Color::from_rgba8(0x4D, 0x52, 0x66, 0.6),
                }),
        ]
        .spacing(8.0)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .into()
}

/// Form row: label | floating-point mm text_input (no spinner buttons).
fn form_mm_edit_row<'a>(
    label: &str,
    value: f32,
    on_change: impl Fn(f32) -> PanelMsg + 'a + Clone,
    label_c: Color,
    input_bg: Color,
    input_border: Color,
) -> Element<'a, PanelMsg> {
    let text_value = format!("{value:.1}");
    let on_change_cl = on_change.clone();
    container(
        row![
            text(label.to_string())
                .size(11)
                .color(label_c)
                .width(Length::FillPortion(PROPERTY_LABEL_PORTION))
                .wrapping(iced::widget::text::Wrapping::None),
            iced::widget::text_input("", &text_value)
                .on_input(move |s| {
                    let parsed: f32 = s.trim().parse().unwrap_or(0.0);
                    (on_change_cl)(parsed.clamp(1.0, 2000.0))
                })
                .size(11)
                .padding([3, 6])
                .width(Length::FillPortion(PROPERTY_CONTROL_PORTION))
                .style(move |_: &Theme, _| iced::widget::text_input::Style {
                    background: Background::Color(input_bg),
                    border: Border {
                        width: 1.0,
                        radius: 2.0.into(),
                        color: input_border,
                    },
                    icon: Color::TRANSPARENT,
                    placeholder: Color::from_rgba8(0x66, 0x6A, 0x7E, 1.0),
                    value: Color::WHITE,
                    selection: Color::from_rgba8(0x4D, 0x52, 0x66, 0.6),
                }),
        ]
        .spacing(8.0)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .into()
}

/// Form row: label | pick_list (dropdown).
fn form_pick_row<'a, T>(
    label: &str,
    options: Vec<T>,
    selected: T,
    on_change: impl Fn(T) -> PanelMsg + 'a,
    label_c: Color,
) -> Element<'a, PanelMsg>
where
    T: Clone + Eq + std::fmt::Display + 'static,
{
    container(
        row![
            text(label.to_string())
                .size(11)
                .color(label_c)
                .width(Length::FillPortion(PROPERTY_LABEL_PORTION))
                .wrapping(iced::widget::text::Wrapping::None),
            iced::widget::pick_list(options, Some(selected), on_change)
                .text_size(11)
                .padding([2, 6])
                .width(Length::FillPortion(PROPERTY_CONTROL_PORTION)),
        ]
        .spacing(8.0)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .into()
}

/// Form row: label | custom widget.
#[allow(dead_code)]
fn form_label_row<'a>(
    label: &str,
    control: Row<'a, PanelMsg>,
    label_c: Color,
) -> Element<'a, PanelMsg> {
    container(
        row![
            text(label.to_string())
                .size(11)
                .color(label_c)
                .width(Length::FillPortion(PROPERTY_LABEL_PORTION))
                .wrapping(iced::widget::text::Wrapping::None),
            container(control).width(Length::FillPortion(PROPERTY_CONTROL_PORTION)),
        ]
        .spacing(8.0)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .into()
}

/// Checkbox form row.
fn form_check_row<'a>(
    label: &str,
    checked: bool,
    msg: PanelMsg,
    label_c: Color,
) -> Element<'a, PanelMsg> {
    container(
        row![
            text(label.to_string())
                .size(11)
                .color(label_c)
                .width(Length::FillPortion(PROPERTY_LABEL_PORTION))
                .wrapping(iced::widget::text::Wrapping::None),
            row![
                iced::widget::checkbox(checked)
                    .on_toggle(move |_| msg.clone())
                    .size(14)
                    .spacing(4),
                text(if checked { "On" } else { "Off" })
                    .size(11)
                    .color(if checked { Color::WHITE } else { label_c }),
            ]
            .spacing(8)
            .width(Length::FillPortion(PROPERTY_CONTROL_PORTION)),
        ]
        .spacing(8.0)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .into()
}

/// Form row: label | pick_list for grid size presets (2.54 mm multiples).
#[allow(dead_code)]
fn form_grid_size_row(current_mm: f32, label_c: Color) -> Element<'static, PanelMsg> {
    use crate::canvas::grid::{GRID_SIZE_LABELS, GRID_SIZES_MM};
    // Find the label that matches the current value (fallback to first).
    let selected: Option<&'static str> = GRID_SIZES_MM
        .iter()
        .zip(GRID_SIZE_LABELS.iter())
        .find(|(sz, _)| (**sz - current_mm).abs() < 1e-4)
        .map(|(_, lbl)| *lbl);
    container(
        row![
            text("Visible Grid".to_string())
                .size(11)
                .color(label_c)
                .width(LABEL_W)
                .wrapping(iced::widget::text::Wrapping::None),
            iced::widget::pick_list(GRID_SIZE_LABELS, selected, |lbl: &'static str| {
                // Map label back to mm value
                let mm = GRID_SIZES_MM
                    .iter()
                    .zip(GRID_SIZE_LABELS.iter())
                    .find(|(_, l)| **l == lbl)
                    .map(|(v, _)| *v)
                    .unwrap_or(2.54);
                PanelMsg::SetGridSize(mm)
            },)
            .text_size(11)
            .width(Length::Fill),
        ]
        .spacing(8.0)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, 8])
    .width(Length::Fill)
    .into()
}

/// Altium-style grid row: [Label] [checkbox toggle] [pick_list] [shortcut hint]
/// Used for both "Visible Grid" (eye/visible toggle) and "Snap Grid" (snap enable toggle).
/// Labels and values are shown in the current `unit` (mm or mil).
#[allow(clippy::too_many_arguments)]
fn form_grid_row(
    label: &'static str,
    current_mm: f32,
    unit: Unit,
    has_checkbox: bool,
    on_size: impl Fn(f32) -> PanelMsg + 'static,
    label_c: Color,
    active: bool,
    on_toggle: PanelMsg,
) -> Element<'static, PanelMsg> {
    use crate::canvas::grid::{GRID_SIZE_LABELS, GRID_SIZE_LABELS_MIL, GRID_SIZES_MM};

    let labels: &'static [&'static str] = if unit == Unit::Mil {
        GRID_SIZE_LABELS_MIL
    } else {
        GRID_SIZE_LABELS
    };

    let selected: Option<&'static str> = GRID_SIZES_MM
        .iter()
        .zip(labels.iter())
        .find(|(sz, _)| (**sz - current_mm).abs() < 1e-4)
        .map(|(_, lbl)| *lbl);

    let pick = iced::widget::pick_list(labels, selected, move |lbl: &'static str| {
        // Map label back to mm value (labels and GRID_SIZES_MM are parallel arrays)
        let mm = GRID_SIZE_LABELS
            .iter()
            .chain(GRID_SIZE_LABELS_MIL.iter())
            .zip(GRID_SIZES_MM.iter().chain(GRID_SIZES_MM.iter()))
            .find(|(l, _)| **l == lbl)
            .map(|(_, v)| *v)
            .unwrap_or(2.54);
        on_size(mm)
    })
    .text_size(11)
    .width(Length::Fill);

    let label_widget = text(label.to_string())
        .size(11)
        .color(label_c)
        .width(Length::FillPortion(PROPERTY_LABEL_PORTION))
        .wrapping(iced::widget::text::Wrapping::None);

    let content: Element<PanelMsg> = if has_checkbox {
        // Snap Grid row: checkbox before pick_list
        row![
            label_widget,
            iced::widget::checkbox(active)
                .on_toggle(move |_| on_toggle.clone())
                .size(12)
                .spacing(4),
            container(pick).width(Length::FillPortion(PROPERTY_CONTROL_PORTION)),
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center)
        .into()
    } else {
        // Visible Grid row: no checkbox
        row![
            label_widget,
            container(pick).width(Length::FillPortion(PROPERTY_CONTROL_PORTION))
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center)
        .into()
    };

    container(content)
        .padding([2, PROPERTY_ROW_PAD_X])
        .width(Length::Fill)
        .into()
}

/// Form row: checkbox with a keyboard shortcut hint on the right.
fn form_check_row_shortcut<'a>(
    label: &'a str,
    value: bool,
    on_toggle: PanelMsg,
    shortcut: &'a str,
    label_c: Color,
) -> Element<'a, PanelMsg> {
    let shortcut_owned = shortcut.to_string();
    container(
        row![
            text(label.to_string())
                .size(11)
                .color(label_c)
                .width(Length::FillPortion(PROPERTY_LABEL_PORTION))
                .wrapping(iced::widget::text::Wrapping::Word),
            row![
                iced::widget::checkbox(value)
                    .on_toggle(move |_| on_toggle.clone())
                    .size(12)
                    .spacing(4),
                Space::new().width(Length::Fill),
                text(shortcut_owned)
                    .size(9)
                    .color(label_c)
                    .wrapping(iced::widget::text::Wrapping::None),
            ]
            .spacing(4)
            .width(Length::FillPortion(PROPERTY_CONTROL_PORTION)),
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .clip(true)
    .into()
}

fn form_font_link_row<'a>(
    label: &'static str,
    current_family: &str,
    current_size_px: f32,
    _bold: bool,
    _italic: bool,
    label_c: Color,
) -> Element<'a, PanelMsg> {
    let summary = format!("{current_family}, {:.0}px", current_size_px);

    container(
        row![
            text(label)
                .size(11)
                .color(label_c)
                .width(Length::FillPortion(PROPERTY_LABEL_PORTION))
                .wrapping(iced::widget::text::Wrapping::None),
            iced::widget::button(
                text(summary)
                    .size(11)
                    .color(Color::from_rgb(0.35, 0.7, 1.0))
                    .width(Length::Fill),
            )
            .on_press(PanelMsg::OpenCanvasFontPopup)
            .padding([1, 0])
            .width(Length::FillPortion(PROPERTY_CONTROL_PORTION))
            .style(move |_: &Theme, status: iced::widget::button::Status| {
                let underline = matches!(status, iced::widget::button::Status::Hovered);
                iced::widget::button::Style {
                    background: None,
                    text_color: if underline {
                        Color::from_rgb(0.55, 0.82, 1.0)
                    } else {
                        Color::from_rgb(0.35, 0.7, 1.0)
                    },
                    border: Border::default(),
                    shadow: iced::Shadow::default(),
                    ..Default::default()
                }
            }),
        ]
        .spacing(4)
        .width(Length::Fill)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .into()
}

fn canvas_font_popup<'a>(
    current_family: &str,
    current_size_px: f32,
    bold: bool,
    italic: bool,
    label_c: Color,
    input_bg: Color,
    input_bdr: Color,
) -> Element<'a, PanelMsg> {
    let families = crate::fonts::system_font_families();
    let family_pick = iced::widget::pick_list(
        families.as_slice(),
        Some(current_family.to_string()),
        PanelMsg::SetCanvasFont,
    )
    .text_size(11)
    .width(Length::Fill);

    let size_input = NumberInput::new(&current_size_px, 6.0..=36.0, PanelMsg::SetCanvasFontSize)
        .step(1.0)
        .width(Length::Fill)
        .padding(4);

    container(
        column![
            row![
                text("Canvas Font Settings").size(11).color(label_c),
                Space::new().width(Length::Fill),
                iced::widget::button(text("Close").size(10))
                    .on_press(PanelMsg::CloseCanvasFontPopup)
                    .padding([2, 6])
            ]
            .align_y(iced::Alignment::Center),
            row![
                text("Family").size(10).color(label_c).width(56),
                family_pick,
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
            row![text("Size").size(10).color(label_c).width(56), size_input,]
                .spacing(8)
                .align_y(iced::Alignment::Center),
            row![
                text("Style").size(10).color(label_c).width(56),
                row![
                    iced::widget::checkbox(bold)
                        .on_toggle(PanelMsg::SetCanvasFontBold)
                        .size(12)
                        .spacing(4),
                    text("Bold").size(10).color(label_c),
                ]
                .spacing(4)
                .align_y(iced::Alignment::Center),
                row![
                    iced::widget::checkbox(italic)
                        .on_toggle(PanelMsg::SetCanvasFontItalic)
                        .size(12)
                        .spacing(4),
                    text("Italic").size(10).color(label_c),
                ]
                .spacing(4)
                .align_y(iced::Alignment::Center),
            ]
            .spacing(12)
            .align_y(iced::Alignment::Center),
            text("Applies immediately to canvas text rendering.")
                .size(9)
                .color(label_c),
        ]
        .spacing(6),
    )
    .padding([6, 8])
    .style(move |_: &Theme| iced::widget::container::Style {
        background: Some(Background::Color(input_bg)),
        border: Border {
            color: input_bdr,
            width: 1.0,
            radius: 4.0.into(),
        },
        ..Default::default()
    })
    .width(Length::Fill)
    .into()
}

/// Form row: label | NumberInput (iced_aw) with step/bounds.
#[allow(dead_code)]
fn form_number_row<'a, T>(
    label: &str,
    value: T,
    bounds: impl std::ops::RangeBounds<T> + 'a,
    step: T,
    on_change: impl Fn(T) -> PanelMsg + 'static + Clone,
    label_c: Color,
) -> Element<'a, PanelMsg>
where
    T: num_traits::Num
        + num_traits::NumAssignOps
        + PartialOrd
        + std::fmt::Display
        + std::str::FromStr
        + Clone
        + num_traits::Bounded
        + 'static,
{
    container(
        row![
            text(label.to_string())
                .size(11)
                .color(label_c)
                .width(Length::FillPortion(PROPERTY_LABEL_PORTION))
                .wrapping(iced::widget::text::Wrapping::None),
            NumberInput::new(&value, bounds, on_change)
                .step(step)
                .width(Length::FillPortion(PROPERTY_CONTROL_PORTION))
                .padding(4),
        ]
        .spacing(8.0)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .into()
}

/// Form row: label | editable text_input that emits a PanelMsg on input.
fn form_edit_row<'a>(
    label: &str,
    value: &str,
    label_c: Color,
    on_input: impl Fn(String) -> PanelMsg + 'a,
) -> Element<'a, PanelMsg> {
    container(
        row![
            text(label.to_string())
                .size(11)
                .color(label_c)
                .width(Length::FillPortion(PROPERTY_LABEL_PORTION))
                .wrapping(iced::widget::text::Wrapping::None),
            iced::widget::text_input("", value)
                .on_input(on_input)
                .size(11)
                .padding(4)
                .width(Length::FillPortion(PROPERTY_CONTROL_PORTION)),
        ]
        .spacing(8.0)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .into()
}

/// Standalone label row (no value, used before segmented controls).
fn form_label<'a, M: 'a>(label: &str, label_c: Color) -> Element<'a, M> {
    container(text(label.to_string()).size(11).color(label_c))
        .padding([4, 8])
        .width(Length::Fill)
        .into()
}

/// Altium-style B/I/U/T (Bold / Italic / Underline / Strikethrough) row.
fn font_style_row<'a>(
    _label_c: Color,
    primary: Color,
    input_bg: Color,
    input_bdr: Color,
) -> Element<'a, PanelMsg> {
    let btn = |glyph: &'static str, style: iced::font::Weight| -> Element<'static, PanelMsg> {
        iced::widget::button(
            text(glyph.to_string())
                .size(12)
                .color(primary)
                .font(iced::Font {
                    weight: style,
                    ..iced::Font::DEFAULT
                })
                .align_x(iced::alignment::Horizontal::Center),
        )
        .width(Length::Fill)
        .padding([4, 6])
        .on_press(PanelMsg::Noop)
        .style(move |_: &Theme, status: iced::widget::button::Status| {
            let hovered = matches!(status, iced::widget::button::Status::Hovered);
            iced::widget::button::Style {
                background: Some(Background::Color(if hovered {
                    input_bdr
                } else {
                    input_bg
                })),
                border: Border {
                    width: 1.0,
                    radius: 2.0.into(),
                    color: input_bdr,
                },
                text_color: primary,
                ..iced::widget::button::Style::default()
            }
        })
        .into()
    };
    container(
        row![
            btn("B", iced::font::Weight::Bold),
            btn("I", iced::font::Weight::Normal),
            btn("U", iced::font::Weight::Normal),
            btn("T", iced::font::Weight::Normal),
        ]
        .spacing(2.0)
        .width(Length::Fill),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .into()
}

/// Net-attribute row: label | checkbox | text value | unit. Used for
/// "Power Net = 0.000 V" and "High Speed = 0.000 Hz".
fn net_numeric_row<'a>(
    label: &str,
    value: &str,
    unit: &str,
    label_c: Color,
    input_bg: Color,
    input_bdr: Color,
) -> Element<'a, PanelMsg> {
    container(
        row![
            text(label.to_string())
                .size(11)
                .color(label_c)
                .width(Length::FillPortion(PROPERTY_LABEL_PORTION))
                .wrapping(iced::widget::text::Wrapping::None),
            iced::widget::checkbox(false)
                .on_toggle(|_| PanelMsg::Noop)
                .size(12)
                .spacing(4),
            container(text(value.to_string()).size(11).color(label_c),)
                .padding([3, 6])
                .width(Length::Fill)
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(input_bg)),
                    border: Border {
                        width: 1.0,
                        radius: 2.0.into(),
                        color: input_bdr
                    },
                    ..container::Style::default()
                }),
            text(unit.to_string()).size(10).color(label_c),
        ]
        .spacing(6.0)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .into()
}

/// Parameters (Net) segmented tabs — All / Parameters / Rules / Classes.
fn net_params_tabs<'a>(
    primary: Color,
    label_c: Color,
    input_bg: Color,
    input_bdr: Color,
) -> Element<'a, PanelMsg> {
    let tab = |label: &'static str, active: bool| -> Element<'static, PanelMsg> {
        let bg_active = input_bdr;
        let fg_active = Color::WHITE;
        let fg_inactive = primary;
        iced::widget::button(
            text(label.to_string())
                .size(11)
                .color(if active { fg_active } else { fg_inactive })
                .align_x(iced::alignment::Horizontal::Center),
        )
        .padding([3, 12])
        .on_press(PanelMsg::Noop)
        .style(move |_: &Theme, status: iced::widget::button::Status| {
            let hovered = matches!(status, iced::widget::button::Status::Hovered);
            iced::widget::button::Style {
                background: Some(Background::Color(if active {
                    bg_active
                } else if hovered {
                    input_bdr
                } else {
                    input_bg
                })),
                border: Border {
                    width: 1.0,
                    radius: 3.0.into(),
                    color: input_bdr,
                },
                text_color: if active { fg_active } else { fg_inactive },
                ..iced::widget::button::Style::default()
            }
        })
        .into()
    };
    let _ = label_c;
    container(
        row![
            tab("All", true),
            tab("Parameters", false),
            tab("Rules", false),
            tab("Classes", false),
        ]
        .spacing(4.0),
    )
    .padding([4, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .into()
}

/// Two-column Name / Value header for the Parameters (Net) table.
fn net_params_header<'a>(label_c: Color, border_c: Color) -> Element<'a, PanelMsg> {
    container(
        row![
            text("Name".to_string())
                .size(10)
                .color(label_c)
                .width(Length::FillPortion(2)),
            text("Value".to_string())
                .size(10)
                .color(label_c)
                .width(Length::FillPortion(3)),
        ]
        .spacing(4.0),
    )
    .padding([4, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .style(move |_: &Theme| container::Style {
        border: Border {
            width: 1.0,
            radius: 0.0.into(),
            color: border_c,
        },
        ..container::Style::default()
    })
    .into()
}

/// Empty-state row — centered muted text spanning the whole row.
fn empty_section_row<'a>(label: &str, label_c: Color, border_c: Color) -> Element<'a, PanelMsg> {
    container(
        container(text(label.to_string()).size(10).color(label_c))
            .width(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center),
    )
    .padding([6, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .style(move |_: &Theme| container::Style {
        border: Border {
            width: 1.0,
            radius: 0.0.into(),
            color: border_c,
        },
        ..container::Style::default()
    })
    .into()
}

/// Add / edit / delete toolbar at the bottom of the Parameters table.
fn net_params_add_bar<'a>(
    label_c: Color,
    input_bg: Color,
    input_bdr: Color,
) -> Element<'a, PanelMsg> {
    let icon_btn = |label: &'static str| -> Element<'static, PanelMsg> {
        iced::widget::button(text(label.to_string()).size(11).color(label_c))
            .padding([4, 8])
            .on_press(PanelMsg::Noop)
            .style(move |_: &Theme, _| iced::widget::button::Style {
                background: Some(Background::Color(input_bg)),
                border: Border {
                    width: 1.0,
                    radius: 2.0.into(),
                    color: input_bdr,
                },
                text_color: label_c,
                ..iced::widget::button::Style::default()
            })
            .into()
    };
    container(
        row![
            Space::new().width(Length::Fill),
            iced::widget::button(
                text("Add \u{25BE}".to_string())
                    .size(11)
                    .color(Color::WHITE)
            )
            .padding([4, 12])
            .on_press(PanelMsg::Noop)
            .style(move |_: &Theme, _| iced::widget::button::Style {
                background: Some(Background::Color(input_bdr)),
                border: Border {
                    width: 1.0,
                    radius: 2.0.into(),
                    color: input_bdr
                },
                text_color: Color::WHITE,
                ..iced::widget::button::Style::default()
            }),
            icon_btn("\u{270E}"),
            icon_btn("\u{1F5D1}"),
        ]
        .spacing(4.0)
        .align_y(iced::Alignment::Center),
    )
    .padding([6, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .into()
}

/// Altium-style 3x3 justification picker with proper SVG arrow icons.
/// Only horizontal is wired to state for now; vertical slots toggle visually
/// but don't mutate the label.
fn justification_grid(
    id: uuid::Uuid,
    rotation_deg: f64,
    h: signex_types::schematic::HAlign,
    input_bg: Color,
    input_bdr: Color,
    primary: Color,
    muted: Color,
    theme: signex_types::theme::ThemeId,
) -> Element<'static, PanelMsg> {
    use signex_types::schematic::HAlign;
    let _ = muted;

    // Cell size mimics Altium's compact 24×24 px anchor picker.
    const CELL_SIZE: f32 = 24.0;
    let cell = |handle: iced::widget::svg::Handle,
                active: bool,
                on_press: PanelMsg|
     -> Element<'static, PanelMsg> {
        let bg_active = input_bdr;
        let fg_active = Color::WHITE;
        let fg_inactive = primary;
        let svg_widget =
            iced::widget::svg(handle)
                .width(12.0)
                .height(12.0)
                .style(move |_: &Theme, _| iced::widget::svg::Style {
                    color: Some(if active { fg_active } else { fg_inactive }),
                });
        iced::widget::button(
            container(svg_widget)
                .width(Length::Fill)
                .height(Length::Fill)
                .center(Length::Fill),
        )
        .width(CELL_SIZE)
        .height(CELL_SIZE)
        .padding(0)
        .on_press(on_press)
        .style(move |_: &Theme, status: iced::widget::button::Status| {
            let hovered = matches!(status, iced::widget::button::Status::Hovered);
            let bg = if active {
                bg_active
            } else if hovered {
                input_bdr
            } else {
                input_bg
            };
            iced::widget::button::Style {
                background: Some(Background::Color(bg)),
                border: Border {
                    width: 1.0,
                    radius: 2.0.into(),
                    color: input_bdr,
                },
                text_color: if active { fg_active } else { fg_inactive },
                ..iced::widget::button::Style::default()
            }
        })
        .into()
    };
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum LabelDir {
        Left,
        Up,
        Right,
        Down,
    }

    let normalize_rot = |deg: f64| {
        let r = (deg.round() as i32) % 360;
        if r < 0 { r + 360 } else { r }
    };

    let current_dir = {
        match normalize_rot(rotation_deg) {
            90 => LabelDir::Up,
            270 => LabelDir::Down,
            180 => {
                if matches!(h, HAlign::Right) {
                    LabelDir::Right
                } else {
                    LabelDir::Left
                }
            }
            _ => {
                if matches!(h, HAlign::Right) {
                    LabelDir::Left
                } else {
                    LabelDir::Right
                }
            }
        }
    };

    let to_msg = |dir: LabelDir| -> PanelMsg {
        match dir {
            LabelDir::Right => PanelMsg::EditLabelDirection(id, 0.0, HAlign::Left),
            LabelDir::Left => PanelMsg::EditLabelDirection(id, 0.0, HAlign::Right),
            LabelDir::Up => PanelMsg::EditLabelDirection(id, 90.0, HAlign::Left),
            LabelDir::Down => PanelMsg::EditLabelDirection(id, 270.0, HAlign::Left),
        }
    };

    let hl = |dir: LabelDir| current_dir == dir;

    iced::widget::column![
        iced::widget::row![
            cell(
                crate::icons::icon_justify_tl(theme),
                hl(LabelDir::Up),
                to_msg(LabelDir::Up)
            ),
            cell(
                crate::icons::icon_justify_t(theme),
                hl(LabelDir::Up),
                to_msg(LabelDir::Up)
            ),
            cell(
                crate::icons::icon_justify_tr(theme),
                hl(LabelDir::Up),
                to_msg(LabelDir::Up)
            ),
        ]
        .spacing(2),
        iced::widget::row![
            cell(
                crate::icons::icon_justify_l(theme),
                hl(LabelDir::Left),
                to_msg(LabelDir::Left)
            ),
            cell(
                crate::icons::icon_justify_c(theme),
                false,
                to_msg(current_dir)
            ),
            cell(
                crate::icons::icon_justify_r(theme),
                hl(LabelDir::Right),
                to_msg(LabelDir::Right)
            ),
        ]
        .spacing(2),
        iced::widget::row![
            cell(
                crate::icons::icon_justify_bl(theme),
                hl(LabelDir::Down),
                to_msg(LabelDir::Down)
            ),
            cell(
                crate::icons::icon_justify_b(theme),
                hl(LabelDir::Down),
                to_msg(LabelDir::Down)
            ),
            cell(
                crate::icons::icon_justify_br(theme),
                hl(LabelDir::Down),
                to_msg(LabelDir::Down)
            ),
        ]
        .spacing(2),
    ]
    .spacing(2)
    .into()
}

/// Pre-placement 3x3 justification picker. Same visual grid as the
/// selection-aware `justification_grid` but dispatches to the
/// `SetPrePlacementJustifyH` message family (no UUID needed).
fn preplacement_justification_grid(
    h: signex_types::schematic::HAlign,
    input_bg: Color,
    input_bdr: Color,
    primary: Color,
    muted: Color,
    theme: signex_types::theme::ThemeId,
) -> Element<'static, PanelMsg> {
    use signex_types::schematic::HAlign;
    let _ = muted;

    const CELL_SIZE: f32 = 24.0;
    let cell = |handle: iced::widget::svg::Handle,
                active: bool,
                on_press: PanelMsg|
     -> Element<'static, PanelMsg> {
        let bg_active = input_bdr;
        let fg_active = Color::WHITE;
        let fg_inactive = primary;
        let svg_widget =
            iced::widget::svg(handle)
                .width(12.0)
                .height(12.0)
                .style(move |_: &Theme, _| iced::widget::svg::Style {
                    color: Some(if active { fg_active } else { fg_inactive }),
                });
        iced::widget::button(
            container(svg_widget)
                .width(Length::Fill)
                .height(Length::Fill)
                .center(Length::Fill),
        )
        .width(CELL_SIZE)
        .height(CELL_SIZE)
        .padding(0)
        .on_press(on_press)
        .style(move |_: &Theme, status: iced::widget::button::Status| {
            let hovered = matches!(status, iced::widget::button::Status::Hovered);
            let bg = if active {
                bg_active
            } else if hovered {
                input_bdr
            } else {
                input_bg
            };
            iced::widget::button::Style {
                background: Some(Background::Color(bg)),
                border: Border {
                    width: 1.0,
                    radius: 2.0.into(),
                    color: input_bdr,
                },
                text_color: if active { fg_active } else { fg_inactive },
                ..iced::widget::button::Style::default()
            }
        })
        .into()
    };
    let hl_mid = |target: HAlign| -> bool { h == target };
    iced::widget::column![
        iced::widget::row![
            cell(
                crate::icons::icon_justify_tl(theme),
                false,
                PanelMsg::SetPrePlacementJustifyH(HAlign::Left)
            ),
            cell(
                crate::icons::icon_justify_t(theme),
                false,
                PanelMsg::SetPrePlacementJustifyH(HAlign::Center)
            ),
            cell(
                crate::icons::icon_justify_tr(theme),
                false,
                PanelMsg::SetPrePlacementJustifyH(HAlign::Right)
            ),
        ]
        .spacing(2),
        iced::widget::row![
            cell(
                crate::icons::icon_justify_l(theme),
                hl_mid(HAlign::Left),
                PanelMsg::SetPrePlacementJustifyH(HAlign::Left)
            ),
            cell(
                crate::icons::icon_justify_c(theme),
                hl_mid(HAlign::Center),
                PanelMsg::SetPrePlacementJustifyH(HAlign::Center)
            ),
            cell(
                crate::icons::icon_justify_r(theme),
                hl_mid(HAlign::Right),
                PanelMsg::SetPrePlacementJustifyH(HAlign::Right)
            ),
        ]
        .spacing(2),
        iced::widget::row![
            cell(
                crate::icons::icon_justify_bl(theme),
                false,
                PanelMsg::SetPrePlacementJustifyH(HAlign::Left)
            ),
            cell(
                crate::icons::icon_justify_b(theme),
                false,
                PanelMsg::SetPrePlacementJustifyH(HAlign::Center)
            ),
            cell(
                crate::icons::icon_justify_br(theme),
                false,
                PanelMsg::SetPrePlacementJustifyH(HAlign::Right)
            ),
        ]
        .spacing(2),
    ]
    .spacing(2)
    .into()
}

/// Tab button in the Custom Selection Filters tab strip. Active tab
/// gets a filled background; both states use the theme accent for the
/// border so the section reads as one piece with the chips and the
/// Active Bar dropdown.
fn custom_filter_tab(
    label: String,
    active: bool,
    idx: usize,
    hover_bg: Color,
    border_c: Color,
) -> Element<'static, PanelMsg> {
    let active_bg = Color::from_rgba8(0x2E, 0x33, 0x45, 1.0);
    let text_on = Color::WHITE;
    let text_off = Color::from_rgba8(0x99, 0x9D, 0xAE, 1.0);
    iced::widget::button(
        text(label)
            .size(11)
            .color(if active { text_on } else { text_off }),
    )
    .padding([3, 10])
    .on_press(PanelMsg::SelectCustomFilterTab(idx))
    .style(move |_: &Theme, status: iced::widget::button::Status| {
        let bg = match status {
            iced::widget::button::Status::Hovered if !active => Background::Color(hover_bg),
            _ if active => Background::Color(active_bg),
            _ => Background::Color(Color::TRANSPARENT),
        };
        iced::widget::button::Style {
            background: Some(bg),
            border: Border {
                width: 1.0,
                radius: 2.0.into(),
                color: border_c,
            },
            text_color: if active { text_on } else { text_off },
            ..iced::widget::button::Style::default()
        }
    })
    .into()
}

/// Member chip for a custom-filter preset card (Properties panel).
/// Border colour matches the Active Bar Filter dropdown chips (theme
/// accent), so chip styling stays consistent across both surfaces.
fn preset_chip(
    label: &str,
    preset_idx: usize,
    filter: crate::active_bar::SelectionFilter,
    enabled: bool,
    hover_bg: Color,
    border_c: Color,
) -> Element<'static, PanelMsg> {
    let active_bg = Color::from_rgba8(0x2E, 0x33, 0x45, 1.0);
    let inactive_bg = Color::from_rgba8(0x1A, 0x1D, 0x28, 1.0);
    let text_on = Color::WHITE;
    let text_off = Color::from_rgba8(0x66, 0x6A, 0x7E, 1.0);
    iced::widget::button(
        text(label.to_string())
            .size(10)
            .color(if enabled { text_on } else { text_off })
            .align_x(iced::alignment::Horizontal::Center),
    )
    .padding([3, 8])
    .on_press(PanelMsg::ToggleCustomFilterPresetMember(preset_idx, filter))
    .style(move |_: &Theme, status: iced::widget::button::Status| {
        let bg = match status {
            iced::widget::button::Status::Hovered => Background::Color(hover_bg),
            _ => Background::Color(if enabled { active_bg } else { inactive_bg }),
        };
        iced::widget::button::Style {
            background: Some(bg),
            border: Border {
                width: 1.0,
                radius: 2.0.into(),
                color: border_c,
            },
            text_color: if enabled { text_on } else { text_off },
            ..iced::widget::button::Style::default()
        }
    })
    .into()
}

/// Selection filter tag button — Altium pill with active/inactive state.
#[allow(dead_code)]
fn tag_btn(
    label: &str,
    filter: crate::active_bar::SelectionFilter,
    enabled: bool,
    hover_bg: Color,
) -> Element<'static, PanelMsg> {
    let active_bg = Color::from_rgba8(0x2E, 0x33, 0x45, 1.0);
    let inactive_bg = Color::from_rgba8(0x1A, 0x1D, 0x28, 1.0);
    let active_border = Color::from_rgba8(0x4D, 0x52, 0x66, 1.0);
    let inactive_border = Color::from_rgba8(0x33, 0x36, 0x44, 1.0);
    let text_on = Color::WHITE;
    let text_off = Color::from_rgba8(0x66, 0x6A, 0x7E, 1.0);
    iced::widget::button(
        text(label.to_string())
            .size(10)
            .color(if enabled { text_on } else { text_off })
            .align_x(iced::alignment::Horizontal::Center),
    )
    .padding([3, 8])
    .on_press(PanelMsg::ToggleSelectionFilter(filter))
    .style(move |_: &Theme, status: iced::widget::button::Status| {
        let bg = match status {
            iced::widget::button::Status::Hovered => Background::Color(hover_bg),
            _ => Background::Color(if enabled { active_bg } else { inactive_bg }),
        };
        iced::widget::button::Style {
            background: Some(bg),
            border: Border {
                width: 1.0,
                radius: 12.0.into(),
                color: if enabled {
                    active_border
                } else {
                    inactive_border
                },
            },
            text_color: if enabled { text_on } else { text_off },
            ..iced::widget::button::Style::default()
        }
    })
    .into()
}

/// Segmented button (for units toggle etc).
#[allow(clippy::too_many_arguments)]
fn seg_btn<'a>(
    label: &str,
    active: bool,
    msg: PanelMsg,
    active_bg: Color,
    text_active: Color,
    text_inactive: Color,
    hover_bg: Color,
    seg_border: Color,
) -> Element<'a, PanelMsg> {
    let bg = if active {
        active_bg
    } else {
        Color::TRANSPARENT
    };
    let text_c = if active { text_active } else { text_inactive };
    iced::widget::button(
        text(label.to_string())
            .size(11)
            .color(text_c)
            .align_x(iced::alignment::Horizontal::Center),
    )
    .padding([4, 0])
    .width(Length::Fill)
    .on_press(msg)
    .style(move |_: &Theme, status: iced::widget::button::Status| {
        let hovered = matches!(status, iced::widget::button::Status::Hovered);
        iced::widget::button::Style {
            background: Some(Background::Color(if hovered && !active {
                hover_bg
            } else {
                bg
            })),
            border: Border {
                width: 1.0,
                radius: 2.0.into(),
                color: seg_border,
            },
            ..iced::widget::button::Style::default()
        }
    })
    .into()
}

// ─── ERC Panel ────────────────────────────────────────────────

fn view_erc<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let mut col: Column<'a, PanelMsg> = Column::new().spacing(2).padding(6).width(Length::Fill);
    col = col.push(
        row![
            section_title("ERC", &ctx.tokens),
            Space::new().width(Length::Fill).height(Length::Shrink),
            iced::widget::button(
                text("Run ERC (F8)")
                    .size(11)
                    .color(theme_ext::text_primary(&ctx.tokens)),
            )
            .padding([3, 10])
            .on_press(PanelMsg::RunErc)
            .style(crate::styles::menu_item(&ctx.tokens)),
            Space::new().width(6).height(Length::Shrink),
            iced::widget::button(
                text("Clear")
                    .size(11)
                    .color(theme_ext::text_secondary(&ctx.tokens)),
            )
            .padding([3, 10])
            .on_press(PanelMsg::ClearErc)
            .style(crate::styles::menu_item(&ctx.tokens)),
        ]
        .align_y(iced::Alignment::Center),
    );
    col = col.push(separator(&ctx.tokens));

    if ctx.erc_diagnostics.is_empty() {
        col = col.push(
            text("No ERC diagnostics yet")
                .size(10)
                .color(theme_ext::text_secondary(&ctx.tokens)),
        );
        col = col.push(
            text("Run ERC to populate project-wide diagnostics")
                .size(9)
                .color(theme_ext::text_secondary(&ctx.tokens)),
        );
        return col.into();
    }

    let errors = ctx
        .erc_diagnostics
        .iter()
        .filter(|v| v.severity == ErcSeverityLite::Error)
        .count();
    let warnings = ctx
        .erc_diagnostics
        .iter()
        .filter(|v| v.severity == ErcSeverityLite::Warning)
        .count();
    let infos = ctx
        .erc_diagnostics
        .iter()
        .filter(|v| v.severity == ErcSeverityLite::Info)
        .count();
    let focus_label = if let Some(i) = ctx.erc_focus_index {
        format!("{}/{}", i + 1, ctx.erc_diagnostics.len())
    } else {
        format!("0/{}", ctx.erc_diagnostics.len())
    };

    col = col.push(
        row![
            text("ERC Diagnostic Results")
                .size(10)
                .color(theme_ext::text_primary(&ctx.tokens)),
            Space::new().width(8).height(Length::Shrink),
            text(format!("{errors} errors"))
                .size(10)
                .color(theme_ext::error_color(&ctx.tokens)),
            Space::new().width(8).height(Length::Shrink),
            text(format!("{warnings} warnings"))
                .size(10)
                .color(theme_ext::warning_color(&ctx.tokens)),
            Space::new().width(8).height(Length::Shrink),
            text(format!("{infos} info"))
                .size(10)
                .color(theme_ext::accent(&ctx.tokens)),
            Space::new().width(Length::Fill).height(Length::Shrink),
            iced::widget::button(text("<").size(11))
                .padding([1, 6])
                .on_press(PanelMsg::FocusPrevErcDiagnostic)
                .style(crate::styles::menu_item(&ctx.tokens)),
            Space::new().width(4).height(Length::Shrink),
            text(focus_label)
                .size(9)
                .color(theme_ext::text_secondary(&ctx.tokens)),
            Space::new().width(4).height(Length::Shrink),
            iced::widget::button(text(">").size(11))
                .padding([1, 6])
                .on_press(PanelMsg::FocusNextErcDiagnostic)
                .style(crate::styles::menu_item(&ctx.tokens)),
        ]
        .align_y(iced::Alignment::Center),
    );

    for v in &ctx.erc_diagnostics {
        let sev_color = match v.severity {
            ErcSeverityLite::Error => theme_ext::error_color(&ctx.tokens),
            ErcSeverityLite::Warning => theme_ext::warning_color(&ctx.tokens),
            ErcSeverityLite::Info => theme_ext::accent(&ctx.tokens),
        };
        let sev_label = match v.severity {
            ErcSeverityLite::Error => "E",
            ErcSeverityLite::Warning => "W",
            ErcSeverityLite::Info => "I",
        };
        let is_focused = ctx.erc_focus_index == Some(v.global_index);
        let row_bg = if is_focused {
            Some(Background::Color(theme_ext::selection_color(&ctx.tokens)))
        } else {
            None
        };
        col = col.push(
            iced::widget::button(
                row![
                    text(sev_label).size(9).color(sev_color),
                    Space::new().width(4).height(Length::Shrink),
                    text(v.rule_label)
                        .size(9)
                        .color(theme_ext::text_primary(&ctx.tokens)),
                    Space::new().width(6).height(Length::Shrink),
                    text(v.message.clone())
                        .size(9)
                        .color(theme_ext::text_secondary(&ctx.tokens)),
                    Space::new().width(6).height(Length::Shrink),
                    text(format!(
                        "{}  ({:.2}, {:.2})  {}",
                        v.sheet_name,
                        v.world_x,
                        v.world_y,
                        v.sheet_path.display()
                    ))
                    .size(9)
                    .color(theme_ext::text_secondary(&ctx.tokens)),
                ]
                .align_y(iced::Alignment::Center)
                .width(Length::Fill),
            )
            .width(Length::Fill)
            .padding([2, 6])
            .on_press(PanelMsg::FocusErcViolation(v.global_index))
            .style(
                move |_theme: &Theme, status: iced::widget::button::Status| {
                    let base = crate::styles::menu_item(&ctx.tokens)(_theme, status);
                    iced::widget::button::Style {
                        background: row_bg.clone().or(base.background),
                        ..base
                    }
                },
            ),
        );
    }

    col.into()
}

// ─── Messages Panel ───────────────────────────────────────────

fn view_messages<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let mut col: Column<'a, PanelMsg> = Column::new().spacing(2).padding(6).width(Length::Fill);
    col = col.push(
        row![
            section_title("Messages", &ctx.tokens),
            Space::new().width(Length::Fill).height(Length::Shrink),
            text(format!("level {}", ctx.diagnostics_level))
                .size(10)
                .color(theme_ext::text_secondary(&ctx.tokens)),
        ]
        .align_y(iced::Alignment::Center),
    );
    col = col.push(separator(&ctx.tokens));

    if ctx.diagnostics.is_empty() {
        col = col.push(
            text("No runtime messages yet")
                .size(10)
                .color(theme_ext::success_color(&ctx.tokens)),
        );
        col = col.push(
            text("Set RUST_LOG=debug or SIGNEX_LOG=debug for verbose output")
                .size(9)
                .color(theme_ext::text_secondary(&ctx.tokens)),
        );
    } else {
        // Table header — mirrors Altium's Messages panel with fixed
        // columns for id, level, source tag, and the message body.
        let muted = theme_ext::text_secondary(&ctx.tokens);
        let primary = theme_ext::text_primary(&ctx.tokens);
        let border = theme_ext::border_color(&ctx.tokens);
        let header_bg = theme_ext::to_color(&ctx.tokens.hover);
        let row_bg = theme_ext::to_color(&ctx.tokens.panel_bg);
        // Alt-row tint: pull a darker variant from the panel background
        // by shifting alpha. iced::Color lets us blend cheaply.
        let alt_bg = Color {
            a: row_bg.a,
            r: (row_bg.r - 0.02).max(0.0),
            g: (row_bg.g - 0.02).max(0.0),
            b: (row_bg.b - 0.02).max(0.0),
        };

        let th = |label: &str| -> Element<'a, PanelMsg> {
            container(text(label.to_string()).size(11).color(muted))
                .padding([4, 8])
                .into()
        };
        // Header row — background fill, no border (separator line
        // below sits in its own element so the table reads as a grid
        // of horizontal rules instead of individually framed boxes).
        col = col.push(
            container(
                row![
                    container(th("#")).width(Length::Fixed(48.0)),
                    container(th("Level")).width(Length::Fixed(64.0)),
                    container(th("Source")).width(Length::Fixed(180.0)),
                    container(th("Message")).width(Length::Fill),
                ]
                .align_y(iced::Alignment::Center),
            )
            .style(move |_theme: &Theme| iced::widget::container::Style {
                background: Some(Background::Color(header_bg)),
                ..iced::widget::container::Style::default()
            }),
        );
        let separator = |bg: Color| -> Element<'a, PanelMsg> {
            container(Space::new())
                .height(Length::Fixed(1.0))
                .width(Length::Fill)
                .style(move |_: &Theme| iced::widget::container::Style {
                    background: Some(Background::Color(bg)),
                    ..iced::widget::container::Style::default()
                })
                .into()
        };
        col = col.push(separator(border));
        for (i, entry) in ctx.diagnostics.iter().rev().enumerate() {
            let level_color = match entry.level {
                crate::diagnostics::DiagnosticLevel::Error => theme_ext::error_color(&ctx.tokens),
                crate::diagnostics::DiagnosticLevel::Warning => {
                    theme_ext::warning_color(&ctx.tokens)
                }
                crate::diagnostics::DiagnosticLevel::Info => theme_ext::accent(&ctx.tokens),
                crate::diagnostics::DiagnosticLevel::Debug
                | crate::diagnostics::DiagnosticLevel::Trace => {
                    theme_ext::text_secondary(&ctx.tokens)
                }
            };
            let bg = if i % 2 == 0 { row_bg } else { alt_bg };
            let cell = |label: String, color: Color, size: f32| -> Element<'a, PanelMsg> {
                container(text(label).size(size).color(color))
                    .padding([4, 8])
                    .into()
            };
            col = col.push(
                container(
                    row![
                        container(cell(format!("#{}", entry.id), muted, 11.0))
                            .width(Length::Fixed(48.0)),
                        container(cell(entry.level.label().to_string(), level_color, 11.0,))
                            .width(Length::Fixed(64.0)),
                        container(cell(entry.code.as_str().to_string(), muted, 11.0))
                            .width(Length::Fixed(180.0)),
                        container(cell(entry.message.as_str().to_string(), primary, 12.0))
                            .width(Length::Fill),
                    ]
                    .align_y(iced::Alignment::Center),
                )
                // No border on the row — the explicit separator line
                // drawn below is the grid. Boxes around each entry
                // (the previous look) were too heavy for a log table.
                .style(move |_theme: &Theme| iced::widget::container::Style {
                    background: Some(Background::Color(bg)),
                    ..iced::widget::container::Style::default()
                }),
            );
            // One-pixel horizontal rule between rows — no right / left
            // borders so the table reads as a stack of records with
            // shared separators, matching Altium's Messages panel.
            col = col.push(separator(border));
        }
    }

    container(col).width(Length::Fill).into()
}

// ─── Drawing properties editor (post-placement) ──────────────────

fn view_drawing_properties<'a>(
    ctx: &'a PanelContext,
    muted: Color,
    _primary: Color,
    border_c: Color,
) -> Element<'a, PanelMsg> {
    use signex_types::schematic::FillType;
    let get = |key: &str| -> String {
        ctx.selection_info
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    };
    let parse_pair = |s: &str| -> (f64, f64) {
        let parts: Vec<&str> = s.split(',').collect();
        let x = parts
            .first()
            .and_then(|p| p.trim().parse::<f64>().ok())
            .unwrap_or(0.0);
        let y = parts
            .get(1)
            .and_then(|p| p.trim().parse::<f64>().ok())
            .unwrap_or(0.0);
        (x, y)
    };
    let parse_f64 = |s: &str| -> f64 { s.trim().parse::<f64>().ok().unwrap_or(0.0) };
    let parse_fill = |s: &str| -> FillType {
        match s {
            "Outline" => FillType::Outline,
            "Background" => FillType::Background,
            _ => FillType::None,
        }
    };

    let elem_type = get("Type");
    // Line + Arc store their stroke in the `Width` key; Rect / Circle
    // / Polygon put stroke under `Border` and use `Width` for the
    // X-dimension (Rect) or nothing (Circle / Polygon). Picking the
    // wrong one here pre-fills the input with the X-dim or 0.
    let stroke_w = match elem_type.as_str() {
        "Line" | "Arc" => parse_f64(&get("Width")),
        _ => parse_f64(&get("Border")),
    };
    let fill = parse_fill(&get("Fill"));
    let show_fill = matches!(elem_type.as_str(), "Rectangle" | "Circle" | "Polygon");

    let mut col = Column::new().spacing(0).width(Length::Fill);
    // Header: shape icon + type label. Draft SVGs live at
    // assets/icons/shape_*.svg and can be swapped out for final art
    // without touching the panel code.
    let header_row: Element<'a, PanelMsg> =
        if let Some(icon) = shape_icon_handle(&elem_type, ctx.theme_id) {
            row![
                iced::widget::svg(icon).width(16).height(16),
                text(elem_type.clone())
                    .size(11)
                    .color(Color::from_rgb(0.90, 0.90, 0.92)),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center)
            .into()
        } else {
            text(elem_type.clone())
                .size(11)
                .color(Color::from_rgb(0.90, 0.90, 0.92))
                .into()
        };
    col = col.push(container(header_row).padding([6, 8]).width(Length::Fill));
    col = col.push(thin_sep(border_c));

    // Live preview canvas — shows the selected shape at panel scale
    // with optional radius/angle annotations so edits to the rows
    // below re-render the preview immediately.
    if let Some(drawing) = &ctx.selected_drawing {
        let preview = DrawingPreview {
            drawing: drawing.clone(),
            stroke: Color::from_rgb(0.94, 0.74, 0.28),
            fill: Color::from_rgb(0.94, 0.74, 0.28),
            muted,
            accent: Color::from_rgb(0.24, 0.62, 0.97),
        };
        let canvas_w: Element<'a, PanelMsg> = iced::widget::canvas(preview)
            .width(Length::Fill)
            .height(Length::Fixed(160.0))
            .into();
        col = col.push(
            container(canvas_w)
                .padding([8, 12])
                .width(Length::Fill)
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(Color::from_rgba(0.07, 0.07, 0.08, 0.6))),
                    border: Border {
                        width: 1.0,
                        color: border_c,
                        radius: 0.0.into(),
                    },
                    ..container::Style::default()
                }),
        );
    }

    let buf = &ctx.drawing_edit_buf;
    match elem_type.as_str() {
        "Line" => {
            let (sx, sy) = parse_pair(&get("Start"));
            let (ex, ey) = parse_pair(&get("End"));
            col = col.push(collapsible_section(
                "draw_line",
                "Properties",
                &ctx.collapsed_sections,
                muted,
                border_c,
                move || {
                    let mut c = Column::new().spacing(0).width(Length::Fill);
                    c = c.push(drawing_num_row(
                        "Start X",
                        DrawingFieldId::LineStartX,
                        sx,
                        buf,
                        muted,
                    ));
                    c = c.push(drawing_num_row(
                        "Start Y",
                        DrawingFieldId::LineStartY,
                        sy,
                        buf,
                        muted,
                    ));
                    c = c.push(drawing_num_row(
                        "End X",
                        DrawingFieldId::LineEndX,
                        ex,
                        buf,
                        muted,
                    ));
                    c = c.push(drawing_num_row(
                        "End Y",
                        DrawingFieldId::LineEndY,
                        ey,
                        buf,
                        muted,
                    ));
                    c = c.push(drawing_num_row(
                        "Width (mm)",
                        DrawingFieldId::LineWidth,
                        stroke_w,
                        buf,
                        muted,
                    ));
                    c
                },
            ));
        }
        "Rectangle" => {
            let (px, py) = parse_pair(&get("Position"));
            let w_mm = parse_f64(&get("Width"));
            let h_mm = parse_f64(&get("Height"));
            col = col.push(collapsible_section(
                "draw_rect",
                "Properties",
                &ctx.collapsed_sections,
                muted,
                border_c,
                move || {
                    let mut c = Column::new().spacing(0).width(Length::Fill);
                    c = c.push(drawing_num_row(
                        "Position X",
                        DrawingFieldId::RectStartX,
                        px,
                        buf,
                        muted,
                    ));
                    c = c.push(drawing_num_row(
                        "Position Y",
                        DrawingFieldId::RectStartY,
                        py,
                        buf,
                        muted,
                    ));
                    c = c.push(drawing_num_row(
                        "Width (mm)",
                        DrawingFieldId::RectWidth,
                        w_mm,
                        buf,
                        muted,
                    ));
                    c = c.push(drawing_num_row(
                        "Height (mm)",
                        DrawingFieldId::RectHeight,
                        h_mm,
                        buf,
                        muted,
                    ));
                    c = c.push(drawing_num_row(
                        "Border",
                        DrawingFieldId::RectBorder,
                        stroke_w,
                        buf,
                        muted,
                    ));
                    if show_fill {
                        c = c.push(drawing_fill_row(fill, muted, border_c));
                    }
                    c
                },
            ));
        }
        "Circle" => {
            let (cx, cy) = parse_pair(&get("Center"));
            let radius = parse_f64(&get("Radius"));
            col = col.push(collapsible_section(
                "draw_circle",
                "Properties",
                &ctx.collapsed_sections,
                muted,
                border_c,
                move || {
                    let mut c = Column::new().spacing(0).width(Length::Fill);
                    c = c.push(drawing_num_row(
                        "Center X",
                        DrawingFieldId::CircleCenterX,
                        cx,
                        buf,
                        muted,
                    ));
                    c = c.push(drawing_num_row(
                        "Center Y",
                        DrawingFieldId::CircleCenterY,
                        cy,
                        buf,
                        muted,
                    ));
                    c = c.push(drawing_num_row(
                        "Radius",
                        DrawingFieldId::CircleRadius,
                        radius,
                        buf,
                        muted,
                    ));
                    c = c.push(drawing_num_row(
                        "Border",
                        DrawingFieldId::CircleBorder,
                        stroke_w,
                        buf,
                        muted,
                    ));
                    if show_fill {
                        c = c.push(drawing_fill_row(fill, muted, border_c));
                    }
                    c
                },
            ));
        }
        "Arc" => {
            let (cx, cy) = parse_pair(&get("Center"));
            let radius = parse_f64(&get("Radius"));
            let start_angle = parse_f64(&get("Start Angle"));
            let end_angle = parse_f64(&get("End Angle"));
            col = col.push(collapsible_section(
                "draw_arc",
                "Properties",
                &ctx.collapsed_sections,
                muted,
                border_c,
                move || {
                    let mut c = Column::new().spacing(0).width(Length::Fill);
                    c = c.push(drawing_num_row(
                        "Center X",
                        DrawingFieldId::ArcCenterX,
                        cx,
                        buf,
                        muted,
                    ));
                    c = c.push(drawing_num_row(
                        "Center Y",
                        DrawingFieldId::ArcCenterY,
                        cy,
                        buf,
                        muted,
                    ));
                    c = c.push(drawing_num_row(
                        "Radius",
                        DrawingFieldId::ArcRadius,
                        radius,
                        buf,
                        muted,
                    ));
                    c = c.push(drawing_num_row(
                        "Start Angle",
                        DrawingFieldId::ArcStartAngle,
                        start_angle,
                        buf,
                        muted,
                    ));
                    c = c.push(drawing_num_row(
                        "End Angle",
                        DrawingFieldId::ArcEndAngle,
                        end_angle,
                        buf,
                        muted,
                    ));
                    c = c.push(drawing_num_row(
                        "Width",
                        DrawingFieldId::ArcWidth,
                        stroke_w,
                        buf,
                        muted,
                    ));
                    c
                },
            ));
        }
        "Polygon" => {
            let vert_count = parse_f64(&get("Vertices")) as i32;
            col = col.push(collapsible_section(
                "draw_poly",
                "Properties",
                &ctx.collapsed_sections,
                muted,
                border_c,
                move || {
                    let mut c = Column::new().spacing(0).width(Length::Fill);
                    c = c.push(prop_kv_row(
                        "Vertices",
                        &vert_count.to_string(),
                        muted,
                        Color::from_rgb(0.90, 0.90, 0.92),
                    ));
                    c = c.push(drawing_num_row(
                        "Border",
                        DrawingFieldId::PolyBorder,
                        stroke_w,
                        buf,
                        muted,
                    ));
                    if show_fill {
                        c = c.push(drawing_fill_row(fill, muted, border_c));
                    }
                    c
                },
            ));
        }
        _ => {
            for (key, value) in &ctx.selection_info {
                if key != "Type" {
                    col = col.push(prop_kv_row(
                        key,
                        value,
                        muted,
                        Color::from_rgb(0.9, 0.9, 0.92),
                    ));
                }
            }
        }
    }
    // Stroke colour swatch row — applies to every drawing kind that
    // matched a known variant above. Reads the current stored colour
    // from the live SchDrawing so the active tile highlights correctly.
    if matches!(
        elem_type.as_str(),
        "Line" | "Rectangle" | "Circle" | "Arc" | "Polygon"
    ) {
        let current_color = ctx.selected_drawing.as_ref().and_then(|d| match d {
            signex_types::schematic::SchDrawing::Line { stroke_color, .. }
            | signex_types::schematic::SchDrawing::Rect { stroke_color, .. }
            | signex_types::schematic::SchDrawing::Circle { stroke_color, .. }
            | signex_types::schematic::SchDrawing::Arc { stroke_color, .. }
            | signex_types::schematic::SchDrawing::Polyline { stroke_color, .. } => *stroke_color,
        });
        col = col.push(drawing_stroke_color_row(current_color, muted));
    }
    col.into()
}

/// Properties section for a single hierarchical child sheet.
/// Shows read-only info (Name / File / Position / Size) plus
/// editable Border Colour, Fill Colour and Line Width with a
/// Reset-to-default button. Colour edits open an iced_aw
/// ColorPicker overlay anchored to a swatch button.
fn view_child_sheet_properties<'a>(
    ctx: &'a PanelContext,
    muted: Color,
    primary: Color,
    border_c: Color,
) -> Element<'a, PanelMsg> {
    let Some(child_sheet) = ctx.selected_child_sheet.as_ref() else {
        return Column::new().width(Length::Fill).into();
    };

    let id = child_sheet.uuid;
    let name = child_sheet.name.clone();
    let filename = child_sheet.filename.clone();
    let position = format!(
        "{:.2}, {:.2}",
        child_sheet.position.x, child_sheet.position.y
    );
    let size = format!(
        "{:.1} x {:.1} mm",
        child_sheet.size.0, child_sheet.size.1
    );

    let stroke_width = child_sheet.stroke_width;
    let stroke_color = child_sheet.stroke_color;
    let fill_color = child_sheet.fill_color;
    let border_picker_open = ctx.child_sheet_border_picker_open;
    let fill_picker_open = ctx.child_sheet_fill_picker_open;
    let border_advanced_open = ctx.child_sheet_border_advanced_open;
    let fill_advanced_open = ctx.child_sheet_fill_advanced_open;
    let stroke_width_buf = ctx.child_sheet_stroke_width_buf.clone();

    let mut col: Column<'a, PanelMsg> = Column::new().spacing(0).width(Length::Fill);

    // ── Properties (read-only identity / geometry) ──
    col = col.push(collapsible_section(
        "sel_child_sheet_props",
        "Properties",
        &ctx.collapsed_sections,
        muted,
        border_c,
        move || {
            let mut c = Column::new().spacing(0).width(Length::Fill);
            c = c.push(prop_kv_row("Name", &name, muted, primary));
            c = c.push(prop_kv_row("File", &filename, muted, primary));
            c = c.push(prop_kv_row("Position", &position, muted, primary));
            c = c.push(prop_kv_row("Size", &size, muted, primary));
            c
        },
    ));

    // ── Style (editable) ──
    col = col.push(collapsible_section(
        "sel_child_sheet_style",
        "Style",
        &ctx.collapsed_sections,
        muted,
        border_c,
        move || {
            let mut c = Column::new().spacing(0).width(Length::Fill);
            c = c.push(child_sheet_color_row(
                "Border Colour",
                id,
                stroke_color,
                border_picker_open,
                border_advanced_open,
                muted,
                border_c,
                /* is_border */ true,
            ));
            c = c.push(child_sheet_color_row(
                "Fill Colour",
                id,
                fill_color,
                fill_picker_open,
                fill_advanced_open,
                muted,
                border_c,
                /* is_border */ false,
            ));
            c = c.push(child_sheet_stroke_width_row(
                id,
                stroke_width,
                stroke_width_buf,
                muted,
                border_c,
            ));
            c = c.push(
                container(
                    iced::widget::button(
                        text("Reset to Default")
                            .size(10)
                            .color(Color::from_rgb(0.92, 0.92, 0.94)),
                    )
                    .padding([4, 10])
                    .on_press(PanelMsg::ResetChildSheetStyle(id))
                    .style(iced::widget::button::secondary),
                )
                .padding([6, 8])
                .width(Length::Fill),
            );
            c
        },
    ));

    col.into()
}

/// One swatch row in the child-sheet Style section.
///
/// Click flow:
///   1. Click swatch → expands an inline preset palette panel below
///      the row (full panel width, like the canvas-font popup) with
///      a 12-colour grid plus "Custom…" and (when an override is
///      active) "Reset to Default".
///   2. Click the swatch again to collapse, or click "Custom…" to
///      switch to the iced_aw HSV / RGB ColorPicker overlay.
///
/// Both the palette pick and the advanced-picker submit reuse the
/// same `EditChildSheet*Color` message so engine command + undo/redo
/// round-trip is identical for both paths.
fn child_sheet_color_row<'a>(
    label: &'a str,
    sheet_id: uuid::Uuid,
    current: Option<signex_types::schematic::StrokeColor>,
    show_picker: bool,
    show_advanced: bool,
    muted: Color,
    border_c: Color,
    is_border: bool,
) -> Element<'a, PanelMsg> {
    let preview_color = current
        .map(|c| {
            iced::Color::from_rgba(
                c.r as f32 / 255.0,
                c.g as f32 / 255.0,
                c.b as f32 / 255.0,
                c.a as f32 / 255.0,
            )
        })
        .unwrap_or(iced::Color::from_rgba(0.5, 0.5, 0.5, 0.4));
    let label_text = if current.is_some() {
        format!(
            "#{:02X}{:02X}{:02X}",
            current.unwrap().r,
            current.unwrap().g,
            current.unwrap().b
        )
    } else {
        "Default".to_string()
    };

    let toggle_msg = if is_border {
        PanelMsg::ToggleChildSheetBorderPicker(sheet_id)
    } else {
        PanelMsg::ToggleChildSheetFillPicker(sheet_id)
    };

    // Swatch button: 18x18 colour fill + small hex / "Default" caption.
    let swatch_color = preview_color;
    let swatch: Element<'a, PanelMsg> = container(Space::new())
        .width(18)
        .height(18)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(swatch_color)),
            border: Border {
                width: 1.0,
                color: border_c,
                radius: 2.0.into(),
            },
            ..container::Style::default()
        })
        .into();

    let swatch_button = iced::widget::button(
        row![
            swatch,
            text(label_text).size(10).color(Color::from_rgb(0.90, 0.90, 0.92)),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, 6])
    .on_press(toggle_msg)
    .style(iced::widget::button::secondary);

    // Build the per-channel "submit colour" message.
    let make_submit = move |c: iced::Color| -> PanelMsg {
        if is_border {
            PanelMsg::EditChildSheetBorderColor(sheet_id, c)
        } else {
            PanelMsg::EditChildSheetFillColor(sheet_id, c)
        }
    };

    // ── Advanced (HSV / RGB) overlay ──
    if show_advanced {
        let picker = iced_aw::ColorPicker::new(
            true,
            preview_color,
            swatch_button,
            PanelMsg::CancelChildSheetColorPicker,
            move |c| make_submit(c),
        );
        return container(
            row![
                text(label.to_string()).size(10).color(muted).width(96),
                picker,
            ]
            .spacing(4)
            .align_y(iced::Alignment::Center),
        )
        .padding([4, 8])
        .width(Length::Fill)
        .into();
    }

    // The header row (label + swatch button) is always shown.
    let header = container(
        row![
            text(label.to_string()).size(10).color(muted).width(96),
            swatch_button,
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center),
    )
    .padding([4, 8])
    .width(Length::Fill);

    if !show_picker {
        return header.into();
    }

    // ── Inline preset palette (rendered below the row, full width) ──
    let presets: [(&str, [u8; 3]); 12] = [
        ("Black",       [0x00, 0x00, 0x00]),
        ("Dark Gray",   [0x40, 0x40, 0x40]),
        ("Gray",        [0x80, 0x80, 0x80]),
        ("White",       [0xFF, 0xFF, 0xFF]),
        ("Red",         [0xC0, 0x39, 0x2B]),
        ("Orange",      [0xE6, 0x7E, 0x22]),
        ("Yellow",      [0xF1, 0xC4, 0x0F]),
        ("Olive",       [0xB4, 0xA5, 0x58]),
        ("Green",       [0x27, 0xAE, 0x60]),
        ("Teal",        [0x16, 0xA0, 0x85]),
        ("Blue",        [0x29, 0x80, 0xB9]),
        ("Purple",      [0x8E, 0x44, 0xAD]),
    ];

    // 6 columns × 2 rows of preset swatches; each cell stretches
    // proportionally so the grid always fills the available panel
    // width (no clipping in narrow docks).
    let mut palette_grid: Column<'a, PanelMsg> = Column::new().spacing(4);
    for chunk in presets.chunks(6) {
        let mut r: iced::widget::Row<'a, PanelMsg> = iced::widget::Row::new().spacing(4);
        for (_name, rgb) in chunk {
            let c = iced::Color::from_rgb(
                rgb[0] as f32 / 255.0,
                rgb[1] as f32 / 255.0,
                rgb[2] as f32 / 255.0,
            );
            let swatch_btn = iced::widget::button(Space::new())
                .width(Length::Fill)
                .height(22)
                .padding(0)
                .on_press(make_submit(c))
                .style(move |_t: &Theme, _s| iced::widget::button::Style {
                    background: Some(Background::Color(c)),
                    border: Border {
                        width: 1.0,
                        color: border_c,
                        radius: 2.0.into(),
                    },
                    ..iced::widget::button::Style::default()
                });
            r = r.push(swatch_btn);
        }
        palette_grid = palette_grid.push(r);
    }

    let mut palette_col: Column<'a, PanelMsg> =
        Column::new().spacing(6).padding([6, 8]).width(Length::Fill);
    palette_col = palette_col.push(text("Preset Colours").size(10).color(muted));
    palette_col = palette_col.push(palette_grid);

    let mut action_row: iced::widget::Row<'a, PanelMsg> =
        iced::widget::Row::new().spacing(4).width(Length::Fill);
    action_row = action_row.push(
        iced::widget::button(
            text("Custom…")
                .size(10)
                .color(Color::from_rgb(0.92, 0.92, 0.94)),
        )
        .padding([4, 10])
        .width(Length::Fill)
        .on_press(PanelMsg::OpenChildSheetAdvancedPicker(sheet_id, is_border))
        .style(iced::widget::button::secondary),
    );
    if current.is_some() {
        action_row = action_row.push(
            iced::widget::button(
                text("Reset to Default")
                    .size(10)
                    .color(Color::from_rgb(0.92, 0.92, 0.94)),
            )
            .padding([4, 10])
            .width(Length::Fill)
            .on_press(PanelMsg::ResetChildSheetStyle(sheet_id))
            .style(iced::widget::button::secondary),
        );
    }
    palette_col = palette_col.push(action_row);

    let palette_panel = container(palette_col)
        .width(Length::Fill)
        .style(move |_t: &Theme| container::Style {
            background: Some(Background::Color(Color::from_rgb(0.16, 0.16, 0.18))),
            border: Border {
                width: 1.0,
                color: border_c,
                radius: 4.0.into(),
            },
            ..container::Style::default()
        });

    column![header, container(palette_panel).padding([0, 8])]
        .spacing(4)
        .width(Length::Fill)
        .into()
}

/// Numeric stroke-width row for the child-sheet Style section.
fn child_sheet_stroke_width_row<'a>(
    sheet_id: uuid::Uuid,
    stored_value: f64,
    buffered: Option<String>,
    muted: Color,
    border_c: Color,
) -> Element<'a, PanelMsg> {
    let display = match buffered {
        Some(s) => s,
        None => format!("{:.4}", stored_value)
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string(),
    };
    let display_for_input = display.clone();
    let input = iced::widget::text_input("0.1524", &display_for_input)
        .size(11)
        .padding(4)
        .width(120)
        .on_input(move |s| PanelMsg::ChildSheetStrokeWidthTyping(sheet_id, s))
        .on_submit(PanelMsg::CommitChildSheetStrokeWidth(sheet_id))
        .style(move |_theme: &Theme, _status| iced::widget::text_input::Style {
            background: Background::Color(Color::from_rgba(0.07, 0.07, 0.08, 1.0)),
            border: Border {
                width: 1.0,
                color: border_c,
                radius: 2.0.into(),
            },
            icon: Color::from_rgba(0.7, 0.7, 0.7, 1.0),
            placeholder: Color::from_rgba(0.5, 0.5, 0.5, 1.0),
            value: Color::from_rgb(0.95, 0.95, 0.96),
            selection: Color::from_rgba(0.24, 0.62, 0.97, 0.4),
        });

    container(
        row![
            text("Line Width (mm)").size(10).color(muted).width(96),
            input,
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center),
    )
    .padding([4, 8])
    .width(Length::Fill)
    .into()
}

/// Buffer-backed numeric row — survives empty / partial input so the
/// user can erase and retype the whole value. Emits DrawingFieldTyping
/// on every keystroke; the handler commits to the engine when the
/// string parses as f64.
fn drawing_num_row<'a>(
    label: &'a str,
    field: DrawingFieldId,
    stored_value: f64,
    buf: &std::collections::HashMap<DrawingFieldId, String>,
    muted: Color,
) -> Element<'a, PanelMsg> {
    use iced::widget::{row, text, text_input};
    let display = buf
        .get(&field)
        .cloned()
        .unwrap_or_else(|| format!("{stored_value:.3}"));
    row![
        text(label)
            .size(10)
            .color(muted)
            .width(Length::FillPortion(2)),
        text_input("", &display)
            .size(11)
            .on_input(move |s| PanelMsg::DrawingFieldTyping(field, s))
            .width(Length::FillPortion(3)),
    ]
    .padding([4, PROPERTY_ROW_PAD_X])
    .spacing(6)
    .align_y(iced::Alignment::Center)
    .into()
}

/// Altium-style stroke colour swatch row. A small preset palette
/// (Theme/Red/Green/Blue/Yellow/Orange/White/Black) lets the user
/// recolour a placed shape without committing to a full colour
/// picker. Each tile dispatches UpdateDrawingEdit::StrokeColor.
fn drawing_stroke_color_row<'a>(
    current: Option<signex_types::schematic::StrokeColor>,
    muted: Color,
) -> Element<'a, PanelMsg> {
    use crate::app::contracts::DrawingFieldEdit as E;
    use iced::widget::{button, row, text};
    use signex_types::schematic::StrokeColor;
    let rgb = |r: u8, g: u8, b: u8| -> StrokeColor { StrokeColor { r, g, b, a: 255 } };
    let tile = |label: &'static str,
                stored: Option<StrokeColor>,
                active: bool,
                fill_color: Color|
     -> Element<'a, PanelMsg> {
        button(text(label).size(9))
            .padding([3, 6])
            .on_press(PanelMsg::UpdateDrawingEdit(E::StrokeColor(stored)))
            .style(move |_: &Theme, _| iced::widget::button::Style {
                background: Some(Background::Color(fill_color)),
                border: Border {
                    width: if active { 2.0 } else { 1.0 },
                    radius: 3.0.into(),
                    color: if active {
                        Color::from_rgb(1.0, 1.0, 1.0)
                    } else {
                        Color::from_rgb(0.28, 0.28, 0.32)
                    },
                },
                text_color: Color::WHITE,
                ..iced::widget::button::Style::default()
            })
            .into()
    };
    let is_active = |c: Option<StrokeColor>| -> bool {
        match (current, c) {
            (None, None) => true,
            (Some(a), Some(b)) => a == b,
            _ => false,
        }
    };
    let theme_active = current.is_none();
    row![
        text("Color")
            .size(10)
            .color(muted)
            .width(Length::FillPortion(2)),
        row![
            tile(
                "Auto",
                None,
                theme_active,
                Color::from_rgba(0.28, 0.28, 0.32, 0.6),
            ),
            tile(
                "",
                Some(rgb(0xE5, 0x3E, 0x3E)),
                is_active(Some(rgb(0xE5, 0x3E, 0x3E))),
                Color::from_rgb(0.90, 0.24, 0.24),
            ),
            tile(
                "",
                Some(rgb(0x3E, 0xA5, 0x44)),
                is_active(Some(rgb(0x3E, 0xA5, 0x44))),
                Color::from_rgb(0.24, 0.65, 0.27),
            ),
            tile(
                "",
                Some(rgb(0x3C, 0x85, 0xD6)),
                is_active(Some(rgb(0x3C, 0x85, 0xD6))),
                Color::from_rgb(0.24, 0.52, 0.84),
            ),
            tile(
                "",
                Some(rgb(0xE6, 0xB7, 0x1E)),
                is_active(Some(rgb(0xE6, 0xB7, 0x1E))),
                Color::from_rgb(0.90, 0.72, 0.12),
            ),
            tile(
                "",
                Some(rgb(0xE0, 0xE0, 0xE0)),
                is_active(Some(rgb(0xE0, 0xE0, 0xE0))),
                Color::from_rgb(0.88, 0.88, 0.88),
            ),
        ]
        .spacing(4)
        .width(Length::FillPortion(3)),
    ]
    .padding([4, PROPERTY_ROW_PAD_X])
    .spacing(6)
    .align_y(iced::Alignment::Center)
    .into()
}

fn drawing_fill_row<'a>(
    current: signex_types::schematic::FillType,
    muted: Color,
    _border_c: Color,
) -> Element<'a, PanelMsg> {
    use crate::app::contracts::DrawingFieldEdit as E;
    use signex_types::schematic::FillType;
    let tile = |label: &'static str, ft: FillType, active: bool| -> Element<'a, PanelMsg> {
        iced::widget::button(text(label).size(10))
            .padding([3, 8])
            .on_press(PanelMsg::UpdateDrawingEdit(E::Fill(ft)))
            .style(move |_: &Theme, _| iced::widget::button::Style {
                background: Some(Background::Color(if active {
                    Color::from_rgb(0.20, 0.36, 0.58)
                } else {
                    Color::from_rgba(0.25, 0.25, 0.28, 0.4)
                })),
                border: Border {
                    width: 1.0,
                    radius: 3.0.into(),
                    color: Color::from_rgb(0.28, 0.28, 0.32),
                },
                text_color: if active {
                    Color::from_rgb(1.0, 1.0, 1.0)
                } else {
                    muted
                },
                ..iced::widget::button::Style::default()
            })
            .into()
    };
    row![
        text("Fill")
            .size(10)
            .color(muted)
            .width(Length::FillPortion(2)),
        row![
            tile("None", FillType::None, current == FillType::None),
            tile("Outline", FillType::Outline, current == FillType::Outline),
            tile(
                "Background",
                FillType::Background,
                current == FillType::Background,
            ),
        ]
        .spacing(4)
        .width(Length::FillPortion(3)),
    ]
    .padding([4, PROPERTY_ROW_PAD_X])
    .spacing(6)
    .align_y(iced::Alignment::Center)
    .into()
}

// ─── Drawing preview widget ─────────────────────────────────────

#[derive(Debug, Clone)]
pub struct DrawingPreview {
    pub drawing: signex_types::schematic::SchDrawing,
    pub stroke: Color,
    pub fill: Color,
    pub muted: Color,
    pub accent: Color,
}

impl<Message> canvas::Program<Message> for DrawingPreview {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        use signex_types::schematic::SchDrawing;
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let pad = 14.0_f32;
        let view_w = (bounds.width - 2.0 * pad).max(20.0);
        let view_h = (bounds.height - 2.0 * pad).max(20.0);
        let cx_px = bounds.width / 2.0;
        let cy_px = bounds.height / 2.0;

        let (min_x, min_y, max_x, max_y) = shape_preview_bbox(&self.drawing);
        let span_w = (max_x - min_x).abs().max(0.1);
        let span_h = (max_y - min_y).abs().max(0.1);
        let scale = (view_w as f64 / span_w).min(view_h as f64 / span_h) as f32;
        let wcx = (min_x + max_x) * 0.5;
        let wcy = (min_y + max_y) * 0.5;
        let w2s = |wx: f64, wy: f64| -> Point {
            Point::new(
                cx_px + ((wx - wcx) as f32) * scale,
                cy_px + ((wy - wcy) as f32) * scale,
            )
        };

        let stroke = canvas::Stroke::default()
            .with_color(self.stroke)
            .with_width(1.8);
        let dashed = canvas::Stroke::default()
            .with_color(Color {
                a: 0.4,
                ..self.muted
            })
            .with_width(1.0);
        let annotation = canvas::Stroke::default()
            .with_color(self.accent)
            .with_width(1.4);

        match &self.drawing {
            SchDrawing::Line { start, end, .. } => {
                frame.stroke(
                    &canvas::Path::line(w2s(start.x, start.y), w2s(end.x, end.y)),
                    stroke,
                );
                let dot = |f: &mut canvas::Frame, p: Point| {
                    f.fill(&canvas::Path::circle(p, 3.0), self.accent);
                };
                dot(&mut frame, w2s(start.x, start.y));
                dot(&mut frame, w2s(end.x, end.y));
            }
            SchDrawing::Rect {
                start, end, fill, ..
            } => {
                let x0 = start.x.min(end.x);
                let x1 = start.x.max(end.x);
                let y0 = start.y.min(end.y);
                let y1 = start.y.max(end.y);
                let a = w2s(x0, y0);
                let b = w2s(x1, y1);
                let rect_pos = Point::new(a.x.min(b.x), a.y.min(b.y));
                let rect_size =
                    iced::Size::new((b.x - a.x).abs().max(1.0), (b.y - a.y).abs().max(1.0));
                let path = canvas::Path::rectangle(rect_pos, rect_size);
                if !matches!(fill, signex_types::schematic::FillType::None) {
                    frame.fill(
                        &path,
                        Color {
                            a: 0.25,
                            ..self.fill
                        },
                    );
                }
                frame.stroke(&path, stroke);
            }
            SchDrawing::Circle {
                center,
                radius,
                fill,
                ..
            } => {
                let cp = w2s(center.x, center.y);
                let rs = (*radius as f32) * scale;
                let path = canvas::Path::circle(cp, rs.max(1.0));
                if !matches!(fill, signex_types::schematic::FillType::None) {
                    frame.fill(
                        &path,
                        Color {
                            a: 0.22,
                            ..self.fill
                        },
                    );
                }
                frame.stroke(&path, stroke);
                let spoke = canvas::Path::line(cp, Point::new(cp.x + rs, cp.y));
                frame.stroke(&spoke, annotation);
            }
            SchDrawing::Arc {
                start, mid, end, ..
            } => {
                if let Some((cxw, cyw, rw)) =
                    circumcircle_points_local((start.x, start.y), (mid.x, mid.y), (end.x, end.y))
                {
                    let cp = w2s(cxw, cyw);
                    let rs = (rw as f32) * scale;
                    frame.stroke(&canvas::Path::circle(cp, rs.max(1.0)), dashed);
                    let sa = (start.y - cyw).atan2(start.x - cxw);
                    let ea = (end.y - cyw).atan2(end.x - cxw);
                    let ma = (mid.y - cyw).atan2(mid.x - cxw);
                    let (from, to) = arc_sweep_local(sa, ma, ea);
                    let steps = 64_usize;
                    let mut prev = w2s(start.x, start.y);
                    for i in 1..=steps {
                        let t = i as f64 / steps as f64;
                        let a = from + (to - from) * t;
                        let wx = cxw + rw * a.cos();
                        let wy = cyw + rw * a.sin();
                        let next = w2s(wx, wy);
                        frame.stroke(&canvas::Path::line(prev, next), stroke);
                        prev = next;
                    }
                    frame.stroke(&canvas::Path::line(cp, w2s(start.x, start.y)), annotation);
                    frame.stroke(&canvas::Path::line(cp, w2s(end.x, end.y)), annotation);
                } else {
                    frame.stroke(
                        &canvas::Path::line(w2s(start.x, start.y), w2s(mid.x, mid.y)),
                        stroke,
                    );
                    frame.stroke(
                        &canvas::Path::line(w2s(mid.x, mid.y), w2s(end.x, end.y)),
                        stroke,
                    );
                }
            }
            SchDrawing::Polyline { points, fill, .. } => {
                if points.len() >= 2 {
                    let close = !matches!(fill, signex_types::schematic::FillType::None)
                        && points.len() >= 3;
                    let path = canvas::Path::new(|b| {
                        let first = w2s(points[0].x, points[0].y);
                        b.move_to(first);
                        for p in &points[1..] {
                            b.line_to(w2s(p.x, p.y));
                        }
                        if close {
                            b.close();
                        }
                    });
                    if close {
                        frame.fill(
                            &path,
                            Color {
                                a: 0.22,
                                ..self.fill
                            },
                        );
                    }
                    frame.stroke(&path, stroke);
                    for p in points {
                        let sp = w2s(p.x, p.y);
                        frame.fill(&canvas::Path::circle(sp, 2.5), self.accent);
                    }
                }
            }
        }

        vec![frame.into_geometry()]
    }
}

fn shape_preview_bbox(d: &signex_types::schematic::SchDrawing) -> (f64, f64, f64, f64) {
    use signex_types::schematic::SchDrawing;
    match d {
        SchDrawing::Line { start, end, .. } | SchDrawing::Rect { start, end, .. } => (
            start.x.min(end.x),
            start.y.min(end.y),
            start.x.max(end.x),
            start.y.max(end.y),
        ),
        SchDrawing::Circle { center, radius, .. } => (
            center.x - *radius,
            center.y - *radius,
            center.x + *radius,
            center.y + *radius,
        ),
        SchDrawing::Arc {
            start, mid, end, ..
        } => {
            let xs = [start.x, mid.x, end.x];
            let ys = [start.y, mid.y, end.y];
            let min_x = xs.iter().cloned().fold(f64::INFINITY, f64::min);
            let max_x = xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let min_y = ys.iter().cloned().fold(f64::INFINITY, f64::min);
            let max_y = ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            (min_x, min_y, max_x, max_y)
        }
        SchDrawing::Polyline { points, .. } => {
            if points.is_empty() {
                return (-1.0, -1.0, 1.0, 1.0);
            }
            let mut min_x = f64::INFINITY;
            let mut max_x = f64::NEG_INFINITY;
            let mut min_y = f64::INFINITY;
            let mut max_y = f64::NEG_INFINITY;
            for p in points {
                min_x = min_x.min(p.x);
                min_y = min_y.min(p.y);
                max_x = max_x.max(p.x);
                max_y = max_y.max(p.y);
            }
            (min_x, min_y, max_x, max_y)
        }
    }
}

fn circumcircle_points_local(
    a: (f64, f64),
    b: (f64, f64),
    c: (f64, f64),
) -> Option<(f64, f64, f64)> {
    let (ax, ay) = a;
    let (bx, by) = b;
    let (cx, cy) = c;
    let d = 2.0 * (ax * (by - cy) + bx * (cy - ay) + cx * (ay - by));
    if d.abs() < 1e-9 {
        return None;
    }
    let ux = ((ax * ax + ay * ay) * (by - cy)
        + (bx * bx + by * by) * (cy - ay)
        + (cx * cx + cy * cy) * (ay - by))
        / d;
    let uy = ((ax * ax + ay * ay) * (cx - bx)
        + (bx * bx + by * by) * (ax - cx)
        + (cx * cx + cy * cy) * (bx - ax))
        / d;
    let r = ((ax - ux) * (ax - ux) + (ay - uy) * (ay - uy)).sqrt();
    Some((ux, uy, r))
}

fn arc_sweep_local(s: f64, m: f64, e: f64) -> (f64, f64) {
    use std::f64::consts::TAU;
    let norm = |a: f64| -> f64 {
        let mut t = a % TAU;
        if t < 0.0 {
            t += TAU;
        }
        t
    };
    let ccw = |a: f64, b: f64| -> f64 {
        let d = b - a;
        if d < 0.0 { d + TAU } else { d }
    };
    let sn = norm(s);
    let mn = norm(m);
    let en = norm(e);
    let s_to_m = ccw(sn, mn);
    let s_to_e = ccw(sn, en);
    if s_to_m <= s_to_e {
        (s, s + s_to_e)
    } else {
        (s, s - (TAU - s_to_e))
    }
}
