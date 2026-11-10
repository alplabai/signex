//! Default dock placement — one home region per panel kind.

use super::types::{DockArea, EditorContext, FloatingPanel, PanelPosition};
use crate::panels::PanelKind;
use crate::styles::TypeScale;
use crate::styles::typography::DEFAULT_BASE_SIZE;

/// A length that was 1:1 pixels at the default font size, scaled by the
/// default type scale's body size. Compile-time only: it does not follow a
/// UI scale chosen at runtime.
const fn em(px_at_default: f32) -> f32 {
    TypeScale::DEFAULT.body * px_at_default / DEFAULT_BASE_SIZE
}

/// Drop zone depth of a collapsed side rail.
const SIDE_RAIL_ZONE: f32 = em(40.0);
/// Smallest drop zone of an expanded side region.
const SIDE_MIN_ZONE: f32 = em(80.0);
/// Drop zone of a side with no panels in it.
const SIDE_EMPTY_ZONE: f32 = em(140.0);
/// Drop zone depth of a collapsed bottom rail.
const BOTTOM_RAIL_ZONE: f32 = em(32.0);
/// Smallest drop zone of an expanded bottom region.
const BOTTOM_MIN_ZONE: f32 = em(60.0);
/// Drop zone of a bottom region with no panels in it.
const BOTTOM_EMPTY_ZONE: f32 = em(120.0);

/// One dock region as the drop decision sees it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RegionExtent {
    /// Whether the region holds any panel.
    pub occupied: bool,
    /// Whether it is folded to its rail.
    pub collapsed: bool,
    /// Its configured width (side regions) or height (bottom).
    pub size: f32,
}

impl RegionExtent {
    /// Depth of the drop zone, from the window edge inward.
    fn zone(self, rail: f32, min: f32, empty: f32) -> f32 {
        match (self.occupied, self.collapsed) {
            (false, _) => empty,
            (true, true) => rail,
            (true, false) => self.size.max(min),
        }
    }

    /// Depth of the area the preview paints: what the region takes once the
    /// panel lands in it. An empty region opens at its configured size.
    fn preview_depth(self, rail: f32, min: f32) -> f32 {
        match (self.occupied, self.collapsed) {
            (true, true) => rail,
            (true, false) => self.size.max(min),
            (false, _) => self.size,
        }
    }
}

/// Window and dock region extents a drop is decided against.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DropGeometry {
    pub window: (f32, f32),
    pub left: RegionExtent,
    pub right: RegionExtent,
    pub bottom: RegionExtent,
}

impl DropGeometry {
    /// Depth of the strip the preview covers for `position`.
    pub fn preview_depth(&self, position: PanelPosition) -> f32 {
        match position {
            PanelPosition::Left => self.left.preview_depth(SIDE_RAIL_ZONE, SIDE_MIN_ZONE),
            PanelPosition::Right => self.right.preview_depth(SIDE_RAIL_ZONE, SIDE_MIN_ZONE),
            PanelPosition::Bottom => self.bottom.preview_depth(BOTTOM_RAIL_ZONE, BOTTOM_MIN_ZONE),
        }
    }
}

/// The region a floating `panel` docks into if released where it is, or
/// `None` to stay floating. The one decision behind both the live drop
/// preview and the release, so they cannot disagree. It tests the panel's
/// top-centre (`x + width/2`, `y + height/4`), left edge first.
pub fn drop_target(panel: &FloatingPanel, geometry: &DropGeometry) -> Option<PanelPosition> {
    let (window_w, window_h) = geometry.window;
    let x = panel.x + panel.width / 2.0;
    let y = panel.y + panel.height / 4.0;
    let left = geometry
        .left
        .zone(SIDE_RAIL_ZONE, SIDE_MIN_ZONE, SIDE_EMPTY_ZONE);
    let right = geometry
        .right
        .zone(SIDE_RAIL_ZONE, SIDE_MIN_ZONE, SIDE_EMPTY_ZONE);
    let bottom = geometry
        .bottom
        .zone(BOTTOM_RAIL_ZONE, BOTTOM_MIN_ZONE, BOTTOM_EMPTY_ZONE);
    if x <= left {
        Some(PanelPosition::Left)
    } else if x >= window_w - right {
        Some(PanelPosition::Right)
    } else if y >= window_h - bottom {
        Some(PanelPosition::Bottom)
    } else {
        None
    }
}

impl DockArea {
    /// The drop geometry for a window of `window` size and the configured
    /// region sizes `(left width, right width, bottom height)`.
    pub fn drop_geometry(&self, window: (f32, f32), sizes: (f32, f32, f32)) -> DropGeometry {
        let extent = |position, size| RegionExtent {
            occupied: self.has_panels(position),
            collapsed: self.is_collapsed(position),
            size,
        };
        DropGeometry {
            window,
            left: extent(PanelPosition::Left, sizes.0),
            right: extent(PanelPosition::Right, sizes.1),
            bottom: extent(PanelPosition::Bottom, sizes.2),
        }
    }
}

impl PanelPosition {
    /// The region `kind` opens into when it is not already on screen.
    ///
    /// Every "open panel X" gesture routes through here — the View
    /// menu, the status-bar panel list, an ERC run surfacing its
    /// results, TAB during placement surfacing Properties, re-docking
    /// a floating panel, closing a detached panel window. Before #641
    /// each of those picked its own region, so the same kind could be
    /// docked in all three at once: Signex AI booted Left, the View menu
    /// added a second one Bottom, and the panel list a third Right.
    ///
    /// The values match the first-run layout seeded in
    /// `app/bootstrap/new.rs` for the kinds it seeds. Kinds reachable
    /// only from the panel list default to `Right`, which is where
    /// that list used to send everything.
    pub fn default_for(kind: PanelKind) -> Self {
        match kind {
            PanelKind::Projects
            | PanelKind::Components
            | PanelKind::Library
            | PanelKind::FootprintLayers
            | PanelKind::PcbNets
            | PanelKind::SignexAi => PanelPosition::Left,
            PanelKind::Erc | PanelKind::SymbolPins => PanelPosition::Bottom,
            PanelKind::Navigator
            | PanelKind::Properties
            | PanelKind::Messages
            | PanelKind::Filter
            | PanelKind::SchFilter
            | PanelKind::SchList
            | PanelKind::Drc
            | PanelKind::LayerStack
            | PanelKind::Layers
            | PanelKind::NetClasses
            | PanelKind::Variants
            | PanelKind::OutputJobs
            | PanelKind::BomStudio
            | PanelKind::Favorites
            | PanelKind::Snippets
            | PanelKind::Todo
            | PanelKind::Wiki
            | PanelKind::SchLibrary
            | PanelKind::FootprintLibrary
            | PanelKind::History => PanelPosition::Right,
        }
    }
}

impl DockArea {
    /// Give a fresh Graphite layout the panels an editor tab needs: the
    /// footprint editor gets Footprint Library and Layers on the left, the
    /// symbol editor the Pins table at the bottom. Runs once per context and
    /// only for a layout `graphite_first_run` built; a saved layout (the
    /// user's own arrangement) never has a seed owed, so it is never touched.
    pub fn seed_for_editor(&mut self, context: EditorContext) {
        match context {
            EditorContext::Footprint if self.editor_seeds.footprint => {
                self.editor_seeds.footprint = false;
                self.add_panel(PanelPosition::Left, PanelKind::FootprintLayers);
                self.show_panel_at(PanelPosition::Left, PanelKind::FootprintLibrary);
            }
            EditorContext::Symbol if self.editor_seeds.symbol => {
                self.editor_seeds.symbol = false;
                self.show_panel_at(PanelPosition::Bottom, PanelKind::SymbolPins);
            }
            EditorContext::Pcb if self.editor_seeds.pcb => {
                self.editor_seeds.pcb = false;
                self.show_panel_at(PanelPosition::Left, PanelKind::PcbNets);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WINDOW: (f32, f32) = (1200.0, 800.0);

    fn region(occupied: bool, collapsed: bool, size: f32) -> RegionExtent {
        RegionExtent {
            occupied,
            collapsed,
            size,
        }
    }

    fn geometry() -> DropGeometry {
        DropGeometry {
            window: WINDOW,
            left: region(true, false, 250.0),
            right: region(true, false, 300.0),
            bottom: region(true, false, 200.0),
        }
    }

    /// A panel whose drop point (`x + w/2`, `y + h/4`) is `(cx, cy)`.
    fn panel_at(cx: f32, cy: f32) -> FloatingPanel {
        FloatingPanel {
            kind: PanelKind::Messages,
            x: cx - 140.0,
            y: cy - 100.0,
            width: 280.0,
            height: 400.0,
            dragging: true,
        }
    }

    #[test]
    fn expanded_regions_use_their_own_size() {
        let g = geometry();
        assert_eq!(
            drop_target(&panel_at(250.0, 300.0), &g),
            Some(PanelPosition::Left)
        );
        assert_eq!(drop_target(&panel_at(251.0, 300.0), &g), None);
        assert_eq!(
            drop_target(&panel_at(900.0, 300.0), &g),
            Some(PanelPosition::Right)
        );
        assert_eq!(drop_target(&panel_at(899.0, 300.0), &g), None);
        assert_eq!(
            drop_target(&panel_at(600.0, 600.0), &g),
            Some(PanelPosition::Bottom)
        );
        assert_eq!(drop_target(&panel_at(600.0, 599.0), &g), None);
    }

    #[test]
    fn a_narrow_region_still_has_the_minimum_zone() {
        let mut g = geometry();
        g.left = region(true, false, 10.0);
        assert_eq!(
            drop_target(&panel_at(SIDE_MIN_ZONE, 300.0), &g),
            Some(PanelPosition::Left)
        );
        assert_eq!(drop_target(&panel_at(SIDE_MIN_ZONE + 1.0, 300.0), &g), None);
        g.bottom = region(true, false, 10.0);
        let edge = WINDOW.1 - BOTTOM_MIN_ZONE;
        assert_eq!(
            drop_target(&panel_at(600.0, edge), &g),
            Some(PanelPosition::Bottom)
        );
        assert_eq!(drop_target(&panel_at(600.0, edge - 1.0), &g), None);
    }

    #[test]
    fn a_collapsed_region_is_only_its_rail() {
        let mut g = geometry();
        g.left = region(true, true, 250.0);
        g.right = region(true, true, 300.0);
        g.bottom = region(true, true, 200.0);
        assert_eq!(
            drop_target(&panel_at(SIDE_RAIL_ZONE, 300.0), &g),
            Some(PanelPosition::Left)
        );
        assert_eq!(
            drop_target(&panel_at(SIDE_RAIL_ZONE + 1.0, 300.0), &g),
            None
        );
        let right_edge = WINDOW.0 - SIDE_RAIL_ZONE;
        assert_eq!(
            drop_target(&panel_at(right_edge, 300.0), &g),
            Some(PanelPosition::Right)
        );
        assert_eq!(drop_target(&panel_at(right_edge - 1.0, 300.0), &g), None);
        let bottom_edge = WINDOW.1 - BOTTOM_RAIL_ZONE;
        assert_eq!(
            drop_target(&panel_at(600.0, bottom_edge), &g),
            Some(PanelPosition::Bottom)
        );
        assert_eq!(drop_target(&panel_at(600.0, bottom_edge - 1.0), &g), None);
    }

    #[test]
    fn an_empty_region_has_a_generous_zone() {
        let mut g = geometry();
        g.left = region(false, false, 250.0);
        g.right = region(false, false, 300.0);
        g.bottom = region(false, false, 200.0);
        assert_eq!(
            drop_target(&panel_at(SIDE_EMPTY_ZONE, 300.0), &g),
            Some(PanelPosition::Left)
        );
        assert_eq!(
            drop_target(&panel_at(SIDE_EMPTY_ZONE + 1.0, 300.0), &g),
            None
        );
        let right_edge = WINDOW.0 - SIDE_EMPTY_ZONE;
        assert_eq!(
            drop_target(&panel_at(right_edge, 300.0), &g),
            Some(PanelPosition::Right)
        );
        let bottom_edge = WINDOW.1 - BOTTOM_EMPTY_ZONE;
        assert_eq!(
            drop_target(&panel_at(600.0, bottom_edge), &g),
            Some(PanelPosition::Bottom)
        );
        assert_eq!(drop_target(&panel_at(600.0, bottom_edge - 1.0), &g), None);
    }

    #[test]
    fn the_left_edge_wins_a_corner_and_the_middle_floats() {
        let g = geometry();
        assert_eq!(
            drop_target(&panel_at(10.0, 790.0), &g),
            Some(PanelPosition::Left)
        );
        assert_eq!(
            drop_target(&panel_at(1190.0, 790.0), &g),
            Some(PanelPosition::Right)
        );
        assert_eq!(drop_target(&panel_at(600.0, 300.0), &g), None);
    }

    #[test]
    fn the_preview_covers_what_the_region_will_occupy() {
        let mut g = geometry();
        assert_eq!(g.preview_depth(PanelPosition::Left), 250.0);
        g.left = region(true, true, 250.0);
        assert_eq!(g.preview_depth(PanelPosition::Left), SIDE_RAIL_ZONE);
        g.right = region(false, false, 300.0);
        assert_eq!(g.preview_depth(PanelPosition::Right), 300.0);
    }

    #[test]
    fn a_fresh_layout_seeds_each_editor_dock_once() {
        let mut dock = crate::dock::graphite_first_run();
        dock.seed_for_editor(EditorContext::Footprint);
        assert!(matches!(
            dock.locate(PanelKind::FootprintLayers),
            Some(crate::dock::PanelSite::Docked(PanelPosition::Left, _))
        ));
        dock.seed_for_editor(EditorContext::Symbol);
        assert!(matches!(
            dock.locate(PanelKind::SymbolPins),
            Some(crate::dock::PanelSite::Docked(PanelPosition::Bottom, _))
        ));
        // A panel the user closed stays closed on the next tab switch.
        assert!(dock.close_panel(PanelKind::SymbolPins));
        dock.seed_for_editor(EditorContext::Symbol);
        assert!(dock.locate(PanelKind::SymbolPins).is_none());
    }

    #[test]
    fn a_saved_layout_is_never_seeded() {
        let mut dock = DockArea::new();
        dock.seed_for_editor(EditorContext::Footprint);
        dock.seed_for_editor(EditorContext::Symbol);
        assert!(dock.locate(PanelKind::FootprintLayers).is_none());
        assert!(dock.locate(PanelKind::SymbolPins).is_none());
    }
}
