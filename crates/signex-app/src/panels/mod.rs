//! Panel implementations — Altium-style panel content.

use iced::widget::{column, container, row, scrollable, text, Column};
use iced::{Element, Length};

use crate::styles;

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

/// Context passed to panels so they can display live state.
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
}

/// Render a panel's content with app context.
pub fn view_panel<'a, M: 'a>(kind: PanelKind, ctx: &'a PanelContext) -> Element<'a, M> {
    let content = match kind {
        PanelKind::Projects => view_projects(ctx),
        PanelKind::Components => view_components(ctx),
        PanelKind::Navigator => view_navigator(ctx),
        PanelKind::Properties => view_properties(ctx),
        PanelKind::Filter => view_filter(),
        PanelKind::Messages => view_messages(ctx),
        PanelKind::Signal => view_signal(),
        PanelKind::Drc => view_drc(),
        PanelKind::LayerStack => view_stub("Layer Stack Manager", "PCB mode only"),
        PanelKind::NetClasses => view_stub("Net Classes", "Define net classes and rules"),
        PanelKind::Variants => view_stub("Variants", "Design variant management"),
        PanelKind::OutputJobs => view_stub("Output Jobs", "Manufacturing output configuration"),
    };

    scrollable(content).width(Length::Fill).into()
}

// ─── Helper ───────────────────────────────────────────────────

fn lbl<'a>(s: impl ToString) -> iced::widget::Text<'a> {
    text(s.to_string()).size(11).color(styles::TEXT_PRIMARY)
}

fn dim<'a>(s: impl ToString) -> iced::widget::Text<'a> {
    text(s.to_string()).size(10).color(styles::TEXT_MUTED)
}

fn tree_item<'a, M: 'a>(indent: u16, icon: impl ToString, label: impl ToString) -> Element<'a, M> {
    row![
        text("").width(indent as u32),
        text(icon.to_string()).size(11).color(styles::TEXT_MUTED),
        text(format!(" {}", label.to_string())).size(11).color(styles::TEXT_PRIMARY),
    ]
    .align_y(iced::Alignment::Center)
    .into()
}

fn tree_header<'a, M: 'a>(icon: impl ToString, label: impl ToString) -> Element<'a, M> {
    row![
        text(icon.to_string()).size(11).color(styles::TEXT_MUTED),
        text(format!(" {}", label.to_string())).size(11).color(iced::Color::WHITE),
    ]
    .align_y(iced::Alignment::Center)
    .into()
}

fn section_header<'a>(title: impl ToString) -> iced::widget::Text<'a> {
    text(title.to_string()).size(10).color(styles::TEXT_MUTED)
}

fn view_stub<'a, M: 'a>(title: &'a str, desc: &'a str) -> Element<'a, M> {
    container(
        column![dim(title), dim(desc)]
            .spacing(4)
            .padding(6),
    )
    .width(Length::Fill)
    .into()
}

// ─── Projects Panel ───────────────────────────────────────────

fn view_projects<'a, M: 'a>(ctx: &'a PanelContext) -> Element<'a, M> {
    let mut col: Column<'a, M> = Column::new().spacing(1).padding(4).width(Length::Fill);

    if let Some(name) = &ctx.project_name {
        // Project root
        col = col.push(tree_header("v", name));

        // Schematic files
        if let Some(file) = &ctx.project_file {
            col = col.push(tree_item(12, ">", file));
        }

        // Child sheets
        for cs in &ctx.child_sheets {
            col = col.push(tree_item(24, " ", cs));
        }

        col = col.push(text("───────────────────────").size(6).color(styles::BORDER_SUBTLE));

        // Stats summary
        col = col.push(dim(format!(
            "  {} sym  {} wires  {} labels",
            ctx.sym_count, ctx.wire_count, ctx.label_count
        )));
    } else {
        col = col.push(dim("No project open"));
        col = col.push(text("").size(6));
        col = col.push(dim("File > Open to begin"));
    }

    container(col).width(Length::Fill).into()
}

// ─── Components Panel ─────────────────────────────────────────

fn view_components<'a, M: 'a>(ctx: &'a PanelContext) -> Element<'a, M> {
    let mut col: Column<'a, M> = Column::new().spacing(2).padding(4).width(Length::Fill);

    col = col.push(section_header("LIBRARIES"));
    col = col.push(text("───────────────────────").size(6).color(styles::BORDER_SUBTLE));

    if ctx.has_schematic {
        col = col.push(tree_item(0, "v", "Project Libraries"));
        col = col.push(tree_item(
            12,
            " ",
            format!("{} unique symbols", ctx.lib_symbol_count),
        ));
    } else {
        col = col.push(dim("Open a project to browse components"));
    }

    container(col).width(Length::Fill).into()
}

// ─── Navigator Panel ──────────────────────────────────────────

fn view_navigator<'a, M: 'a>(ctx: &'a PanelContext) -> Element<'a, M> {
    let mut col: Column<'a, M> = Column::new().spacing(2).padding(4).width(Length::Fill);

    col = col.push(section_header("SHEETS"));
    col = col.push(text("───────────────────────").size(6).color(styles::BORDER_SUBTLE));

    if let Some(name) = &ctx.project_name {
        col = col.push(tree_item(0, ">", name));
        for cs in &ctx.child_sheets {
            col = col.push(tree_item(12, " ", cs));
        }
    } else {
        col = col.push(dim("No project open"));
    }

    container(col).width(Length::Fill).into()
}

// ─── Properties Panel ─────────────────────────────────────────

fn view_properties<'a, M: 'a>(ctx: &'a PanelContext) -> Element<'a, M> {
    let mut col: Column<'a, M> = Column::new().spacing(2).padding(4).width(Length::Fill);

    if ctx.has_schematic {
        col = col.push(section_header("DOCUMENT"));
        col = col.push(text("───────────────────────").size(6).color(styles::BORDER_SUBTLE));

        if let Some(name) = &ctx.project_name {
            col = col.push(prop_row("File", name));
        }
        col = col.push(prop_row("Paper", &ctx.paper_size));
        col = col.push(prop_row("Symbols", ctx.sym_count));
        col = col.push(prop_row("Wires", ctx.wire_count));
        col = col.push(prop_row("Junctions", ctx.junction_count));
        col = col.push(prop_row("Labels", ctx.label_count));
        col = col.push(prop_row("Sheets", ctx.child_sheets.len()));

        col = col.push(text("").size(8));
        col = col.push(section_header("SELECTION"));
        col = col.push(text("───────────────────────").size(6).color(styles::BORDER_SUBTLE));
        col = col.push(dim("No selection"));
    } else {
        col = col.push(dim("Open a project to see properties"));
    }

    container(col).width(Length::Fill).into()
}

fn prop_row<'a, M: 'a>(key: impl ToString, value: impl ToString) -> Element<'a, M> {
    row![
        text(key.to_string()).size(10).color(styles::TEXT_MUTED).width(70),
        text(value.to_string()).size(10).color(styles::TEXT_PRIMARY),
    ]
    .spacing(4)
    .into()
}

// ─── Filter Panel ─────────────────────────────────────────────

fn view_filter<'a, M: 'a>() -> Element<'a, M> {
    let mut col: Column<'a, M> = Column::new().spacing(2).padding(4).width(Length::Fill);

    col = col.push(section_header("SELECTION FILTER"));
    col = col.push(text("───────────────────────").size(6).color(styles::BORDER_SUBTLE));
    col = col.push(dim("All object types enabled"));

    container(col).width(Length::Fill).into()
}

// ─── Messages Panel ───────────────────────────────────────────

fn view_messages<'a, M: 'a>(ctx: &'a PanelContext) -> Element<'a, M> {
    let mut col: Column<'a, M> = Column::new().spacing(1).padding(4).width(Length::Fill);

    col = col.push(section_header("MESSAGES"));
    col = col.push(text("───────────────────────").size(6).color(styles::BORDER_SUBTLE));

    if ctx.has_schematic {
        col = col.push(dim("No violations. Run ERC to check."));
    } else {
        col = col.push(dim("Open a project first."));
    }

    container(col).width(Length::Fill).into()
}

// ─── Signal AI Panel ──────────────────────────────────────────

fn view_signal<'a, M: 'a>() -> Element<'a, M> {
    let mut col: Column<'a, M> = Column::new().spacing(2).padding(4).width(Length::Fill);

    col = col.push(section_header("SIGNAL AI"));
    col = col.push(text("───────────────────────").size(6).color(styles::BORDER_SUBTLE));
    col = col.push(dim("Pro feature"));
    col = col.push(dim("AI-assisted design review"));

    container(col).width(Length::Fill).into()
}

// ─── DRC Panel ────────────────────────────────────────────────

fn view_drc<'a, M: 'a>() -> Element<'a, M> {
    let mut col: Column<'a, M> = Column::new().spacing(1).padding(4).width(Length::Fill);

    col = col.push(section_header("DESIGN RULE CHECK"));
    col = col.push(text("───────────────────────").size(6).color(styles::BORDER_SUBTLE));
    col = col.push(dim("Run DRC to check PCB design rules."));

    container(col).width(Length::Fill).into()
}
