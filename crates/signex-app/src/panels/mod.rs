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
    ActiveBom,
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
    PanelKind::ActiveBom,
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
            PanelKind::ActiveBom => "ActiveBOM",
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
    pub wire_count: usize,
    #[allow(dead_code)]
    pub label_count: usize,
}

/// Context passed to panels — owned data to avoid lifetime issues.
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
    /// (symbol_name, pin_count) from the currently loaded library.
    pub library_symbols: Vec<(String, usize)>,
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
}

/// Pre-placement configuration data — shown in Properties panel when Tab pressed.
#[derive(Debug, Clone)]
pub struct PrePlacementData {
    /// Which tool is being configured.
    pub tool_name: String,
    /// Net label / text note text.
    pub label_text: String,
    /// Component designator override.
    pub designator: String,
    /// Rotation (degrees).
    pub rotation: f64,
}

/// Panel-level message wrapping widget messages.
#[derive(Debug, Clone)]
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
    /// Edit a label's text (committed on submit).
    EditLabelText(uuid::Uuid, String),
    /// Edit a text note's text (committed on submit).
    EditTextNoteText(uuid::Uuid, String),
    /// Pre-placement: update label/text field.
    SetPrePlacementText(String),
    /// Pre-placement: update designator field.
    SetPrePlacementDesignator(String),
    /// Pre-placement: update rotation.
    SetPrePlacementRotation(f64),
    /// Pre-placement: confirm and close.
    ConfirmPrePlacement,
    /// Set snap grid size (mm).
    SetGridSize(f32),
    /// Set visible grid size (mm) — independent of snap grid.
    SetVisibleGridSize(f32),
    /// Toggle snap to electrical object hotspots.
    ToggleSnapHotspots,
    /// Change the UI font (saved to prefs; applies on next restart).
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
        PanelKind::ActiveBom => view_stub("ActiveBOM", "Bill of Materials management", ctx),
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
                iced::widget::button::Status::Hovered => {
                    Some(iced::Background::Color(border_c))
                }
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
            text(key.to_string()).size(10).color(key_c).width(100),
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
            let badge = format!("{}c {}w", sheet.sym_count, sheet.wire_count);
            source_docs.push(
                TreeNode::leaf(sheet.filename.clone(), TreeIcon::Schematic).with_badge(badge),
            );
        }
    } else if let Some(file) = &ctx.project_file {
        source_docs.push(
            TreeNode::leaf(file.clone(), TreeIcon::Schematic)
                .with_badge(format!("{}c {}w", ctx.sym_count, ctx.wire_count)),
        );
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
    let lib_children = vec![
        TreeNode::leaf(format!("{} symbols loaded", lib_count), TreeIcon::Component)
            .with_badge(lib_count.to_string()),
    ];

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
                    .width(Length::FillPortion(4)),
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
    let filtered_symbols: Vec<&(String, usize)> = if filter.is_empty() {
        ctx.library_symbols.iter().collect()
    } else {
        ctx.library_symbols
            .iter()
            .filter(|(name, _)| name.to_ascii_lowercase().contains(&filter))
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
        for &(name, pins) in &filtered_symbols {
            let is_sel = sel.as_deref() == Some(name.as_str());
            let row_bg = if is_sel {
                theme_ext::selection_color(&ctx.tokens)
            } else {
                Color::TRANSPARENT
            };
            let name_c = if is_sel { Color::WHITE } else { primary };
            let n = name.clone();
            list_col = list_col.push(
                column![
                    iced::widget::button(
                        row![
                            text(name.clone())
                                .size(10)
                                .color(name_c)
                                .width(Length::FillPortion(4))
                                .wrapping(iced::widget::text::Wrapping::None),
                            text(pins.to_string())
                                .size(10)
                                .color(muted)
                                .width(Length::FillPortion(1)),
                        ]
                        .spacing(4.0),
                    )
                    .padding([3, 8])
                    .width(Length::Fill)
                    .on_press(PanelMsg::SelectComponent(n))
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

    if let Some(comp_name) = &ctx.selected_component {
        detail_col = detail_col.push(section_hdr(
            &format!("\u{25BC} Details  {comp_name}"),
            primary,
            border_c,
        ));
        let pin_count = ctx
            .library_symbols
            .iter()
            .find(|(n, _)| n == comp_name)
            .map(|(_, p)| *p)
            .unwrap_or(0);
        detail_col = detail_col.push(form_input_row("Symbol", comp_name, muted, input_bg, input_bdr));
        detail_col = detail_col.push(form_input_row("Pins", &pin_count.to_string(), muted, input_bg, input_bdr));
        detail_col = detail_col.push(form_input_row(
            "Library",
            ctx.active_library.as_deref().unwrap_or(""),
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

const LABEL_W: f32 = 90.0;

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
    if let Some(ref pp) = ctx.pre_placement {
        return view_pre_placement(pp, muted, primary, border_c, input_bg, input_bdr);
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
                props_tab_btn("General", tab == 0, PanelMsg::PropertiesTab(0), primary, text_inactive, tab_hover, border_c),
                props_tab_btn("Parameters", tab == 1, PanelMsg::PropertiesTab(1), primary, text_inactive, tab_hover, border_c),
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
            muted, primary, border_c,
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
    let input_bg  = crate::styles::ti(ctx.tokens.selection);
    let input_bdr = crate::styles::ti(ctx.tokens.accent);
    let mut col: Column<'a, PanelMsg> = Column::new().spacing(0).width(Length::Fill);

    let elem_type = ctx
        .selection_info
        .iter()
        .find(|(k, _)| k == "Type")
        .map(|(_, v)| v.as_str())
        .unwrap_or("Object");

    let uuid = ctx.selected_uuid;

    // ── Header ──
    col = col.push(
        container(text(elem_type.to_owned()).size(11).color(primary))
            .padding([6, 8])
            .width(Length::Fill),
    );
    col = col.push(thin_sep(border_c));

    // ── Editable properties based on element type ──
    match elem_type {
        "Symbol" => {
            let get = |key: &str| -> String {
                ctx.selection_info
                    .iter()
                    .find(|(k, _)| k == key)
                    .map(|(_, v)| v.clone())
                    .unwrap_or_default()
            };
            let reference = get("Reference");
            let value = get("Value");
            let footprint = get("Footprint");
            let lib_id = get("Library ID");
            let position = get("Position");
            let rotation = get("Rotation");
            let has_mirror_x = ctx.selection_info.iter().any(|(k, v)| k == "Mirror" && v == "X");
            let has_mirror_y = ctx.selection_info.iter().any(|(k, v)| k == "Mirror" && v == "Y");

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
                        c = c.push(form_edit_row("Designator", &reference, muted,
                            move |s| PanelMsg::EditSymbolDesignator(id, s)));
                        c = c.push(form_edit_row("Value", &value, muted,
                            move |s| PanelMsg::EditSymbolValue(id, s)));
                        c = c.push(form_edit_row("Footprint", &footprint, muted,
                            move |s| PanelMsg::EditSymbolFootprint(id, s)));
                        c = c.push(form_input_row("Library ID", &lib_id, muted, input_bg, input_bdr));
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
                        c = c.push(form_input_row("Position", &position, muted, input_bg, input_bdr));
                        c = c.push(form_input_row("Rotation", &rotation, muted, input_bg, input_bdr));
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
                        c = c.push(form_check_row("Mirror X", has_mirror_x,
                            PanelMsg::ToggleSymbolMirrorX(id), muted));
                        c = c.push(form_check_row("Mirror Y", has_mirror_y,
                            PanelMsg::ToggleSymbolMirrorY(id), muted));
                        c = c.push(form_check_row("Locked", false,
                            PanelMsg::ToggleSymbolLocked(id), muted));
                        c = c.push(form_check_row("DNP", false,
                            PanelMsg::ToggleSymbolDnp(id), muted));
                        c
                    },
                ));
            }
        }
        "Label" | "Global Label" | "Hierarchical Label" => {
            let label_text = ctx.selection_info.iter()
                .find(|(k, _)| k == "Text")
                .map(|(_, v)| v.clone())
                .unwrap_or_default();
            let position = ctx.selection_info.iter()
                .find(|(k, _)| k == "Position")
                .map(|(_, v)| v.clone())
                .unwrap_or_default();

            if let Some(id) = uuid {
                col = col.push(collapsible_section(
                    "sel_general",
                    "General",
                    &ctx.collapsed_sections,
                    muted,
                    border_c,
                    move || {
                        let mut c = Column::new().spacing(0).width(Length::Fill);
                        c = c.push(form_edit_row("Text", &label_text, muted,
                            move |s| PanelMsg::EditLabelText(id, s)));
                        c = c.push(form_input_row("Position", &position, muted, input_bg, input_bdr));
                        c
                    },
                ));
            }
        }
        "Text Note" => {
            let note_text = ctx.selection_info.iter()
                .find(|(k, _)| k == "Text")
                .map(|(_, v)| v.clone())
                .unwrap_or_default();

            if let Some(id) = uuid {
                col = col.push(collapsible_section(
                    "sel_general",
                    "General",
                    &ctx.collapsed_sections,
                    muted,
                    border_c,
                    move || {
                        let mut c = Column::new().spacing(0).width(Length::Fill);
                        c = c.push(form_edit_row("Text", &note_text, muted,
                            move |s| PanelMsg::EditTextNoteText(id, s)));
                        c
                    },
                ));
            }
        }
        _ => {
            // Generic read-only properties for other types
            let info: Vec<(String, String)> = ctx.selection_info
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

    let mut col: Column<'a, PanelMsg> = Column::new().spacing(0).width(Length::Fill);

    // Header
    col = col.push(
        container(
            row![
                text(format!("{} Properties", tool_name))
                    .size(12)
                    .color(primary),
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

    // Separator
    col = col.push(
        container(Space::new())
            .height(1)
            .width(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(border_c)),
                ..container::Style::default()
            }),
    );

    // Fields based on tool type
    col = col.push(
        container(
            column![
                // Text / Net Name field
                container(
                    row![
                        text("Text / Name").size(11).color(muted).width(LABEL_W),
                        iced::widget::text_input("Enter text...", &label_text)
                            .on_input(PanelMsg::SetPrePlacementText)
                            .size(11)
                            .padding(4)
                            .width(Length::Fill),
                    ]
                    .spacing(8)
                    .align_y(iced::Alignment::Center),
                )
                .padding([4, 8]),
                // Designator field
                container(
                    row![
                        text("Designator").size(11).color(muted).width(LABEL_W),
                        iced::widget::text_input("e.g. R1, U1", &designator)
                            .on_input(PanelMsg::SetPrePlacementDesignator)
                            .size(11)
                            .padding(4)
                            .width(Length::Fill),
                    ]
                    .spacing(8)
                    .align_y(iced::Alignment::Center),
                )
                .padding([4, 8]),
                // Rotation
                form_input_row("Rotation", &format!("{rotation:.0}°"), muted, input_bg, input_bdr),
            ]
            .spacing(0),
        )
        .width(Length::Fill),
    );

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
    let input_bg   = crate::styles::ti(ctx.tokens.selection); // deep blue tint
    let input_bdr  = crate::styles::ti(ctx.tokens.accent);
    let tag_bg     = crate::styles::ti(ctx.tokens.accent);
    let tag_hover  = {
        let c = crate::styles::ti(ctx.tokens.accent);
        Color { r: (c.r * 1.3).min(1.0), g: (c.g * 1.3).min(1.0), b: (c.b * 1.3).min(1.0), ..c }
    };
    let seg_hover  = crate::styles::ti(ctx.tokens.hover);

    let mut col: Column<'a, PanelMsg> = Column::new().spacing(0).width(Length::Fill);

    // Selection Filter (collapsible)
    col = col.push(collapsible_section(
        "prop_sel_filter",
        "Selection Filter",
        &ctx.collapsed_sections,
        primary,
        border_c,
        || {
            let mut c = Column::new().spacing(0).width(Length::Fill);
            c = c.push(
                container(
                    Wrap::new()
                        .spacing(4.0)
                        .line_spacing(4.0)
                        .push(tag_btn("Components", tag_bg, tag_hover))
                        .push(tag_btn("Wires", tag_bg, tag_hover))
                        .push(tag_btn("Buses", tag_bg, tag_hover))
                        .push(tag_btn("Sheet Symbols", tag_bg, tag_hover))
                        .push(tag_btn("Sheet Entries", tag_bg, tag_hover))
                        .push(tag_btn("Net Labels", tag_bg, tag_hover))
                        .push(tag_btn("Parameters", tag_bg, tag_hover))
                        .push(tag_btn("Ports", tag_bg, tag_hover))
                        .push(tag_btn("Power Ports", tag_bg, tag_hover))
                        .push(tag_btn("Texts", tag_bg, tag_hover))
                        .push(tag_btn("Drawing Objects", tag_bg, tag_hover))
                        .push(tag_btn("Other", tag_bg, tag_hover)),
                )
                .padding([6, 8]),
            );
            c
        },
    ));

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
                            seg_btn("mm", unit == Unit::Mm, PanelMsg::SetUnit(Unit::Mm), input_bg, primary, muted, seg_hover, input_bdr),
                            seg_btn("mils", unit == Unit::Mil, PanelMsg::SetUnit(Unit::Mil), input_bg, primary, muted, seg_hover, input_bdr),
                        ]
                        .spacing(0.0)
                        .width(Length::Fill),
                    )
                    .padding([2, 8]),
                );
                // Altium-style: Visible Grid and Snap Grid are independent
                c = c.push(form_grid_row("Visible Grid", visible_grid_mm, unit, false, PanelMsg::SetVisibleGridSize, muted, grid_visible, PanelMsg::ToggleGrid));
                c = c.push(form_grid_row("Snap Grid", grid_size_mm, unit, true, PanelMsg::SetGridSize, muted, snap_enabled, PanelMsg::ToggleSnap));
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
                c = c.push(form_input_row("Sheet Color", "Black", muted, input_bg, input_bdr));
                c
            },
        ));
    }

    // Page Options (collapsible)
    {
        let paper_size = ctx.paper_size.clone();
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
                            seg_btn("Template", true, PanelMsg::Noop, input_bg, primary, muted, seg_hover, input_bdr),
                            seg_btn("Standard", false, PanelMsg::Noop, input_bg, primary, muted, seg_hover, input_bdr),
                            seg_btn("Custom", false, PanelMsg::Noop, input_bg, primary, muted, seg_hover, input_bdr),
                        ]
                        .spacing(0.0)
                        .width(Length::Fill),
                    )
                    .padding([2, 8]),
                );
                c = c.push(form_input_row("Paper", &paper_size, muted, input_bg, input_bdr));
                let dims = match paper_size.as_str() {
                    "A4" => "Width: 297mm  Height: 210mm",
                    "A3" => "Width: 420mm  Height: 297mm",
                    _ => "Width: 297mm  Height: 210mm",
                };
                c = c.push(container(text(dims.to_string()).size(10).color(muted)).padding([3, 8]));
                c = c.push(form_label("Margin and Zones", muted));
                c = c.push(form_number_row(
                    "Vertical",
                    1_u32,
                    0..=10,
                    1,
                    PanelMsg::SetMarginVertical,
                    muted,
                ));
                c = c.push(form_number_row(
                    "Horizontal",
                    1_u32,
                    0..=10,
                    1,
                    PanelMsg::SetMarginHorizontal,
                    muted,
                ));
                c = c.push(form_input_row("Origin", "Upper Left", muted, input_bg, input_bdr));
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
                seg_btn("All", false, PanelMsg::PropertiesTab(1), input_bg, primary, muted, seg_hover, input_bdr),
                seg_btn("Parameters", true, PanelMsg::PropertiesTab(1), input_bg, primary, muted, seg_hover, input_bdr),
                seg_btn("Rules", false, PanelMsg::PropertiesTab(1), input_bg, primary, muted, seg_hover, input_bdr),
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
                .width(LABEL_W)
                .wrapping(iced::widget::text::Wrapping::None),
            container(
                text(value.to_string())
                    .size(11)
                    .color(Color::WHITE)
                    .wrapping(iced::widget::text::Wrapping::None),
            )
            .padding([3, 6])
            .width(Length::Fill)
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
    .padding([2, 8])
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
                .width(LABEL_W)
                .wrapping(iced::widget::text::Wrapping::None),
            control,
        ]
        .spacing(8.0)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, 8])
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
                .width(LABEL_W)
                .wrapping(iced::widget::text::Wrapping::None),
            iced::widget::checkbox(checked)
                .on_toggle(move |_| msg.clone())
                .size(14)
                .spacing(4),
            text(if checked { "On" } else { "Off" })
                .size(11)
                .color(if checked { Color::WHITE } else { label_c }),
        ]
        .spacing(8.0)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, 8])
    .width(Length::Fill)
    .into()
}

/// Form row: label | pick_list for grid size presets (2.54 mm multiples).
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
            iced::widget::pick_list(
                GRID_SIZE_LABELS,
                selected,
                |lbl: &'static str| {
                    // Map label back to mm value
                    let mm = GRID_SIZES_MM
                        .iter()
                        .zip(GRID_SIZE_LABELS.iter())
                        .find(|(_, l)| **l == lbl)
                        .map(|(v, _)| *v)
                        .unwrap_or(2.54);
                    PanelMsg::SetGridSize(mm)
                },
            )
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

    let pick = iced::widget::pick_list(
        labels,
        selected,
        move |lbl: &'static str| {
            // Map label back to mm value (labels and GRID_SIZES_MM are parallel arrays)
            let mm = GRID_SIZE_LABELS
                .iter()
                .chain(GRID_SIZE_LABELS_MIL.iter())
                .zip(GRID_SIZES_MM.iter().chain(GRID_SIZES_MM.iter()))
                .find(|(l, _)| **l == lbl)
                .map(|(_, v)| *v)
                .unwrap_or(2.54);
            on_size(mm)
        },
    )
    .text_size(11)
    .width(Length::Fill);

    let label_widget = text(label.to_string())
        .size(11)
        .color(label_c)
        .width(LABEL_W)
        .wrapping(iced::widget::text::Wrapping::None);

    let content: Element<PanelMsg> = if has_checkbox {
        // Snap Grid row: checkbox before pick_list
        row![
            label_widget,
            iced::widget::checkbox(active)
                .on_toggle(move |_| on_toggle.clone())
                .size(12)
                .spacing(4),
            pick,
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center)
        .into()
    } else {
        // Visible Grid row: no checkbox
        row![label_widget, pick]
            .spacing(4)
            .align_y(iced::Alignment::Center)
            .into()
    };

    container(content)
        .padding([2, 8])
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
                .width(LABEL_W)
                .wrapping(iced::widget::text::Wrapping::None),
            iced::widget::checkbox(value)
                .on_toggle(move |_| on_toggle.clone())
                .size(12)
                .spacing(4),
            Space::new().width(Length::Fill),
            text(shortcut_owned)
                .size(10)
                .color(label_c),
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, 8])
    .width(Length::Fill)
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
                .width(LABEL_W)
                .wrapping(iced::widget::text::Wrapping::None),
            iced::widget::button(
                text(summary)
                    .size(11)
                    .color(Color::from_rgb(0.35, 0.7, 1.0))
                    .width(Length::Fill),
            )
            .on_press(PanelMsg::OpenCanvasFontPopup)
            .padding([1, 0])
            .width(Length::Fill)
            .style(move |_: &Theme, status: iced::widget::button::Status| {
                let underline = match status {
                    iced::widget::button::Status::Hovered => true,
                    _ => false,
                };
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
    .padding([2, 8])
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

    let size_input = NumberInput::new(
        &current_size_px,
        6.0..=36.0,
        PanelMsg::SetCanvasFontSize,
    )
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
                text("Family")
                    .size(10)
                    .color(label_c)
                    .width(56),
                family_pick,
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
            row![
                text("Size")
                    .size(10)
                    .color(label_c)
                    .width(56),
                size_input,
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
            row![
                text("Style")
                    .size(10)
                    .color(label_c)
                    .width(56),
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
                .width(LABEL_W)
                .wrapping(iced::widget::text::Wrapping::None),
            NumberInput::new(&value, bounds, on_change)
                .step(step)
                .width(Length::Fill)
                .padding(4),
        ]
        .spacing(8.0)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, 8])
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
                .width(LABEL_W)
                .wrapping(iced::widget::text::Wrapping::None),
            iced::widget::text_input("", value)
                .on_input(on_input)
                .size(11)
                .padding(4)
                .width(Length::Fill),
        ]
        .spacing(8.0)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, 8])
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

/// Selection filter tag button (Altium blue pill).
fn tag_btn(label: &str, bg: Color, hover_bg: Color) -> Element<'static, PanelMsg> {
    iced::widget::button(
        text(label.to_string())
            .size(10)
            .color(Color::WHITE)
            .align_x(iced::alignment::Horizontal::Center),
    )
    .padding([3, 8])
    .style(move |_: &Theme, status: iced::widget::button::Status| {
        let hovered = matches!(status, iced::widget::button::Status::Hovered);
        iced::widget::button::Style {
            background: Some(Background::Color(if hovered { hover_bg } else { bg })),
            border: Border {
                radius: 3.0.into(),
                ..Border::default()
            },
            ..iced::widget::button::Style::default()
        }
    })
    .into()
}

/// Segmented button (for units toggle etc).
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
    let bg = if active { active_bg } else { Color::TRANSPARENT };
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
    col = col.push(section_title("Messages", &ctx.tokens));
    col = col.push(separator(&ctx.tokens));

    if ctx.has_schematic {
        col = col.push(
            text("No violations")
                .size(10)
                .color(theme_ext::success_color(&ctx.tokens)),
        );
        col = col.push(
            text("Run ERC to check")
                .size(9)
                .color(theme_ext::text_secondary(&ctx.tokens)),
        );
    } else {
        col = col.push(
            text("Open a project first")
                .size(10)
                .color(theme_ext::text_secondary(&ctx.tokens)),
        );
    }

    container(col).width(Length::Fill).into()
}
