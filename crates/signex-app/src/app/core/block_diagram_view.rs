//! What the block diagram tab shows of one open diagram (mockup row 171):
//! the selection, the armed tool, the camera, the pane, the Properties text
//! being typed, the export popover and the Re-sync review sheet. View state,
//! kept with the GUI side (`DocumentViewState::block_diagrams`, ADR-0006
//! slice 4a); the diagram and its history stay in `DocumentState`. Pure data
//! and pure rules.

use std::collections::BTreeMap;

use signex_types::block_diagram::{BlockDiagram, Endpoint};
use uuid::Uuid;

use super::block_diagram_editor::{
    Camera, DiagramItem, DiagramTool, DiagramView, ExportDraft, PowerTreeView, PropertyField,
    ResyncReview, has_item,
};

/// The tab's view of one diagram.
#[derive(Debug, Clone, Default)]
pub struct BlockDiagramViewState {
    pub selection: Vec<DiagramItem>,
    pub tool: DiagramTool,
    /// The first end picked with the Connect tool.
    pub connect_from: Option<Endpoint>,
    pub camera: Camera,
    /// Properties text typed and not committed yet.
    pub drafts: BTreeMap<PropertyField, String>,
    /// What the tab shows.
    pub view: DiagramView,
    pub power: PowerTreeView,
    /// The export popover while it is open.
    pub export: Option<ExportDraft>,
    /// Show GND (mockup row 171): ground connections are hidden until on.
    pub show_ground: bool,
    /// The Re-sync review while it is open (mockup row 210).
    pub review: Option<ResyncReview>,
}

impl BlockDiagramViewState {
    /// Drop the selected objects and the connect start `diagram` no longer
    /// has, and the drafts typed for them.
    pub fn prune(&mut self, diagram: &BlockDiagram) {
        let before = self.selection.clone();
        self.selection.retain(|item| has_item(diagram, *item));
        if self.selection != before {
            self.drafts.clear();
        }
        let start_gone = self.connect_from.is_some_and(|end| {
            diagram
                .block(end.block)
                .is_none_or(|b| end.port.is_some_and(|p| b.port(p).is_none()))
        });
        if start_gone {
            self.connect_from = None;
        }
    }

    /// The one selected object, when exactly one is.
    pub fn single_selection(&self) -> Option<DiagramItem> {
        match self.selection.as_slice() {
            [item] => Some(*item),
            _ => None,
        }
    }

    /// The selected blocks, in selection order.
    pub fn selected_blocks(&self) -> Vec<Uuid> {
        self.selection
            .iter()
            .filter_map(|item| match item {
                DiagramItem::Block(id) => Some(*id),
                _ => None,
            })
            .collect()
    }

    /// Pick `item`, or with `add` add it to the selection or take it out.
    pub fn pick(&mut self, item: Option<DiagramItem>, add: bool) {
        self.drafts.clear();
        match (item, add) {
            (None, false) => self.selection.clear(),
            (None, true) => {}
            (Some(item), false) => self.selection = vec![item],
            (Some(item), true) => {
                if let Some(at) = self.selection.iter().position(|i| *i == item) {
                    self.selection.remove(at);
                } else {
                    self.selection.push(item);
                }
            }
        }
    }
}
