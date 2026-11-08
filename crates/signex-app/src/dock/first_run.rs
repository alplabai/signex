//! First-run panel layout of the Graphite themes: a short Messages dock at
//! the bottom (ERC and History as its other tabs). A saved layout never
//! comes through here.

use super::types::{DockArea, EditorSeeds, PanelPosition};
use crate::panels::PanelKind;

/// Height of the bottom dock in the Graphite design.
pub const GRAPHITE_BOTTOM_HEIGHT: f32 = 208.0;

/// Fresh layout: Projects, Components and Navigator on the left, Properties
/// and Signex AI on the right, Messages, ERC and History at the bottom. Library
/// opens on demand (View menu, Library menu) into its default home.
pub fn graphite_first_run() -> DockArea {
    let mut dock = DockArea::new();
    let layout = [
        (PanelPosition::Left, PanelKind::Projects),
        (PanelPosition::Left, PanelKind::Components),
        (PanelPosition::Left, PanelKind::Navigator),
        (PanelPosition::Right, PanelKind::Properties),
        (PanelPosition::Right, PanelKind::SignexAi),
        (PanelPosition::Bottom, PanelKind::Messages),
        (PanelPosition::Bottom, PanelKind::Erc),
        (PanelPosition::Bottom, PanelKind::History),
    ];
    for (position, kind) in layout {
        dock.add_panel(position, kind);
    }
    dock.editor_seeds = EditorSeeds {
        footprint: true,
        symbol: true,
        pcb: true,
    };
    dock
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dock::PanelSite;

    #[test]
    fn messages_leads_the_bottom_dock() {
        let dock = graphite_first_run();
        assert_eq!(
            dock.locate(PanelKind::Messages),
            Some(PanelSite::Docked(PanelPosition::Bottom, 0))
        );
        assert!(dock.locate(PanelKind::Erc).is_some());
        assert!(dock.locate(PanelKind::History).is_some());
    }
}
