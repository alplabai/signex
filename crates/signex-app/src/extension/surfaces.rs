//! What the installed extensions add to each surface, read by the views and
//! the dock: their panels, menu and Active Bar rows, Start-screen sections
//! and preference values. Every list is in install order, then in the order
//! each extension added its entries.

use std::collections::BTreeMap;

use iced::widget::Space;
use iced::{Element, Widget as _};

use super::points::{ActiveBarRow, ExtensionPanelKind, MenuRow, PcbBarGroup, StartScreenSlot};
use super::{ExtensionHost, ExtensionMsg, PreferenceValues, ViewContext};
use crate::keymap::{AppCommandId, ExtensionCommandName, ExtensionCommandNames};
use signex_i18n::MessageId;

impl ExtensionHost {
    /// Every installed extension's dock panels, in install order: the
    /// Extensions group of the panel list and of the View menu.
    pub fn panels(&self) -> Vec<ExtensionPanelKind> {
        self.points()
            .flat_map(|points| points.panels.kinds())
            .collect()
    }

    /// The installed commands as the palette and the keymap surfaces name
    /// them: the extension's name and the command's label, by id.
    pub fn command_names(&self) -> ExtensionCommandNames {
        let mut names = ExtensionCommandNames::default();
        for entry in &self.installed {
            for command in entry.points.commands.iter() {
                let metadata = command.metadata();
                let Ok(id) = AppCommandId::new(metadata.id) else {
                    continue;
                };
                names.insert(
                    id,
                    ExtensionCommandName {
                        extension: entry.id.name().to_owned(),
                        label: metadata.label,
                        category: metadata.category,
                    },
                );
            }
        }
        names
    }
}

/// One Start-screen section, drawn: its heading and its body.
pub(crate) struct DrawnSection<'a> {
    pub title: MessageId,
    pub body: Element<'a, ExtensionMsg>,
}

impl ExtensionHost {
    /// The registered panel saved under `key`, if an installed extension
    /// added one.
    pub fn panel(&self, key: &str) -> Option<ExtensionPanelKind> {
        self.points().find_map(|points| points.panels.find(key))
    }

    /// Draw the panel `kind`. A kind no installed extension registered
    /// draws as empty space.
    pub(crate) fn view_panel<'a>(
        &'a self,
        kind: ExtensionPanelKind,
        cx: ViewContext<'a>,
    ) -> Element<'a, ExtensionMsg> {
        self.points()
            .find_map(|points| points.panels.view(kind.key()))
            .map_or_else(|| Space::new().boxed(), |view| view.view(cx))
    }

    /// Every menu bar row the extensions added. Owned: the menu context and
    /// the native menu's state keep them. With no extension it allocates
    /// nothing.
    pub fn menu_rows(&self) -> Vec<MenuRow> {
        self.points()
            .flat_map(|points| points.menu_rows.menu().iter().cloned())
            .collect()
    }

    /// Every Active Bar row the extensions added.
    pub fn active_bar_rows(&self) -> Vec<ActiveBarRow> {
        self.points()
            .flat_map(|points| points.menu_rows.active_bar().iter().cloned())
            .collect()
    }

    /// The Active Bar rows of the PCB tool group `group`.
    pub(crate) fn active_bar_rows_of(
        &self,
        group: PcbBarGroup,
    ) -> impl Iterator<Item = &ActiveBarRow> {
        self.points()
            .flat_map(|points| points.menu_rows.active_bar())
            .filter(move |row| row.group == group)
    }

    /// The sections of the Start-screen slot `slot`, drawn. Empty when no
    /// extension added one, so the Start screen draws nothing there.
    pub(crate) fn view_start_screen_slot<'a>(
        &'a self,
        slot: StartScreenSlot,
        cx: ViewContext<'a>,
    ) -> Vec<DrawnSection<'a>> {
        self.points()
            .flat_map(|points| points.start_screen.in_slot(slot))
            .map(|(_, title, view)| DrawnSection {
                title,
                body: view.view(cx),
            })
            .collect()
    }

    /// Hand each extension its preference values from `stored`, the
    /// `extensions` object of `prefs.json` (a key it did not add is not
    /// read; a missing one reads as its default). Called once at launch,
    /// before the first window; an extension that added no preference is
    /// not called.
    pub fn load_preferences(&mut self, stored: &BTreeMap<String, serde_json::Value>) {
        for entry in &mut self.installed {
            if entry.points.preferences.is_empty() {
                continue;
            }
            let values = entry.points.preferences.values_from(stored);
            entry.extension.preferences_loaded(&values);
            entry.preference_values = Some(values);
        }
    }

    /// The name of the extension installed at `index`.
    pub fn name_at(&self, index: usize) -> Option<&str> {
        self.installed.get(index).map(|entry| entry.id.name())
    }

    /// The Preferences page of each installed extension that added
    /// preferences, in install order.
    pub fn preference_pages(&self) -> impl Iterator<Item = PreferencePage<'_>> {
        (0..self.installed.len()).filter_map(|index| self.preference_page(index))
    }

    /// The Preferences page of the extension installed at `index`: its
    /// values as last loaded or saved, else its defaults. `None` when it
    /// added no preference.
    pub fn preference_page(&self, index: usize) -> Option<PreferencePage<'_>> {
        let entry = self.installed.get(index)?;
        if entry.points.preferences.is_empty() {
            return None;
        }
        let values = entry
            .preference_values
            .clone()
            .unwrap_or_else(|| entry.points.preferences.values_from(&BTreeMap::new()));
        Some(PreferencePage {
            index,
            name: entry.id.name(),
            values,
        })
    }
}

/// One installed extension's Preferences page (#2495).
#[derive(Debug, Clone, PartialEq)]
pub struct PreferencePage<'a> {
    /// The extension's install index (`PrefNav::Extension`).
    pub index: usize,
    /// The extension's name.
    pub name: &'a str,
    /// Its preferences and their values.
    pub values: PreferenceValues,
}
