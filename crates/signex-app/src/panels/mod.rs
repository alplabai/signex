//! Panel implementations — each panel is a function returning an Element.
//!
//! Panels are stubs in v0.1.0, populated with real content in later phases.

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

/// Render a panel's content. Stubs for v0.1.0.
pub fn view_panel<'a, M: 'a>(kind: PanelKind) -> Element<'a, M> {
    let content = match kind {
        PanelKind::Projects => view_projects_stub(),
        PanelKind::Components => view_stub("Component browser — 226 KiCad libraries\nSearch components here."),
        PanelKind::Navigator => view_stub("Navigator"),
        PanelKind::Properties => view_stub("Properties (F11)\nSelect an object to see its properties."),
        PanelKind::Filter => view_stub("Selection filter"),
        PanelKind::Messages => view_stub("Messages — ERC violations will appear here."),
        PanelKind::Signal => view_stub("Signal AI — Pro feature\nAI-assisted design review and chat."),
        PanelKind::Drc => view_stub("DRC — Design rule check violations."),
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

fn view_projects_stub<'a, M: 'a>() -> Element<'a, M> {
    container(
        column![
            text("Projects").size(13),
            text("  No project open").size(12),
            text("").size(8),
            text("  File > Open Project to begin").size(11),
        ]
        .spacing(4)
        .padding(4),
    )
    .into()
}
