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

/// The group `kind` is listed under. `None` for a tool document: Layer
/// Stack, BOM Studio, Export Jobs, Variants and the Design Notebook open as
/// document tabs of their project, so the list leaves them out (DECISIONS
/// 2026-11-18, mockup row 7). To-Do is left out too: it folds into the
/// Design Notebook (decision of 2026-11-24, mockup row 7).
pub fn panel_group(kind: PanelKind) -> Option<PanelGroup> {
    let group = match kind {
        PanelKind::LayerStack
        | PanelKind::BomStudio
        | PanelKind::OutputJobs
        | PanelKind::Variants
        | PanelKind::Wiki
        | PanelKind::Todo => return None,
        PanelKind::Projects
        | PanelKind::Components
        | PanelKind::Library
        | PanelKind::Navigator
        | PanelKind::Annotations
        | PanelKind::Favorites
        | PanelKind::Snippets
        // Classes belong to the project (#2417); on a board they are a mode
        // of the Nets panel (mockup row 106), so the PCB group has no row.
        | PanelKind::NetClasses
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
        PanelKind::Layers | PanelKind::PcbNets => PanelGroup::Pcb,
    };
    Some(group)
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
                .filter(|&kind| panel_group(kind) == Some(group) && is_shown(kind))
                .collect();
            (group, kinds)
        })
        .filter(|(_, kinds)| !kinds.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tool documents that open as tabs (DECISIONS 2026-11-18).
    const TOOL_DOCUMENTS: [PanelKind; 5] = [
        PanelKind::LayerStack,
        PanelKind::BomStudio,
        PanelKind::OutputJobs,
        PanelKind::Variants,
        PanelKind::Wiki,
    ];

    /// Panels the list leaves out: the tool documents, and To-Do, which
    /// folds into the Design Notebook (decision of 2026-11-24).
    fn is_unlisted(kind: PanelKind) -> bool {
        TOOL_DOCUMENTS.contains(&kind) || kind == PanelKind::Todo
    }

    /// Every panel of the list sits in exactly one group, so grouping
    /// drops none and lists none twice; an unlisted panel sits in none.
    #[test]
    fn every_panel_is_in_exactly_one_group() {
        let grouped = grouped_panels(|_| true);
        let listed: Vec<PanelKind> = grouped.iter().flat_map(|(_, k)| k.clone()).collect();
        let unlisted = ALL_PANELS.iter().filter(|&&k| is_unlisted(k)).count();
        assert_eq!(unlisted, TOOL_DOCUMENTS.len() + 1);
        assert_eq!(listed.len(), ALL_PANELS.len() - unlisted);
        for kind in ALL_PANELS {
            let expected = usize::from(!is_unlisted(*kind));
            assert_eq!(
                listed.iter().filter(|&&k| k == *kind).count(),
                expected,
                "{kind:?}"
            );
        }
    }

    /// Mockup row 7: Layer Stack, BOM Studio, Export Jobs, Variants and the
    /// Design Notebook are document tabs, so the panel list never lists
    /// them, whatever is open; each is still a panel kind the command
    /// palette reaches.
    #[test]
    fn the_tool_documents_are_not_listed() {
        for kind in TOOL_DOCUMENTS {
            assert!(ALL_PANELS.contains(&kind), "{kind:?}");
            assert_eq!(panel_group(kind), None, "{kind:?}");
        }
        let listed: Vec<PanelKind> = grouped_panels(|_| true)
            .into_iter()
            .flat_map(|(_, kinds)| kinds)
            .collect();
        assert!(
            TOOL_DOCUMENTS.iter().all(|kind| !listed.contains(kind)),
            "{listed:?}"
        );
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
                K::Favorites,
                K::Snippets,
                K::NetClasses,
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
        assert_eq!(members(PanelGroup::Pcb), [K::Layers, K::PcbNets]);
    }

    /// Mockup row 7, decision of 2026-11-24: To-Do folds into the Design
    /// Notebook, so the list never lists it, in no group and with no
    /// document open; it is still a panel kind the command palette reaches.
    #[test]
    fn to_do_is_not_listed() {
        assert!(ALL_PANELS.contains(&PanelKind::Todo));
        assert_eq!(panel_group(PanelKind::Todo), None);
        assert!(
            grouped_panels(|_| true)
                .iter()
                .all(|(_, kinds)| !kinds.contains(&PanelKind::Todo))
        );
    }

    /// Mockup row 7 lists the PCB group as Layers and Nets only: Net Classes
    /// is a project panel (#2417), so a sheet with no board shows no PCB
    /// caption for it.
    #[test]
    fn net_classes_is_a_project_panel() {
        assert_eq!(
            panel_group(PanelKind::NetClasses),
            Some(PanelGroup::Project)
        );
        let sheet_only = grouped_panels(|kind| !kind.needs_pcb());
        assert!(
            sheet_only.iter().all(|(g, _)| *g != PanelGroup::Pcb),
            "{sheet_only:?}"
        );
    }

    /// A group whose panels are all hidden gets no caption: it is left out.
    #[test]
    fn a_group_with_no_shown_panel_is_omitted() {
        let grouped = grouped_panels(|kind| panel_group(kind) != Some(PanelGroup::Pcb));
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
