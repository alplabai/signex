//! Panel implementations — uses signex-widgets for proper Altium-style content.

use iced::widget::{column, container, row, scrollable, text, Column, Row, Space};
use iced::{Background, Border, Color, Element, Length, Theme};
use signex_types::coord::Unit;
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;
use signex_widgets::tree_view::{TreeIcon, TreeMsg, TreeNode, TreeView};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
}

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
            PanelKind::OutputJobs => "Output Jobs",
        }
    }
}

/// Per-sheet info for the project tree.
#[derive(Debug, Clone)]
pub struct SheetInfo {
    pub name: String,
    pub filename: String,
    pub sym_count: usize,
    pub wire_count: usize,
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
    /// Description of the selected item (for single selection).
    pub selection_info: Vec<(String, String)>,
    /// Component search filter text.
    pub component_filter: String,
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
}

/// Render a panel's content.
pub fn view_panel<'a>(kind: PanelKind, ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    // Components has its own split scrollables — don't wrap again
    if kind == PanelKind::Components {
        return view_components(ctx);
    }

    let content = match kind {
        PanelKind::Components => unreachable!(),
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
    };

    scrollable(content).width(Length::Fill).into()
}

// ─── Helpers ──────────────────────────────────────────────────

fn section_title<'a>(title: &str, tokens: &ThemeTokens) -> iced::widget::Text<'a> {
    text(title.to_uppercase())
        .size(9)
        .color(theme_ext::text_secondary(tokens))
}

fn prop_row<'a>(key: impl ToString, value: impl ToString, tokens: &ThemeTokens) -> Element<'a, PanelMsg> {
    row![
        text(key.to_string()).size(10).color(theme_ext::text_secondary(tokens)).width(70),
        text(value.to_string()).size(10).color(theme_ext::text_primary(tokens)),
    ]
    .spacing(4)
    .into()
}

fn separator<'a>(tokens: &ThemeTokens) -> iced::widget::Text<'a> {
    text("─".repeat(30)).size(4).color(theme_ext::border_color(tokens))
}

fn view_stub<'a>(title: &str, desc: &str, ctx: &PanelContext) -> Element<'a, PanelMsg> {
    container(
        column![
            section_title(title, &ctx.tokens),
            separator(&ctx.tokens),
            text(desc.to_string()).size(10).color(theme_ext::text_secondary(&ctx.tokens)),
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
                TreeNode::leaf(sheet.filename.clone(), TreeIcon::Schematic)
                    .with_badge(badge),
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
        TreeNode::leaf(
            format!("{} symbols loaded", lib_count),
            TreeIcon::Component,
        )
        .with_badge(lib_count.to_string()),
    ];

    let mut settings = TreeNode::branch("Settings".to_string(), TreeIcon::File, vec![]);
    settings.expanded = false;

    vec![TreeNode::branch(
        name.clone(),
        TreeIcon::Folder,
        vec![
            TreeNode::branch("Source Documents".to_string(), TreeIcon::Folder, source_docs),
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
                text("Name").size(10).color(muted).width(Length::FillPortion(4)),
                text("Pins").size(10).color(muted).width(Length::FillPortion(1)),
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
        ctx.library_symbols.iter().filter(|(name, _)| name.to_ascii_lowercase().contains(&filter)).collect()
    };

    if filtered_symbols.is_empty() {
        let msg = if ctx.active_library.is_some() {
            if filter.is_empty() { "Loading..." } else { "No matches" }
        } else {
            "Select a library above"
        };
        list_col = list_col.push(
            container(text(msg).size(10).color(muted)).padding([8, 8]),
        );
    } else {
        let sel = &ctx.selected_component;
        for &(name, pins) in &filtered_symbols {
            let is_sel = sel.as_deref() == Some(name.as_str());
            let row_bg = if is_sel { theme_ext::selection_color(&ctx.tokens) } else { Color::TRANSPARENT };
            let name_c = if is_sel { Color::WHITE } else { primary };
            let n = name.clone();
            list_col = list_col.push(column![
                iced::widget::button(
                    row![
                        text(name.clone()).size(10).color(name_c)
                            .width(Length::FillPortion(4))
                            .wrapping(iced::widget::text::Wrapping::None),
                        text(pins.to_string()).size(10).color(muted)
                            .width(Length::FillPortion(1)),
                    ].spacing(4.0),
                )
                .padding([3, 8]).width(Length::Fill)
                .on_press(PanelMsg::SelectComponent(n))
                .style(move |_: &Theme, status: iced::widget::button::Status| {
                    let bg = match (is_sel, status) {
                        (true, _) => Some(Background::Color(row_bg)),
                        (false, iced::widget::button::Status::Hovered) =>
                            Some(Background::Color(Color::from_rgb(0.20, 0.20, 0.23))),
                        _ => None,
                    };
                    iced::widget::button::Style { background: bg, border: Border::default(), ..iced::widget::button::Style::default() }
                }),
                thin_sep(border_c),
            ].spacing(0));
        }
    }

    list_col = list_col.push(
        container(text(format!("Results: {}", filtered_symbols.len())).size(10).color(muted))
            .padding([4, 8]),
    );

    // ── BOTTOM: Details panel (scrollable) ──
    let mut detail_col: Column<'a, PanelMsg> = Column::new().spacing(0).width(Length::Fill);

    if let Some(comp_name) = &ctx.selected_component {
        detail_col = detail_col.push(section_hdr(
            &format!("\u{25BC} Details  {comp_name}"), primary, border_c,
        ));
        let pin_count = ctx.library_symbols.iter()
            .find(|(n, _)| n == comp_name).map(|(_, p)| *p).unwrap_or(0);
        detail_col = detail_col.push(form_input_row("Symbol", comp_name, muted));
        detail_col = detail_col.push(form_input_row("Pins", &pin_count.to_string(), muted));
        detail_col = detail_col.push(form_input_row("Library", ctx.active_library.as_deref().unwrap_or(""), muted));

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
                        background: Some(Background::Color(Color::from_rgb(0.12, 0.12, 0.14))),
                        border: Border { width: 1.0, radius: 2.0.into(), color: border_c },
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
                        background: Some(Background::Color(Color::from_rgb(0.12, 0.12, 0.14))),
                        border: Border { width: 1.0, radius: 2.0.into(), color: border_c },
                        ..container::Style::default()
                    }),
                )
                .padding([4, 8]),
            );
        }

        for section in &["References", "Part Choices", "Where Used"] {
            detail_col = detail_col.push(Space::new().height(2.0));
            detail_col = detail_col.push(section_hdr(&format!("\u{25BC} {section}"), primary, border_c));
        }
    } else {
        detail_col = detail_col.push(
            container(text("Select a component").size(10).color(muted))
                .padding([12, 8]).width(Length::Fill),
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
        col = col.push(TreeView::new(&roots, &ctx.tokens).view().map(PanelMsg::Tree));
    } else {
        col = col.push(text("No project").size(10).color(theme_ext::text_secondary(&ctx.tokens)));
    }
    container(col).width(Length::Fill).into()
}

// ─── Properties Panel (matched to Altium Designer) ───────────

const LABEL_W: f32 = 90.0;
const INPUT_BG: Color = Color::from_rgb(0.16, 0.22, 0.32);
const TAG_BG: Color = Color::from_rgb(0.20, 0.35, 0.55);

fn view_properties<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let muted = theme_ext::text_secondary(&ctx.tokens);
    let primary = theme_ext::text_primary(&ctx.tokens);
    let border_c = theme_ext::border_color(&ctx.tokens);

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

    // ── Context-aware: if something is selected, show element properties (Altium style) ──
    if ctx.selection_count == 1 && !ctx.selection_info.is_empty() {
        return view_selected_element_properties(ctx, muted, primary, border_c);
    }
    if ctx.selection_count > 1 {
        let mut col: Column<'a, PanelMsg> = Column::new().spacing(0).width(Length::Fill);
        col = col.push(
            container(text("Multi-Selection").size(11).color(primary))
                .padding([6, 8]).width(Length::Fill),
        );
        col = col.push(thin_sep(border_c));
        col = col.push(
            container(text(format!("{} objects selected", ctx.selection_count)).size(10).color(muted))
                .padding([4, 8]),
        );
        for (key, value) in &ctx.selection_info {
            col = col.push(
                container(row![
                    text(key).size(10).color(muted).width(Length::FillPortion(2)),
                    text(value).size(10).color(primary).width(Length::FillPortion(3)),
                ].spacing(4)).padding([3, 8]).width(Length::Fill),
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
    col = col.push(
        container(
            row![
                props_tab_btn("General", tab == 0, PanelMsg::PropertiesTab(0)),
                props_tab_btn("Parameters", tab == 1, PanelMsg::PropertiesTab(1)),
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
        col = col.push(view_properties_parameters(muted, primary, border_c));
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
fn view_selected_element_properties<'a>(
    ctx: &'a PanelContext,
    muted: Color,
    primary: Color,
    border_c: Color,
) -> Element<'a, PanelMsg> {
    let mut col: Column<'a, PanelMsg> = Column::new().spacing(0).width(Length::Fill);

    // Determine element type from selection_info
    let elem_type = ctx.selection_info.iter()
        .find(|(k, _)| k == "Type")
        .map(|(_, v)| v.as_str())
        .unwrap_or("Object");

    // ── Header: element type (like Altium's "Component" header) ──
    col = col.push(
        container(text(elem_type.to_owned()).size(11).color(primary))
            .padding([6, 8]).width(Length::Fill),
    );
    col = col.push(thin_sep(border_c));

    // ── General section ──
    col = col.push(section_hdr("General", muted, border_c));

    for (key, value) in &ctx.selection_info {
        if key == "Type" { continue; } // already shown in header
        col = col.push(
            container(row![
                text(key).size(10).color(muted).width(100),
                text(value).size(10).color(primary),
            ].spacing(4)).padding([4, 8]).width(Length::Fill),
        );
    }

    // ── Location section (for elements that have position) ──
    let has_position = ctx.selection_info.iter().any(|(k, _)| k == "Position");
    if has_position {
        col = col.push(Space::new().height(4.0));
        col = col.push(section_hdr("Location", muted, border_c));
        for (key, value) in &ctx.selection_info {
            if key == "Position" || key == "Rotation" || key == "Start" || key == "End" || key == "Length" {
                col = col.push(
                    container(row![
                        text(key).size(10).color(muted).width(100),
                        text(value).size(10).color(primary),
                    ].spacing(4)).padding([4, 8]).width(Length::Fill),
                );
            }
        }
    }

    // ── Graphical section (for symbols) ──
    let has_mirror = ctx.selection_info.iter().any(|(k, _)| k == "Mirror");
    if has_mirror || elem_type == "Symbol" {
        col = col.push(Space::new().height(4.0));
        col = col.push(section_hdr("Graphical", muted, border_c));
        for (key, value) in &ctx.selection_info {
            if key == "Mirror" || key == "Unit" {
                col = col.push(
                    container(row![
                        text(key).size(10).color(muted).width(100),
                        text(value).size(10).color(primary),
                    ].spacing(4)).padding([4, 8]).width(Length::Fill),
                );
            }
        }
    }

    // ── Status bar ──
    col = col.push(Space::new().height(8.0));
    col = col.push(thin_sep(border_c));
    col = col.push(
        container(text("1 object selected").size(10).color(muted))
            .padding([4, 8]),
    );

    scrollable(col).width(Length::Fill).into()
}

fn view_properties_general<'a>(
    ctx: &'a PanelContext,
    muted: Color,
    primary: Color,
    border_c: Color,
) -> Column<'a, PanelMsg> {
    let mut col: Column<'a, PanelMsg> = Column::new().spacing(0).width(Length::Fill);

    // Selection Filter
    col = col.push(section_hdr("\u{25BC} Selection Filter", primary, border_c));
    col = col.push(
        container(
            column![
                row![tag_btn("Components"), tag_btn("Wires"), tag_btn("Buses"),].spacing(4.0),
                row![
                    tag_btn("Sheet Symbols"),
                    tag_btn("Sheet Entries"),
                    tag_btn("Net Labels"),
                ]
                .spacing(4.0),
                row![
                    tag_btn("Parameters"),
                    tag_btn("Ports"),
                    tag_btn("Power Ports"),
                    tag_btn("Texts"),
                ]
                .spacing(4.0),
                row![tag_btn("Drawing Objects"), tag_btn("Other"),].spacing(4.0),
            ]
            .spacing(4.0),
        )
        .padding([6, 8]),
    );

    // General
    col = col.push(section_hdr("\u{25BC} General", primary, border_c));
    col = col.push(form_label("Units", muted));
    col = col.push(
        container(
            row![
                seg_btn("mm", ctx.unit == Unit::Mm, PanelMsg::SetUnit(Unit::Mm)),
                seg_btn("mils", ctx.unit == Unit::Mil, PanelMsg::SetUnit(Unit::Mil)),
            ]
            .spacing(0.0)
            .width(Length::Fill),
        )
        .padding([2, 8]),
    );
    col = col.push(form_input_row("Visible Grid", &format!("{}mm", ctx.grid_size_mm), muted));
    col = col.push(form_check_row("Snap Grid", ctx.snap_enabled, PanelMsg::ToggleSnap, muted));
    col = col.push(form_check_row("Grid Visible", ctx.grid_visible, PanelMsg::ToggleGrid, muted));
    col = col.push(form_input_row("Document Font", "Arial, 8", muted));
    col = col.push(form_input_row("Sheet Color", "Black", muted));

    // Page Options
    col = col.push(Space::new().height(2.0));
    col = col.push(section_hdr("\u{25BC} Page Options", primary, border_c));
    col = col.push(form_label("Formatting and Size", muted));
    col = col.push(
        container(
            row![
                seg_btn("Template", true, PanelMsg::ToggleGrid),
                seg_btn("Standard", false, PanelMsg::ToggleGrid),
                seg_btn("Custom", false, PanelMsg::ToggleGrid),
            ]
            .spacing(0.0)
            .width(Length::Fill),
        )
        .padding([2, 8]),
    );
    col = col.push(form_input_row("Paper", &ctx.paper_size, muted));
    let dims = match ctx.paper_size.as_str() {
        "A4" => "Width: 297mm  Height: 210mm",
        "A3" => "Width: 420mm  Height: 297mm",
        _ => "Width: 297mm  Height: 210mm",
    };
    col = col.push(container(text(dims.to_string()).size(10).color(muted)).padding([3, 8]));
    col = col.push(form_label("Margin and Zones", muted));
    col = col.push(form_input_row("Vertical", "1", muted));
    col = col.push(form_input_row("Horizontal", "1", muted));
    col = col.push(form_input_row("Origin", "Upper Left", muted));

    col
}

fn view_properties_parameters<'a>(
    muted: Color,
    primary: Color,
    border_c: Color,
) -> Column<'a, PanelMsg> {
    let mut col: Column<'a, PanelMsg> = Column::new().spacing(0).width(Length::Fill);

    col = col.push(section_hdr("\u{25BC} Parameters", primary, border_c));

    // Sub-tabs: All | Parameters | Rules
    col = col.push(
        container(
            row![
                seg_btn("All", false, PanelMsg::PropertiesTab(1)),
                seg_btn("Parameters", true, PanelMsg::PropertiesTab(1)),
                seg_btn("Rules", false, PanelMsg::PropertiesTab(1)),
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
                text("Name").size(10).color(muted).width(Length::FillPortion(3)),
                text("Value").size(10).color(muted).width(Length::FillPortion(2)),
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
fn props_tab_btn(label: &str, active: bool, msg: PanelMsg) -> Element<'static, PanelMsg> {
    let text_c = if active {
        Color::WHITE
    } else {
        Color::from_rgb(0.55, 0.55, 0.58)
    };
    iced::widget::button(text(label.to_string()).size(11).color(text_c))
        .padding([4, 12])
        .on_press(msg)
        .style(move |_: &Theme, status: iced::widget::button::Status| {
            let hover = matches!(status, iced::widget::button::Status::Hovered);
            iced::widget::button::Style {
                background: if active || hover {
                    Some(Background::Color(Color::from_rgb(0.22, 0.22, 0.25)))
                } else {
                    None
                },
                border: Border {
                    width: 1.0,
                    radius: 2.0.into(),
                    color: Color::from_rgb(0.30, 0.30, 0.33),
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
fn form_input_row<'a, M: 'a>(label: &str, value: &str, label_c: Color) -> Element<'a, M> {
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
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(INPUT_BG)),
                border: Border {
                    width: 1.0,
                    radius: 2.0.into(),
                    color: Color::from_rgb(0.22, 0.28, 0.38),
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
                .color(if checked {
                    Color::WHITE
                } else {
                    label_c
                }),
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
fn tag_btn(label: &str) -> Element<'static, PanelMsg> {
    iced::widget::button(
        text(label.to_string())
            .size(10)
            .color(Color::WHITE)
            .align_x(iced::alignment::Horizontal::Center),
    )
    .padding([3, 8])
    .style(|_: &Theme, status: iced::widget::button::Status| {
        let hover = matches!(status, iced::widget::button::Status::Hovered);
        iced::widget::button::Style {
            background: Some(Background::Color(if hover {
                Color::from_rgb(0.25, 0.42, 0.65)
            } else {
                TAG_BG
            })),
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
fn seg_btn<'a>(label: &str, active: bool, msg: PanelMsg) -> Element<'a, PanelMsg> {
    let bg = if active { INPUT_BG } else { Color::TRANSPARENT };
    let text_c = if active {
        Color::WHITE
    } else {
        Color::from_rgb(0.55, 0.55, 0.58)
    };
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
        let hover = matches!(status, iced::widget::button::Status::Hovered);
        iced::widget::button::Style {
            background: Some(Background::Color(if hover && !active {
                Color::from_rgb(0.20, 0.20, 0.23)
            } else {
                bg
            })),
            border: Border {
                width: 1.0,
                radius: 2.0.into(),
                color: Color::from_rgb(0.22, 0.28, 0.38),
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
        col = col.push(text("No violations").size(10).color(theme_ext::success_color(&ctx.tokens)));
        col = col.push(text("Run ERC to check").size(9).color(theme_ext::text_secondary(&ctx.tokens)));
    } else {
        col = col.push(text("Open a project first").size(10).color(theme_ext::text_secondary(&ctx.tokens)));
    }

    container(col).width(Length::Fill).into()
}
