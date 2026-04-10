//! Panel implementations — uses signex-widgets for proper Altium-style content.

use iced::widget::{column, container, row, scrollable, text, Column};
use iced::{Element, Length};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;
use signex_widgets::tree_view::{self, TreeIcon, TreeMsg, TreeNode};

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

/// Context passed to panels — owned data to avoid lifetime issues.
pub struct PanelContext {
    pub project_name: Option<String>,
    pub project_file: Option<String>,
    pub sym_count: usize,
    pub wire_count: usize,
    pub label_count: usize,
    pub junction_count: usize,
    pub child_sheets: Vec<String>,
    pub has_schematic: bool,
    pub paper_size: String,
    pub lib_symbol_count: usize,
    pub tokens: ThemeTokens,
}

/// Panel-level message wrapping widget messages.
#[derive(Debug, Clone)]
pub enum PanelMsg {
    Tree(TreeMsg),
}

/// Render a panel's content.
pub fn view_panel<'a>(kind: PanelKind, ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let content = match kind {
        PanelKind::Projects => view_projects(ctx),
        PanelKind::Components => view_components(ctx),
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

fn view_projects<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    if let Some(name) = &ctx.project_name {
        // Build tree data
        let mut sheets = vec![];
        if let Some(file) = &ctx.project_file {
            sheets.push(TreeNode::leaf(file.clone(), TreeIcon::Schematic));
        }
        for cs in &ctx.child_sheets {
            sheets.push(TreeNode::leaf(cs.clone(), TreeIcon::Sheet));
        }

        let roots = vec![TreeNode::branch(name.clone(), TreeIcon::Folder, sheets)];

        let tree = tree_view::tree_view(&roots, None, &ctx.tokens);
        let stats = text(format!(
            "{} sym  {} wires  {} labels",
            ctx.sym_count, ctx.wire_count, ctx.label_count
        ))
        .size(9)
        .color(theme_ext::text_secondary(&ctx.tokens));

        column![
            tree.map(PanelMsg::Tree),
            separator(&ctx.tokens),
            container(stats).padding([2, 4]),
        ]
        .spacing(2)
        .width(Length::Fill)
        .into()
    } else {
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
    }
}

// ─── Components Panel ─────────────────────────────────────────

fn view_components<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let mut col: Column<'a, PanelMsg> = Column::new().spacing(2).padding(6).width(Length::Fill);
    col = col.push(section_title("Libraries", &ctx.tokens));
    col = col.push(separator(&ctx.tokens));

    if ctx.has_schematic {
        let roots = vec![TreeNode::branch(
            "Project Libraries".to_string(),
            TreeIcon::Library,
            vec![TreeNode::leaf(
                format!("{} symbols", ctx.lib_symbol_count),
                TreeIcon::Component,
            )],
        )];
        col = col.push(tree_view::tree_view(&roots, None, &ctx.tokens).map(PanelMsg::Tree));
    } else {
        col = col.push(
            text("Open a project to browse")
                .size(10)
                .color(theme_ext::text_secondary(&ctx.tokens)),
        );
    }
    container(col).width(Length::Fill).into()
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
        col = col.push(tree_view::tree_view(&roots, None, &ctx.tokens).map(PanelMsg::Tree));
    } else {
        col = col.push(text("No project").size(10).color(theme_ext::text_secondary(&ctx.tokens)));
    }
    container(col).width(Length::Fill).into()
}

// ─── Properties Panel ─────────────────────────────────────────

fn view_properties<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let mut col: Column<'a, PanelMsg> = Column::new().spacing(2).padding(6).width(Length::Fill);

    if ctx.has_schematic {
        col = col.push(section_title("Document", &ctx.tokens));
        col = col.push(separator(&ctx.tokens));

        if let Some(name) = &ctx.project_name {
            col = col.push(prop_row("File", name, &ctx.tokens));
        }
        col = col.push(prop_row("Paper", &ctx.paper_size, &ctx.tokens));
        col = col.push(prop_row("Symbols", ctx.sym_count, &ctx.tokens));
        col = col.push(prop_row("Wires", ctx.wire_count, &ctx.tokens));
        col = col.push(prop_row("Junctions", ctx.junction_count, &ctx.tokens));
        col = col.push(prop_row("Labels", ctx.label_count, &ctx.tokens));
        col = col.push(prop_row("Sheets", ctx.child_sheets.len(), &ctx.tokens));

        col = col.push(text("").size(6));
        col = col.push(section_title("Selection", &ctx.tokens));
        col = col.push(separator(&ctx.tokens));
        col = col.push(text("No selection").size(10).color(theme_ext::text_secondary(&ctx.tokens)));
    } else {
        col = col.push(text("Open a project").size(10).color(theme_ext::text_secondary(&ctx.tokens)));
    }

    container(col).width(Length::Fill).into()
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
