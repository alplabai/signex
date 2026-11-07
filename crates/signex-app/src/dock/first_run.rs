//! First-run panel layout of the Graphite themes: a short Messages dock at
//! the bottom (ERC and History as its other tabs). A saved layout never
//! comes through here.

use super::types::{DockArea, EditorSeeds, PanelPosition};
use crate::panels::PanelKind;

/// Height of the bottom dock in the Graphite design.
pub const GRAPHITE_BOTTOM_HEIGHT: f32 = 208.0;

/// Fresh layout: Projects, Components, Navigator, Library and Signal on the
/// left, Properties on the right, Messages, ERC and History at the bottom.
pub fn graphite_first_run() -> DockArea {
    let mut dock = DockArea::new();
    let layout = [
        (PanelPosition::Left, PanelKind::Projects),
        (PanelPosition::Left, PanelKind::Components),
        (PanelPosition::Left, PanelKind::Navigator),
        (PanelPosition::Left, PanelKind::Library),
        (PanelPosition::Left, PanelKind::Signal),
        (PanelPosition::Right, PanelKind::Properties),
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
