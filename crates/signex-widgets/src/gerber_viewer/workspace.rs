use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GerberDocumentId(u64);

impl GerberDocumentId {
    pub const fn value(self) -> u64 {
        self.0
    }
}

#[derive(Debug)]
pub struct GerberDocumentState {
    pub id: GerberDocumentId,
    pub title: String,
    pub viewer: GerberViewerState,
}

#[derive(Debug)]
pub struct GerberWorkspaceState {
    pub(super) dock: super::dock::GerberDockState,
    documents: Vec<GerberDocumentState>,
    active_document: GerberDocumentId,
    next_document_id: u64,
}

impl Default for GerberWorkspaceState {
    fn default() -> Self {
        let id = GerberDocumentId(1);
        let title = "Gerber Viewer 1".to_owned();
        Self {
            dock: super::dock::GerberDockState::new(id, &title),
            documents: vec![GerberDocumentState {
                id,
                title,
                viewer: GerberViewerState::default(),
            }],
            active_document: id,
            next_document_id: 2,
        }
    }
}

impl GerberWorkspaceState {
    pub fn documents(&self) -> &[GerberDocumentState] {
        &self.documents
    }

    pub const fn active_document_id(&self) -> GerberDocumentId {
        self.active_document
    }

    pub fn active_viewer(&self) -> Option<&GerberViewerState> {
        self.viewer(self.active_document)
    }

    pub fn viewer(&self, document_id: GerberDocumentId) -> Option<&GerberViewerState> {
        self.documents
            .iter()
            .find(|document| document.id == document_id)
            .map(|document| &document.viewer)
    }

    pub fn viewer_mut(&mut self, document_id: GerberDocumentId) -> Option<&mut GerberViewerState> {
        self.documents
            .iter_mut()
            .find(|document| document.id == document_id)
            .map(|document| &mut document.viewer)
    }

    pub fn take_viewer(&mut self, document_id: GerberDocumentId) -> Option<GerberViewerState> {
        let document = self
            .documents
            .iter_mut()
            .find(|document| document.id == document_id)?;
        Some(std::mem::take(&mut document.viewer))
    }

    pub fn restore_viewer(&mut self, document_id: GerberDocumentId, viewer: GerberViewerState) {
        if let Some(document) = self
            .documents
            .iter_mut()
            .find(|document| document.id == document_id)
        {
            document.viewer = viewer;
        }
    }

    pub fn new_document(&mut self) -> GerberDocumentId {
        let id = GerberDocumentId(self.next_document_id);
        self.next_document_id += 1;
        let title = format!("Gerber Viewer {}", id.value());
        let mut viewer = GerberViewerState::default();
        for panel in GerberDockPanel::TOOL_PANELS {
            viewer.set_tool_panel_visible(panel, self.dock.is_open(panel));
        }
        self.documents.push(GerberDocumentState {
            id,
            title: title.clone(),
            viewer,
        });
        self.active_document = id;
        self.dock.add_document(id, &title);
        id
    }

    pub fn rename_document(&mut self, document_id: GerberDocumentId, title: String) {
        if let Some(document) = self
            .documents
            .iter_mut()
            .find(|document| document.id == document_id)
        {
            document.title = title.clone();
            self.dock.rename_document(document_id, &title);
        }
    }

    pub fn set_tool_visible(&mut self, panel: GerberDockPanel, visible: bool) {
        for document in &mut self.documents {
            document.viewer.set_tool_panel_visible(panel, visible);
        }
        if visible {
            self.dock.show_tool(panel);
        } else {
            self.dock.close_tool(panel);
        }
    }

    pub fn handle_dock_event(
        &mut self,
        event: &iced_dock::DockEvent<GerberDockPanel>,
    ) -> Option<GerberDocumentId> {
        match event {
            iced_dock::DockEvent::TabSelected {
                panel: GerberDockPanel::Document(document_id),
                ..
            }
            | iced_dock::DockEvent::PaneFocused {
                panel: Some(GerberDockPanel::Document(document_id)),
                ..
            } => {
                self.active_document = *document_id;
                None
            }
            iced_dock::DockEvent::TabClosed {
                panel: GerberDockPanel::Document(document_id),
            } => {
                self.documents
                    .retain(|document| document.id != *document_id);
                if self.documents.is_empty() {
                    self.new_document();
                } else if self.active_document == *document_id {
                    self.active_document = self.documents[0].id;
                    self.dock.select_document(self.active_document);
                }
                Some(*document_id)
            }
            iced_dock::DockEvent::TabClosed { panel } => {
                self.set_tool_visible(*panel, false);
                None
            }
            _ => None,
        }
    }
}
