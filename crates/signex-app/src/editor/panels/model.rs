//! Panel models and message contracts.

use signex_types::coord::Unit;
use signex_types::theme::ThemeTokens;
use signex_widgets::tree_view::{TreeMsg, TreeNode};

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
}

/// All available panel kinds for the panel list button.
pub const ALL_PANELS: &[PanelKind] = &[
    PanelKind::Projects,
    PanelKind::Components,
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
    /// Display name (project stem — "MyBoard" from "MyBoard.snxprj").
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
    /// Whether this is the currently-active project — drives accent
    /// styling on the root node.
    pub is_active: bool,
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
    // Components panel — repopulated by the v0.10.x `.snxlib` library
    // plumbing. The legacy symbol-library scanner that previously
    // fed these was removed in v0.10.0 (Apache-clean residual polish);
    // the panel now shows a placeholder until the new plumbing lands.
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
    /// Currently highlighted project-tree row. Set by single-click;
    /// double-click on the same row opens the file. `None` when no row
    /// has been clicked since the last refresh. Path indices into
    /// `project_tree` (matches the `TreeMsg::Select(path)` payload).
    pub selected_tree_path: Option<Vec<usize>>,
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
    /// Polygon). 0 = default ≈ 0.15 mm.
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

