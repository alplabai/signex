//! The native (macOS system menu bar) form of the main menu.
//!
//! Everything here is pure and compiled on every OS so the tests run on all
//! CI platforms: the [`MenuBarModel`] becomes a [`NativeMenuSpec`] tree with
//! stable ids, plus the id -> [`MenuMessage`] map the activation path reads.
//! Only `macos.rs` touches `muda`. Build timing, the id scheme, the sync rule
//! and why no row carries a key equivalent: docs/MENU_BAR.md, "Native macOS
//! menu".

use std::collections::HashMap;

use super::MenuMessage;
use super::model::{ItemState, Menu, MenuBarModel, MenuEntry, MenuItem};

#[cfg(target_os = "macos")]
mod macos;
mod state;
#[cfg(test)]
mod tests;

#[cfg(target_os = "macos")]
pub(crate) use macos::activations;
#[cfg_attr(not(target_os = "macos"), allow(unused_imports))] // MenuInputs: macOS sync only
pub(crate) use state::{MenuInputs, NativeMenuState};

/// Title of the app menu. AppKit shows the process name regardless.
const APP_MENU_TITLE: &str = "Signex";

/// A key equivalent. Only the app menu has any (docs/MENU_BAR.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Accelerator {
    /// Command + comma.
    CommandComma,
    /// Command + Q.
    CommandQ,
}

/// A stock AppKit item the app menu uses as is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Predefined {
    Hide,
    HideOthers,
    ShowAll,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NativeItem {
    pub id: String,
    pub label: String,
    pub enabled: bool,
    pub accelerator: Option<Accelerator>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NativeNode {
    Item(NativeItem),
    Predefined(Predefined),
    Submenu(NativeMenu),
    Separator,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NativeMenu {
    pub title: String,
    pub nodes: Vec<NativeNode>,
}

/// The whole native bar: the app menu first, then the model's roots.
/// `PartialEq` is the sync fingerprint; it carries no [`MenuMessage`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct NativeMenuSpec {
    pub menus: Vec<NativeMenu>,
}

/// Id -> what activating it sends. Only live, enabled rows are present, so
/// a hit is always dispatchable and a disabled or vanished id is a miss.
pub(crate) type ActionMap = HashMap<String, MenuMessage>;

#[cfg_attr(not(target_os = "macos"), allow(dead_code))] // only the macOS sync reads it
pub(crate) struct NativeBuild {
    pub spec: NativeMenuSpec,
    pub actions: ActionMap,
}

/// Build the native bar for `model`. Pure: the same model gives the same
/// spec and map.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))] // only the macOS sync calls it outside tests
pub(crate) fn build(model: &MenuBarModel) -> NativeBuild {
    let mut builder = Builder::default();
    let mut menus = vec![builder.app_menu()];
    menus.extend(model.menus.iter().map(|menu| builder.menu(menu, "")));
    NativeBuild {
        spec: NativeMenuSpec { menus },
        actions: builder.actions,
    }
}

#[derive(Default)]
struct Builder {
    actions: ActionMap,
    /// How often each id was handed out, to keep ids unique if two rows of
    /// one menu ever share a label.
    seen: HashMap<String, usize>,
}

impl Builder {
    fn app_menu(&mut self) -> NativeMenu {
        let nodes = vec![
            self.item(
                APP_MENU_TITLE,
                "About Signex",
                Some(MenuMessage::OpenAbout),
                true,
                None,
            ),
            NativeNode::Separator,
            self.item(
                APP_MENU_TITLE,
                "Preferences\u{2026}",
                Some(MenuMessage::OpenPreferences),
                true,
                Some(Accelerator::CommandComma),
            ),
            NativeNode::Separator,
            NativeNode::Predefined(Predefined::Hide),
            NativeNode::Predefined(Predefined::HideOthers),
            NativeNode::Predefined(Predefined::ShowAll),
            NativeNode::Separator,
            // Routed to the unsaved-changes guard, not the stock `terminate:`.
            self.item(
                APP_MENU_TITLE,
                "Quit Signex",
                Some(MenuMessage::Exit),
                true,
                Some(Accelerator::CommandQ),
            ),
        ];
        NativeMenu {
            title: APP_MENU_TITLE.to_owned(),
            nodes,
        }
    }

    fn menu(&mut self, menu: &Menu, parent_path: &str) -> NativeMenu {
        let path = join(parent_path, &menu.title);
        let nodes = menu
            .entries
            .iter()
            .map(|entry| match entry {
                MenuEntry::Separator => NativeNode::Separator,
                MenuEntry::Submenu(sub) => NativeNode::Submenu(self.menu(sub, &path)),
                MenuEntry::Item(item) => self.model_item(item, &path),
            })
            .collect();
        NativeMenu {
            title: menu.title.clone(),
            nodes,
        }
    }

    fn model_item(&mut self, item: &MenuItem, path: &str) -> NativeNode {
        let (action, enabled) = match &item.state {
            ItemState::Placeholder => (None, false),
            ItemState::Live { action, is_enabled } => (Some(action.clone()), *is_enabled),
        };
        // Recent projects share file names across folders; the path is the
        // stable key.
        let key = match &action {
            Some(MenuMessage::OpenRecentProject(path)) => path.display().to_string(),
            _ => item.label.clone(),
        };
        self.item_keyed(path, &key, &item.label, action, enabled, None)
    }

    fn item(
        &mut self,
        path: &str,
        label: &str,
        action: Option<MenuMessage>,
        enabled: bool,
        accelerator: Option<Accelerator>,
    ) -> NativeNode {
        self.item_keyed(path, label, label, action, enabled, accelerator)
    }

    fn item_keyed(
        &mut self,
        path: &str,
        key: &str,
        label: &str,
        action: Option<MenuMessage>,
        enabled: bool,
        accelerator: Option<Accelerator>,
    ) -> NativeNode {
        let id = self.unique_id(join(path, key));
        if let (Some(action), true) = (action, enabled) {
            self.actions.insert(id.clone(), action);
        }
        NativeNode::Item(NativeItem {
            id,
            label: label.to_owned(),
            enabled,
            accelerator,
        })
    }

    fn unique_id(&mut self, id: String) -> String {
        let count = self.seen.entry(id.clone()).or_insert(0);
        *count += 1;
        if *count == 1 {
            id
        } else {
            format!("{id}#{count}")
        }
    }
}

fn join(path: &str, segment: &str) -> String {
    if path.is_empty() {
        segment.to_owned()
    } else {
        format!("{path}/{segment}")
    }
}

/// What activating `id` sends right now: `None` for an id that is unknown,
/// stale, or belongs to a row that is disabled in the current map.
pub(crate) fn resolve_activation<'a>(actions: &'a ActionMap, id: &str) -> Option<&'a MenuMessage> {
    actions.get(id)
}
