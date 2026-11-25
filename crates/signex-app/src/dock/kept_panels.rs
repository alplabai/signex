//! Extension panels in the dock (#2495): the keys a saved layout names that
//! no installed extension registered are kept and written back where they
//! stood, and a key an installed extension did register is docked as its
//! panel (DECISIONS 2026-11-25).

use super::KeptPanelKey;
use super::types::{DockArea, DockRegion, PanelPosition};
use crate::extension::points::ExtensionPanelKind;
use crate::panels::PanelKind;

const POSITIONS: [PanelPosition; 3] = [
    PanelPosition::Left,
    PanelPosition::Right,
    PanelPosition::Bottom,
];

impl DockArea {
    fn region_of(&self, position: PanelPosition) -> &DockRegion {
        match position {
            PanelPosition::Left => &self.left,
            PanelPosition::Right => &self.right,
            PanelPosition::Bottom => &self.bottom,
        }
    }

    fn region_of_mut(&mut self, position: PanelPosition) -> &mut DockRegion {
        match position {
            PanelPosition::Left => &mut self.left,
            PanelPosition::Right => &mut self.right,
            PanelPosition::Bottom => &mut self.bottom,
        }
    }

    /// The extension panel keys `position` keeps for a later run, in
    /// stored order.
    pub fn kept_panel_keys(&self, position: PanelPosition) -> &[KeptPanelKey] {
        &self.region_of(position).kept
    }

    /// Keep `kept` in `position`: the keys a stored layout named that no
    /// installed extension registered.
    pub(crate) fn set_kept_panel_keys(&mut self, position: PanelPosition, kept: Vec<KeptPanelKey>) {
        self.region_of_mut(position).kept = kept;
    }

    /// Dock each kept key that `find` resolves to a registered panel at its
    /// stored place, and keep the rest. A panel already open elsewhere is
    /// not docked twice. The active tab stays on the panel it was on.
    pub fn adopt_extension_panels(&mut self, find: impl Fn(&str) -> Option<ExtensionPanelKind>) {
        for position in POSITIONS {
            let kept = std::mem::take(&mut self.region_of_mut(position).kept);
            let mut still_kept: Vec<KeptPanelKey> = Vec::new();
            for entry in kept {
                let Some(panel) = find(entry.key.as_str()) else {
                    still_kept.push(entry);
                    continue;
                };
                let kind = PanelKind::Extension(panel);
                if self.locate(kind).is_some() {
                    continue;
                }
                let region = self.region_of_mut(position);
                // Its stored place counts the keys still kept before it.
                let at = entry
                    .index
                    .saturating_sub(still_kept.len())
                    .min(region.panels.len());
                region.panels.insert(at, kind);
                if region.panels.len() > 1 && at <= region.active {
                    region.active += 1;
                }
            }
            self.region_of_mut(position).kept = still_kept;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extension::points::Panels;

    fn kept(index: usize, key: &str) -> KeptPanelKey {
        KeptPanelKey {
            index,
            key: key.to_owned(),
        }
    }

    /// A registered panel kind for `key`, as an extension named `sample`
    /// would register it.
    fn registered(key: &'static str, home: PanelPosition) -> ExtensionPanelKind {
        struct Blank;
        impl crate::extension::ExtensionView for Blank {
            fn view<'a>(
                &'a self,
                _cx: crate::extension::ViewContext<'a>,
            ) -> iced::Element<'a, crate::extension::ExtensionMsg> {
                iced::widget::Space::new().into()
            }
        }
        let owner = crate::extension::ExtensionId::new("sample").expect("valid name");
        let mut panels = Panels::for_tests(owner);
        panels
            .add(key, signex_i18n::ids::PANELS_KIND_WIKI, home, Blank)
            .expect("registered")
    }

    #[test]
    fn a_registered_key_is_docked_at_its_place_and_the_rest_stay_kept() {
        let notes = registered("ext.sample.notes", PanelPosition::Right);
        let mut dock = DockArea::new();
        dock.add_panel(PanelPosition::Right, PanelKind::Properties);
        dock.add_panel(PanelPosition::Right, PanelKind::History);
        // Stored: properties, ext.other.log, ext.sample.notes, history.
        dock.set_kept_panel_keys(
            PanelPosition::Right,
            vec![kept(1, "ext.other.log"), kept(2, "ext.sample.notes")],
        );
        dock.update(crate::dock::DockMessage::SelectTab(PanelPosition::Right, 1));

        dock.adopt_extension_panels(|key| (key == notes.key()).then_some(notes));

        assert_eq!(
            dock.panel_kinds(PanelPosition::Right),
            [
                PanelKind::Properties,
                PanelKind::Extension(notes),
                PanelKind::History
            ]
        );
        assert_eq!(
            dock.kept_panel_keys(PanelPosition::Right),
            [kept(1, "ext.other.log")]
        );
        // History was in front and still is.
        assert!(dock.is_frontmost(PanelKind::History));
    }

    #[test]
    fn a_panel_already_open_is_not_docked_twice() {
        let notes = registered("ext.sample.notes", PanelPosition::Left);
        let mut dock = DockArea::new();
        dock.add_panel(PanelPosition::Left, PanelKind::Extension(notes));
        dock.set_kept_panel_keys(PanelPosition::Bottom, vec![kept(0, "ext.sample.notes")]);

        dock.adopt_extension_panels(|key| (key == notes.key()).then_some(notes));

        assert!(dock.panel_kinds(PanelPosition::Bottom).is_empty());
        assert!(dock.kept_panel_keys(PanelPosition::Bottom).is_empty());
        assert_eq!(
            dock.panel_kinds(PanelPosition::Left),
            [PanelKind::Extension(notes)]
        );
    }
}
