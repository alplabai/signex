//! Default dock placement — one home region per panel kind.

use super::types::PanelPosition;
use crate::panels::PanelKind;

impl PanelPosition {
    /// The region `kind` opens into when it is not already on screen.
    ///
    /// Every "open panel X" gesture routes through here — the View
    /// menu, the status-bar panel list, an ERC run surfacing its
    /// results, TAB during placement surfacing Properties, re-docking
    /// a floating panel, closing a detached panel window. Before #641
    /// each of those picked its own region, so the same kind could be
    /// docked in all three at once: Signal booted Left, the View menu
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
            | PanelKind::Signal => PanelPosition::Left,
            PanelKind::Erc => PanelPosition::Bottom,
            PanelKind::Navigator
            | PanelKind::Properties
            | PanelKind::Messages
            | PanelKind::Filter
            | PanelKind::SchFilter
            | PanelKind::SchList
            | PanelKind::Drc
            | PanelKind::LayerStack
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
