//! The groups of the status bar's panel list (mockup row 7): Project, Inspect, Verify,
//! Library and PCB, each under its own caption.

use signex_i18n::{MessageId, ids};

use super::{ALL_PANELS, PanelKind};

/// A group of the panel list, in the order the list shows them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PanelGroup {
    /// The project and its parts: files, components, libraries, notes.
    Project,
    /// What the selection is and how to pick it.
    Inspect,
    /// Checks, messages and history.
    Verify,
    /// The library editors' panels.
    Library,
    /// The board's panels.
    Pcb,
}

impl PanelGroup {
    /// Every group, top to bottom.
    pub const ALL: [PanelGroup; 5] = [
        PanelGroup::Project,
        PanelGroup::Inspect,
        PanelGroup::Verify,
        PanelGroup::Library,
        PanelGroup::Pcb,
    ];

    /// The group's caption in the list.
    pub fn caption(self) -> MessageId {
        match self {
            PanelGroup::Project => ids::PANELS_LIST_GROUP_PROJECT,
            PanelGroup::Inspect => ids::PANELS_LIST_GROUP_INSPECT,
            PanelGroup::Verify => ids::PANELS_LIST_GROUP_VERIFY,
            PanelGroup::Library => ids::PANELS_LIST_GROUP_LIBRARY,
            PanelGroup::Pcb => ids::PANELS_LIST_GROUP_PCB,
        }
    }
}

/// The group `kind` is listed under.
pub fn panel_group(kind: PanelKind) -> PanelGroup {
    match kind {
        PanelKind::Projects
        | PanelKind::Components
        | PanelKind::Library
        | PanelKind::Navigator
        | PanelKind::Annotations
        | PanelKind::Favorites
        | PanelKind::Snippets
        | PanelKind::Variants
        | PanelKind::BomStudio
        | PanelKind::OutputJobs
        | PanelKind::Wiki
        | PanelKind::Todo
        // A sibling tab of Projects (#1308).
        | PanelKind::CompiledDesign => PanelGroup::Project,
        PanelKind::Properties
        | PanelKind::Filter
        | PanelKind::SchFilter
        | PanelKind::ObjectList
        | PanelKind::NetInspector
        | PanelKind::Unions => PanelGroup::Inspect,
        PanelKind::Messages | PanelKind::Erc | PanelKind::Drc | PanelKind::History => {
            PanelGroup::Verify
        }
        PanelKind::CommandLog => PanelGroup::Verify,
        PanelKind::SchLibrary | PanelKind::FootprintLibrary => PanelGroup::Library,
        PanelKind::Layers
        | PanelKind::LayerStack
        | PanelKind::NetClasses
        | PanelKind::PcbNets
        | PanelKind::LengthMatching => PanelGroup::Pcb,
    }
}

/// The panels `is_shown` keeps, grouped: each group in [`PanelGroup::ALL`]
/// order with its panels in [`ALL_PANELS`] order. A group with no panel left
/// is omitted.
pub fn grouped_panels(is_shown: impl Fn(PanelKind) -> bool) -> Vec<(PanelGroup, Vec<PanelKind>)> {
    PanelGroup::ALL
        .into_iter()
        .map(|group| {
            let kinds: Vec<PanelKind> = ALL_PANELS
                .iter()
                .copied()
                .filter(|&kind| panel_group(kind) == group && is_shown(kind))
                .collect();
            (group, kinds)
        })
        .filter(|(_, kinds)| !kinds.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every panel of the list sits in exactly one group, so grouping
    /// drops none and lists none twice.
    #[test]
    fn every_panel_is_in_exactly_one_group() {
        let grouped = grouped_panels(|_| true);
        let listed: Vec<PanelKind> = grouped.iter().flat_map(|(_, k)| k.clone()).collect();
        assert_eq!(listed.len(), ALL_PANELS.len());
        for kind in ALL_PANELS {
            assert_eq!(
                listed.iter().filter(|&&k| k == *kind).count(),
                1,
                "{kind:?}"
            );
        }
    }

    /// The groups and their members as mockup row 7 lists them; inside a
    /// group the panels keep the list's order.
    #[test]
    fn the_groups_hold_the_mapped_panels_in_list_order() {
        let grouped = grouped_panels(|_| true);
        let groups: Vec<PanelGroup> = grouped.iter().map(|(g, _)| *g).collect();
        assert_eq!(groups, PanelGroup::ALL);
        let members = |group| {
            grouped
                .iter()
                .find(|(g, _)| *g == group)
                .map(|(_, k)| k.clone())
                .unwrap_or_default()
        };
        use PanelKind as K;
        assert_eq!(
            members(PanelGroup::Project),
            [
                K::Projects,
                K::Components,
                K::Library,
                K::Navigator,
                K::Annotations,
                K::BomStudio,
                K::Favorites,
                K::Snippets,
                K::Variants,
                K::OutputJobs,
                K::Todo,
                K::Wiki,
                K::CompiledDesign,
            ]
        );
        assert_eq!(
            members(PanelGroup::Inspect),
            [
                K::Properties,
                K::Filter,
                K::SchFilter,
                K::ObjectList,
                K::NetInspector,
                K::Unions,
            ]
        );
        assert_eq!(
            members(PanelGroup::Verify),
            [K::Erc, K::Messages, K::CommandLog, K::Drc, K::History]
        );
        assert_eq!(
            members(PanelGroup::Library),
            [K::SchLibrary, K::FootprintLibrary]
        );
        assert_eq!(
            members(PanelGroup::Pcb),
            [
                K::LayerStack,
                K::Layers,
                K::NetClasses,
                K::PcbNets,
                K::LengthMatching,
            ]
        );
    }

    /// A group whose panels are all hidden gets no caption: it is left out.
    #[test]
    fn a_group_with_no_shown_panel_is_omitted() {
        let grouped = grouped_panels(|kind| panel_group(kind) != PanelGroup::Pcb);
        assert!(grouped.iter().all(|(g, _)| *g != PanelGroup::Pcb));
        assert_eq!(grouped.len(), PanelGroup::ALL.len() - 1);
        assert!(grouped_panels(|_| false).is_empty());
    }

    /// Each group has its own caption.
    #[test]
    fn each_group_has_its_own_caption() {
        let captions: std::collections::HashSet<_> =
            PanelGroup::ALL.iter().map(|g| g.caption()).collect();
        assert_eq!(captions.len(), PanelGroup::ALL.len());
    }
}
