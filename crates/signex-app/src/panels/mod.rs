//! Panel implementations — uses signex-widgets for proper Altium-style content.

use iced::widget::{Column, Row, Space, column, container, row, scrollable, svg, text};
use iced::{Background, Border, Color, Element, Length, Theme};
use iced_aw::{NumberInput, Wrap};
use signex_types::coord::Unit;
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;
use signex_widgets::tree_view::{TreeIcon, TreeMsg, TreeNode, TreeView};
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum PanelKind {
    Projects,
    Components,
    Navigator,
    Properties,
    Filter,
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
}

/// Context passed to panels — owned data to avoid lifetime issues.
#[derive(Debug, Clone)]
pub struct LibrarySymbolEntry {
    pub lib_id: String,
    pub symbol_name: String,
    pub library_name: String,
    pub pin_count: usize,
}

pub struct PanelContext {
    pub project_name: Option<String>,
    pub project_file: Option<String>,
    pub pcb_file: Option<String>,
    pub sheets: Vec<SheetInfo>,
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
    pub kicad_libraries: Vec<String>,
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
    /// Component search filter text.
    pub component_filter: String,
    /// Which sections are collapsed (by section name key).
    pub collapsed_sections: CollapsedSections,
    /// Pre-placement configuration (shown when Tab pressed during placement tool).
    pub pre_placement: Option<PrePlacementData>,
    /// Current diagnostics level resolved from SIGNEX_LOG / RUST_LOG.
    pub diagnostics_level: String,
    /// Recent application diagnostics shown in the Messages panel.
    pub diagnostics: Vec<crate::diagnostics::DiagnosticEntry>,
    /// Selection filter state, shared with the Active Bar.
    pub selection_filters: std::collections::HashSet<crate::active_bar::SelectionFilter>,
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
    Other,
}

/// Panel-level message wrapping widget messages.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum PanelMsg {
    Tree(TreeMsg),
    SetUnit(Unit),
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

// ─── Projects Panel (TreeView) ────────────────────────────────

/// Build the project tree from panel context data.
/// Called once on project load; result is stored in `PanelContext::project_tree`.
pub fn build_project_tree(ctx: &PanelContext) -> Vec<TreeNode> {
    let Some(name) = &ctx.project_name else {
        return vec![];
    };

    // Source documents from project sheets
    let mut source_docs: Vec<TreeNode> = vec![];

    if !ctx.sheets.is_empty() {
        for sheet in &ctx.sheets {
            source_docs.push(TreeNode::leaf(sheet.filename.clone(), TreeIcon::Schematic));
        }
    } else if let Some(file) = &ctx.project_file {
        source_docs.push(TreeNode::leaf(file.clone(), TreeIcon::Schematic));
    }

    // PCB file
    if let Some(pcb) = &ctx.pcb_file {
        source_docs.push(TreeNode::leaf(pcb.clone(), TreeIcon::Pcb));
    }

    // Libraries
    let lib_count = if ctx.lib_symbol_count > 0 {
        ctx.lib_symbol_count
    } else {
        ctx.sheets.iter().map(|s| s.sym_count).sum::<usize>()
    };
    let lib_children = vec![TreeNode::leaf(
        format!("{} symbols loaded", lib_count),
        TreeIcon::Component,
    )];

    let mut settings = TreeNode::branch("Settings".to_string(), TreeIcon::File, vec![]);
    settings.expanded = false;

    vec![TreeNode::branch(
        name.clone(),
        TreeIcon::Folder,
        vec![
            TreeNode::branch(
                "Source Documents".to_string(),
                TreeIcon::Folder,
                source_docs,
            ),
            TreeNode::branch("Libraries".to_string(), TreeIcon::Library, lib_children),
            settings,
        ],
    )]
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
        // Render the persistent tree — toggle state is preserved
        TreeView::new(&ctx.project_tree, &ctx.tokens)
            .view()
            .map(PanelMsg::Tree)
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
                ctx.kicad_libraries.clone(),
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

    if let Some(name) = &ctx.project_name {
        let mut sheets = vec![];
        for cs in &ctx.child_sheets {
            sheets.push(TreeNode::leaf(cs.clone(), TreeIcon::Sheet));
        }
        let roots = vec![TreeNode::branch(name.clone(), TreeIcon::Schematic, sheets)];
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

    if !ctx.has_schematic {
        return container(
            column![
                text("Properties").size(12).color(primary),
                Space::new().height(12.0),
                text("Open a project").size(11).color(muted),
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
                        c = c.push(form_edit_row("Footprint", &footprint, muted, move |s| {
                            PanelMsg::EditSymbolFootprint(id, s)
                        }));
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
            }
        }
        Some(signex_types::schematic::SelectedKind::SymbolRefField)
        | Some(signex_types::schematic::SelectedKind::SymbolValField) => {
            let text_value = get("Text");
            let position = get("Position");
            let rotation = get("Rotation");
            let text_size = get("Text Size");
            let justify_h = get("Horizontal Justification");
            let justify_v = get("Vertical Justification");
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
                            "Horizontal Justification",
                            &justify_h,
                            muted,
                            input_bg,
                            input_bdr,
                        ));
                        c = c.push(form_input_row(
                            "Vertical Justification",
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
            // Net Name stored in KiCad escapes `/` as `{slash}`. Show the
            // visible form in the panel; the edit handler re-escapes on save.
            let label_text = signex_render::schematic::text::expand_char_escapes(&get("Text"));
            let position = get("Position");
            let rotation_str = get("Rotation");
            let text_size_str = get("Text Size");
            let justify_h_str = get("Horizontal Justification");

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
                                id, justify_h, input_bg, input_bdr, primary, muted,
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
            let justify_h = get("Horizontal Justification");
            let justify_v = get("Vertical Justification");

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
                            "Horizontal Justification",
                            &justify_h,
                            muted,
                            input_bg,
                            input_bdr,
                        ));
                        c = c.push(form_input_row(
                            "Vertical Justification",
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
                            justify_h, input_bg, input_bdr, primary, muted,
                        ))
                        .padding([4, 8]),
                    );
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

    // Selection Filter (collapsible) — shares state with the Active Bar filter dropdown.
    {
        use crate::active_bar::SelectionFilter;
        let filters = ctx.selection_filters.clone();
        let all_on = filters.len() == SelectionFilter::ALL.len();
        col = col.push(collapsible_section(
            "prop_sel_filter",
            "Selection Filter",
            &ctx.collapsed_sections,
            primary,
            border_c,
            move || {
                // All-On/Off toggle row
                let all_active_bg = Color::from_rgba8(0x2E, 0x33, 0x45, 1.0);
                let all_inactive_bg = Color::from_rgba8(0x1A, 0x1D, 0x28, 1.0);
                let all_active_border = Color::from_rgba8(0x4D, 0x52, 0x66, 1.0);
                let all_inactive_border = Color::from_rgba8(0x33, 0x36, 0x44, 1.0);
                let all_text_off = Color::from_rgba8(0x66, 0x6A, 0x7E, 1.0);
                let all_text_on = Color::WHITE;
                let all_label = if all_on { "All - On" } else { "All - Off" };
                let all_toggle = iced::widget::button(
                    text(all_label.to_string())
                        .size(10)
                        .color(if all_on { all_text_on } else { all_text_off })
                        .align_x(iced::alignment::Horizontal::Center),
                )
                .padding([3, 10])
                .on_press(PanelMsg::ToggleAllSelectionFilters)
                .style(move |_: &Theme, status: iced::widget::button::Status| {
                    let bg = match status {
                        iced::widget::button::Status::Hovered => Background::Color(tag_hover),
                        _ => Background::Color(if all_on {
                            all_active_bg
                        } else {
                            all_inactive_bg
                        }),
                    };
                    iced::widget::button::Style {
                        background: Some(bg),
                        border: Border {
                            width: 1.0,
                            radius: 12.0.into(),
                            color: if all_on {
                                all_active_border
                            } else {
                                all_inactive_border
                            },
                        },
                        text_color: if all_on { all_text_on } else { all_text_off },
                        ..iced::widget::button::Style::default()
                    }
                });
                let mut c = Column::new().spacing(4).width(Length::Fill);
                c = c.push(container(all_toggle).padding([4, 8]));
                c = c.push(
                    container(
                        Wrap::new()
                            .spacing(4.0)
                            .line_spacing(4.0)
                            .push(tag_btn(
                                "Components",
                                SelectionFilter::Components,
                                filters.contains(&SelectionFilter::Components),
                                tag_hover,
                            ))
                            .push(tag_btn(
                                "Wires",
                                SelectionFilter::Wires,
                                filters.contains(&SelectionFilter::Wires),
                                tag_hover,
                            ))
                            .push(tag_btn(
                                "Buses",
                                SelectionFilter::Buses,
                                filters.contains(&SelectionFilter::Buses),
                                tag_hover,
                            ))
                            .push(tag_btn(
                                "Sheet Symbols",
                                SelectionFilter::SheetSymbols,
                                filters.contains(&SelectionFilter::SheetSymbols),
                                tag_hover,
                            ))
                            .push(tag_btn(
                                "Sheet Entries",
                                SelectionFilter::SheetEntries,
                                filters.contains(&SelectionFilter::SheetEntries),
                                tag_hover,
                            ))
                            .push(tag_btn(
                                "Net Labels",
                                SelectionFilter::NetLabels,
                                filters.contains(&SelectionFilter::NetLabels),
                                tag_hover,
                            ))
                            .push(tag_btn(
                                "Parameters",
                                SelectionFilter::Parameters,
                                filters.contains(&SelectionFilter::Parameters),
                                tag_hover,
                            ))
                            .push(tag_btn(
                                "Ports",
                                SelectionFilter::Ports,
                                filters.contains(&SelectionFilter::Ports),
                                tag_hover,
                            ))
                            .push(tag_btn(
                                "Power Ports",
                                SelectionFilter::PowerPorts,
                                filters.contains(&SelectionFilter::PowerPorts),
                                tag_hover,
                            ))
                            .push(tag_btn(
                                "Texts",
                                SelectionFilter::Texts,
                                filters.contains(&SelectionFilter::Texts),
                                tag_hover,
                            ))
                            .push(tag_btn(
                                "Drawing Objects",
                                SelectionFilter::DrawingObjects,
                                filters.contains(&SelectionFilter::DrawingObjects),
                                tag_hover,
                            ))
                            .push(tag_btn(
                                "Other",
                                SelectionFilter::Other,
                                filters.contains(&SelectionFilter::Other),
                                tag_hover,
                            )),
                    )
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
    h: signex_types::schematic::HAlign,
    input_bg: Color,
    input_bdr: Color,
    primary: Color,
    muted: Color,
) -> Element<'static, PanelMsg> {
    use signex_types::schematic::HAlign;
    use std::sync::LazyLock;
    static ICON_TL: LazyLock<iced::widget::svg::Handle> = LazyLock::new(|| {
        iced::widget::svg::Handle::from_memory(include_bytes!("../../assets/icons/justify/tl.svg"))
    });
    static ICON_T: LazyLock<iced::widget::svg::Handle> = LazyLock::new(|| {
        iced::widget::svg::Handle::from_memory(include_bytes!("../../assets/icons/justify/t.svg"))
    });
    static ICON_TR: LazyLock<iced::widget::svg::Handle> = LazyLock::new(|| {
        iced::widget::svg::Handle::from_memory(include_bytes!("../../assets/icons/justify/tr.svg"))
    });
    static ICON_L: LazyLock<iced::widget::svg::Handle> = LazyLock::new(|| {
        iced::widget::svg::Handle::from_memory(include_bytes!("../../assets/icons/justify/l.svg"))
    });
    static ICON_C: LazyLock<iced::widget::svg::Handle> = LazyLock::new(|| {
        iced::widget::svg::Handle::from_memory(include_bytes!("../../assets/icons/justify/c.svg"))
    });
    static ICON_R: LazyLock<iced::widget::svg::Handle> = LazyLock::new(|| {
        iced::widget::svg::Handle::from_memory(include_bytes!("../../assets/icons/justify/r.svg"))
    });
    static ICON_BL: LazyLock<iced::widget::svg::Handle> = LazyLock::new(|| {
        iced::widget::svg::Handle::from_memory(include_bytes!("../../assets/icons/justify/bl.svg"))
    });
    static ICON_B: LazyLock<iced::widget::svg::Handle> = LazyLock::new(|| {
        iced::widget::svg::Handle::from_memory(include_bytes!("../../assets/icons/justify/b.svg"))
    });
    static ICON_BR: LazyLock<iced::widget::svg::Handle> = LazyLock::new(|| {
        iced::widget::svg::Handle::from_memory(include_bytes!("../../assets/icons/justify/br.svg"))
    });
    let _ = muted;

    // Cell size mimics Altium's compact 24×24 px anchor picker.
    const CELL_SIZE: f32 = 24.0;
    let cell = |handle: &LazyLock<iced::widget::svg::Handle>,
                active: bool,
                on_press: PanelMsg|
     -> Element<'static, PanelMsg> {
        let bg_active = input_bdr;
        let fg_active = Color::WHITE;
        let fg_inactive = primary;
        let svg_widget = iced::widget::svg((*handle).clone())
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
    // Only the middle-row cell of the matching column lights up — labels in
    // KiCad have no vertical-justify field, so the top/bottom rows of the
    // 9-grid are visual controls only (a future addition).
    let hl_mid = |target: HAlign| -> bool { h == target };
    iced::widget::column![
        iced::widget::row![
            cell(
                &ICON_TL,
                false,
                PanelMsg::EditLabelJustifyH(id, HAlign::Left)
            ),
            cell(
                &ICON_T,
                false,
                PanelMsg::EditLabelJustifyH(id, HAlign::Center)
            ),
            cell(
                &ICON_TR,
                false,
                PanelMsg::EditLabelJustifyH(id, HAlign::Right)
            ),
        ]
        .spacing(2),
        iced::widget::row![
            cell(
                &ICON_L,
                hl_mid(HAlign::Left),
                PanelMsg::EditLabelJustifyH(id, HAlign::Left)
            ),
            cell(
                &ICON_C,
                hl_mid(HAlign::Center),
                PanelMsg::EditLabelJustifyH(id, HAlign::Center)
            ),
            cell(
                &ICON_R,
                hl_mid(HAlign::Right),
                PanelMsg::EditLabelJustifyH(id, HAlign::Right)
            ),
        ]
        .spacing(2),
        iced::widget::row![
            cell(
                &ICON_BL,
                false,
                PanelMsg::EditLabelJustifyH(id, HAlign::Left)
            ),
            cell(
                &ICON_B,
                false,
                PanelMsg::EditLabelJustifyH(id, HAlign::Center)
            ),
            cell(
                &ICON_BR,
                false,
                PanelMsg::EditLabelJustifyH(id, HAlign::Right)
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
) -> Element<'static, PanelMsg> {
    use signex_types::schematic::HAlign;
    use std::sync::LazyLock;
    static ICON_TL: LazyLock<iced::widget::svg::Handle> = LazyLock::new(|| {
        iced::widget::svg::Handle::from_memory(include_bytes!("../../assets/icons/justify/tl.svg"))
    });
    static ICON_T: LazyLock<iced::widget::svg::Handle> = LazyLock::new(|| {
        iced::widget::svg::Handle::from_memory(include_bytes!("../../assets/icons/justify/t.svg"))
    });
    static ICON_TR: LazyLock<iced::widget::svg::Handle> = LazyLock::new(|| {
        iced::widget::svg::Handle::from_memory(include_bytes!("../../assets/icons/justify/tr.svg"))
    });
    static ICON_L: LazyLock<iced::widget::svg::Handle> = LazyLock::new(|| {
        iced::widget::svg::Handle::from_memory(include_bytes!("../../assets/icons/justify/l.svg"))
    });
    static ICON_C: LazyLock<iced::widget::svg::Handle> = LazyLock::new(|| {
        iced::widget::svg::Handle::from_memory(include_bytes!("../../assets/icons/justify/c.svg"))
    });
    static ICON_R: LazyLock<iced::widget::svg::Handle> = LazyLock::new(|| {
        iced::widget::svg::Handle::from_memory(include_bytes!("../../assets/icons/justify/r.svg"))
    });
    static ICON_BL: LazyLock<iced::widget::svg::Handle> = LazyLock::new(|| {
        iced::widget::svg::Handle::from_memory(include_bytes!("../../assets/icons/justify/bl.svg"))
    });
    static ICON_B: LazyLock<iced::widget::svg::Handle> = LazyLock::new(|| {
        iced::widget::svg::Handle::from_memory(include_bytes!("../../assets/icons/justify/b.svg"))
    });
    static ICON_BR: LazyLock<iced::widget::svg::Handle> = LazyLock::new(|| {
        iced::widget::svg::Handle::from_memory(include_bytes!("../../assets/icons/justify/br.svg"))
    });
    let _ = muted;

    const CELL_SIZE: f32 = 24.0;
    let cell = |handle: &LazyLock<iced::widget::svg::Handle>,
                active: bool,
                on_press: PanelMsg|
     -> Element<'static, PanelMsg> {
        let bg_active = input_bdr;
        let fg_active = Color::WHITE;
        let fg_inactive = primary;
        let svg_widget = iced::widget::svg((*handle).clone())
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
                &ICON_TL,
                false,
                PanelMsg::SetPrePlacementJustifyH(HAlign::Left)
            ),
            cell(
                &ICON_T,
                false,
                PanelMsg::SetPrePlacementJustifyH(HAlign::Center)
            ),
            cell(
                &ICON_TR,
                false,
                PanelMsg::SetPrePlacementJustifyH(HAlign::Right)
            ),
        ]
        .spacing(2),
        iced::widget::row![
            cell(
                &ICON_L,
                hl_mid(HAlign::Left),
                PanelMsg::SetPrePlacementJustifyH(HAlign::Left)
            ),
            cell(
                &ICON_C,
                hl_mid(HAlign::Center),
                PanelMsg::SetPrePlacementJustifyH(HAlign::Center)
            ),
            cell(
                &ICON_R,
                hl_mid(HAlign::Right),
                PanelMsg::SetPrePlacementJustifyH(HAlign::Right)
            ),
        ]
        .spacing(2),
        iced::widget::row![
            cell(
                &ICON_BL,
                false,
                PanelMsg::SetPrePlacementJustifyH(HAlign::Left)
            ),
            cell(
                &ICON_B,
                false,
                PanelMsg::SetPrePlacementJustifyH(HAlign::Center)
            ),
            cell(
                &ICON_BR,
                false,
                PanelMsg::SetPrePlacementJustifyH(HAlign::Right)
            ),
        ]
        .spacing(2),
    ]
    .spacing(2)
    .into()
}

/// Selection filter tag button — Altium pill with active/inactive state.
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

// ─── Messages Panel ───────────────────────────────────────────

fn view_messages<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let mut col: Column<'a, PanelMsg> = Column::new().spacing(2).padding(6).width(Length::Fill);
    col = col.push(
        row![
            section_title("Messages", &ctx.tokens),
            Space::new().width(Length::Fill).height(Length::Shrink),
            text(format!("level {}", ctx.diagnostics_level))
                .size(9)
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
        for entry in ctx.diagnostics.iter().rev() {
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

            col = col.push(
                container(
                    column![
                        row![
                            text(format!("#{}", entry.id))
                                .size(9)
                                .color(theme_ext::text_secondary(&ctx.tokens)),
                            Space::new().width(6).height(Length::Shrink),
                            text(entry.level.label()).size(9).color(level_color),
                            Space::new().width(6).height(Length::Shrink),
                            text(entry.code.as_str())
                                .size(9)
                                .color(theme_ext::text_secondary(&ctx.tokens)),
                        ]
                        .align_y(iced::Alignment::Center),
                        text(entry.message.as_str())
                            .size(10)
                            .color(theme_ext::text_primary(&ctx.tokens)),
                    ]
                    .spacing(3),
                )
                .padding([5, 6])
                .style(move |_theme: &Theme| iced::widget::container::Style {
                    background: Some(Background::Color(theme_ext::to_color(&ctx.tokens.panel_bg))),
                    border: Border {
                        width: 1.0,
                        radius: 0.0.into(),
                        color: theme_ext::border_color(&ctx.tokens),
                    },
                    ..iced::widget::container::Style::default()
                }),
            );
        }
    }

    container(col).width(Length::Fill).into()
}
