use iced_dock::{DockSession, LayoutTree, PaneTarget, horizontal, panel, tabs};

use super::GerberDocumentId;

const DOCUMENTS_PANE: &str = "gerber-documents";
const TOOLS_PANE: &str = "gerber-tools";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GerberDockPanel {
    Document(GerberDocumentId),
    Layers,
    Highlight,
    Grid,
    LayerInformation,
    DCodes,
    Source,
}

impl GerberDockPanel {
    pub const TOOL_PANELS: [Self; 6] = [
        Self::Layers,
        Self::Highlight,
        Self::Grid,
        Self::LayerInformation,
        Self::DCodes,
        Self::Source,
    ];

    pub fn id(self) -> String {
        match self {
            Self::Document(id) => format!("document-{}", id.value()),
            Self::Layers => "layers".to_owned(),
            Self::Highlight => "highlight".to_owned(),
            Self::Grid => "grid".to_owned(),
            Self::LayerInformation => "layer-information".to_owned(),
            Self::DCodes => "d-codes".to_owned(),
            Self::Source => "source".to_owned(),
        }
    }

    pub const fn title(self) -> &'static str {
        match self {
            Self::Document(_) => "Gerber",
            Self::Layers => "Layers",
            Self::Highlight => "Highlight",
            Self::Grid => "Grid",
            Self::LayerInformation => "Layer Information",
            Self::DCodes => "D-Codes",
            Self::Source => "Original Source",
        }
    }

    pub const fn is_document(self) -> bool {
        matches!(self, Self::Document(_))
    }
}

pub struct GerberDockState {
    session: DockSession<GerberDockPanel>,
}

impl std::fmt::Debug for GerberDockState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GerberDockState")
            .field("panels", &self.session.panel_ids())
            .finish()
    }
}

impl GerberDockState {
    pub fn new(document_id: GerberDocumentId, title: &str) -> Self {
        Self {
            session: DockSession::from_tree(default_layout(document_id, title))
                .expect("the static Gerber dock layout must be valid"),
        }
    }

    pub fn session(&self) -> &DockSession<GerberDockPanel> {
        &self.session
    }

    pub fn add_document(&self, document_id: GerberDocumentId, title: &str) {
        let _ = self.session.open_panel(
            PaneTarget::Named(DOCUMENTS_PANE.to_owned()),
            document_panel(document_id, title),
        );
    }

    pub fn select_document(&self, document_id: GerberDocumentId) {
        let _ = self
            .session
            .select_panel(&GerberDockPanel::Document(document_id).id());
    }

    pub fn rename_document(&self, document_id: GerberDocumentId, title: &str) {
        let panel_id = GerberDockPanel::Document(document_id).id();
        let _ = self.session.close_panel(&panel_id);
        self.add_document(document_id, title);
    }

    pub fn show_tool(&self, panel_kind: GerberDockPanel) {
        if self.session.select_panel(&panel_kind.id()).is_ok() {
            return;
        }

        let _ = self.session.open_panel(
            PaneTarget::Named(TOOLS_PANE.to_owned()),
            tool_panel(panel_kind),
        );
    }

    pub fn close_tool(&self, panel_kind: GerberDockPanel) {
        if !panel_kind.is_document() {
            let _ = self.session.close_panel(&panel_kind.id());
        }
    }

    pub fn is_open(&self, panel_kind: GerberDockPanel) -> bool {
        self.session.panel_ids().contains(&panel_kind.id())
    }

    #[cfg(test)]
    pub fn panel_ids(&self) -> Vec<String> {
        self.session.panel_ids()
    }
}

fn default_layout(document_id: GerberDocumentId, title: &str) -> LayoutTree<GerberDockPanel> {
    horizontal([
        tabs([document_panel(document_id, title)])
            .named(DOCUMENTS_PANE)
            .persistent(true),
        tabs(GerberDockPanel::TOOL_PANELS.map(tool_panel))
            .active(GerberDockPanel::Layers.id())
            .named(TOOLS_PANE)
            .persistent(true),
    ])
    .weights([0.76, 0.24])
}

fn document_panel(
    document_id: GerberDocumentId,
    title: &str,
) -> iced_dock::PanelDef<GerberDockPanel> {
    let panel_kind = GerberDockPanel::Document(document_id);
    panel(panel_kind.id(), title, panel_kind)
}

fn tool_panel(panel_kind: GerberDockPanel) -> iced_dock::PanelDef<GerberDockPanel> {
    panel(panel_kind.id(), panel_kind.title(), panel_kind)
}
