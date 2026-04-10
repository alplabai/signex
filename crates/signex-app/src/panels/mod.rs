//! Panel implementations — each panel is a function returning an Element.

use iced::widget::{column, container, scrollable, text};
use iced::Element;

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
/// Uses owned strings to avoid lifetime issues with the view tree.
pub struct PanelContext {
    pub project_name: Option<String>,
    pub sym_count: usize,
    pub wire_count: usize,
    pub label_count: usize,
    pub child_sheets: Vec<String>,
    pub has_schematic: bool,
}

/// Render a panel's content with app context.
pub fn view_panel<'a, M: 'a>(kind: PanelKind, ctx: &'a PanelContext) -> Element<'a, M> {
    let content = match kind {
        PanelKind::Projects => view_projects(ctx),
        PanelKind::Components => view_stub("Component browser\n226 KiCad libraries"),
        PanelKind::Navigator => view_stub("Navigator"),
        PanelKind::Properties => view_stub("Properties (F11)\nSelect an object to see\nits properties."),
        PanelKind::Filter => view_stub("Selection filter"),
        PanelKind::Messages => view_messages(ctx),
        PanelKind::Signal => view_stub("Signal AI (Pro)\nAI-assisted design review."),
        PanelKind::Drc => view_stub("DRC violations"),
        PanelKind::LayerStack => view_stub("Layer Stack Manager"),
        PanelKind::NetClasses => view_stub("Net Classes"),
        PanelKind::Variants => view_stub("Design Variants"),
        PanelKind::OutputJobs => view_stub("Output Jobs"),
    };

    scrollable(content).into()
}

fn view_stub<'a, M: 'a>(description: &'a str) -> Element<'a, M> {
    container(
        column![text(description).size(12)]
            .spacing(4)
            .padding(4),
    )
    .into()
}

fn view_projects<'a, M: 'a>(ctx: &'a PanelContext) -> Element<'a, M> {
    let mut col = column![].spacing(2).padding(4);

    if let Some(name) = &ctx.project_name {
        col = col.push(text(format!("v {name}")).size(13));
        col = col.push(text(format!("  {} symbols", ctx.sym_count)).size(11));
        col = col.push(text(format!("  {} wires", ctx.wire_count)).size(11));
        col = col.push(text(format!("  {} labels", ctx.label_count)).size(11));

        if !ctx.child_sheets.is_empty() {
            col = col.push(text("").size(4));
            col = col.push(text("Child sheets:").size(12));
            for cs in &ctx.child_sheets {
                col = col.push(text(format!("  > {cs}")).size(11));
            }
        }
    } else {
        col = col.push(text("No project open").size(12));
        col = col.push(text("").size(4));
        col = col.push(text("File > Open Project").size(11));
    }

    container(col).into()
}

fn view_messages<'a, M: 'a>(ctx: &'a PanelContext) -> Element<'a, M> {
    let mut col = column![].spacing(2).padding(4);

    if ctx.has_schematic {
        col = col.push(text("No violations.").size(12));
        col = col.push(text("Run ERC to check (v0.7.0)").size(11));
    } else {
        col = col.push(text("Open a project first.").size(12));
    }

    container(col).into()
}
