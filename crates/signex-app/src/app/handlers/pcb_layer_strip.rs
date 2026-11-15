//! The layer strip under a board canvas (#2379, #2394): the layer tab menu,
//! the layer set button's menu, the tab names and the saved layer sets.
//! What a set or a menu row shows is the board view's (`pcb_view`), never the
//! board's; the tab names and the saved sets are per user, in `prefs.json`.

use std::path::PathBuf;

use iced::Task;
use iced::widget::operation;
use signex_renderer::pcb::{LayerSetId, PcbLayerView, ShownLayerSet, listed_layers};
use signex_types::pcb::LayerDef;
use signex_types::pcb_layer::PcbLayer;

use crate::app::contracts::LAYER_SET_NAME_INPUT_ID;
use crate::app::{
    LayerSetCard, LayerSetMenu, LayerSetMenuMsg, LayerSetMsg, LayerStripMsg, LayerTabMenu,
    LayerTabMenuMsg, Message, PcbViewMsg, Signex,
};

/// The board whose view a layer set acts on: the board a window shows (a
/// menu, `None`: the main window), or an open board a command names.
pub(crate) enum BoardViewTarget {
    Window(Option<iced::window::Id>),
    Board(PathBuf),
}

impl Signex {
    /// The right-click menu of a layer tab: it opens at the pointer, one
    /// menu at a time, and a picked row changes the view of the board of
    /// the window it was opened in, or the strip's settings.
    pub(crate) fn handle_layer_tab_menu(&mut self, event: LayerTabMenuMsg) -> Task<Message> {
        match event {
            LayerTabMenuMsg::Opened { window, layer } => {
                self.close_all_context_menus();
                // The menu takes the cursor: a card the canvas showed goes.
                self.clear_canvas_hover();
                let (x, y) = self.interaction_state.last_mouse_pos;
                self.interaction_state.pcb_layer_tab_menu = Some(LayerTabMenu {
                    x,
                    y,
                    layer,
                    window,
                    submenu: None,
                });
            }
            LayerTabMenuMsg::Dismissed => self.interaction_state.pcb_layer_tab_menu = None,
            LayerTabMenuMsg::SubmenuOpened(submenu) => {
                if let Some(menu) = self.interaction_state.pcb_layer_tab_menu.as_mut() {
                    menu.submenu = Some(submenu);
                }
            }
            LayerTabMenuMsg::Picked(view_event) => {
                if let Some(menu) = self.interaction_state.pcb_layer_tab_menu.take() {
                    self.apply_pcb_view_event(menu.window, view_event);
                }
            }
            LayerTabMenuMsg::SettingPicked(setting) => {
                self.interaction_state.pcb_layer_tab_menu = None;
                return self.apply_layer_strip_setting(setting);
            }
        }
        Task::none()
    }

    /// The layer set button's menu: pick a set, save the shown layers as a
    /// set, or delete saved sets.
    pub(crate) fn handle_layer_set_menu(&mut self, event: LayerSetMenuMsg) -> Task<Message> {
        match event {
            LayerSetMenuMsg::Opened { window } => {
                self.close_all_context_menus();
                self.clear_canvas_hover();
                let (x, y) = self.interaction_state.last_mouse_pos;
                self.interaction_state.pcb_layer_set_menu = Some(LayerSetMenu {
                    x,
                    y,
                    window,
                    card: LayerSetCard::Sets,
                });
            }
            LayerSetMenuMsg::Dismissed => self.interaction_state.pcb_layer_set_menu = None,
            LayerSetMenuMsg::BuiltinPicked(set) => {
                if let Some(menu) = self.interaction_state.pcb_layer_set_menu.take() {
                    self.apply_pcb_view_event(menu.window, PcbViewMsg::BuiltinLayerSetShown(set));
                }
            }
            LayerSetMenuMsg::SavedPicked(name) => {
                if let Some(menu) = self.interaction_state.pcb_layer_set_menu.take() {
                    let window = menu.window;
                    return self.handle_layer_set(LayerSetMsg::Shown { window, name });
                }
            }
            LayerSetMenuMsg::SaveStarted => {
                if let Some(menu) = self.interaction_state.pcb_layer_set_menu.as_mut() {
                    menu.card = LayerSetCard::Naming(String::new());
                    let input = LAYER_SET_NAME_INPUT_ID.clone();
                    return operation::focus(input);
                }
            }
            LayerSetMenuMsg::NameEdited(text) => {
                if let Some(LayerSetMenu {
                    card: LayerSetCard::Naming(name),
                    ..
                }) = self.interaction_state.pcb_layer_set_menu.as_mut()
                {
                    *name = text;
                }
            }
            LayerSetMenuMsg::NameSubmitted => return self.submit_layer_set_name(),
            LayerSetMenuMsg::ManageStarted => {
                if let Some(menu) = self.interaction_state.pcb_layer_set_menu.as_mut() {
                    menu.card = LayerSetCard::Managing;
                }
            }
            LayerSetMenuMsg::DeletePicked(name) => {
                return self.handle_layer_set(LayerSetMsg::Deleted(name));
            }
        }
        Task::none()
    }

    /// Save current as set…: a typed name saves the set and closes the
    /// menu; a blank one keeps the field open.
    fn submit_layer_set_name(&mut self) -> Task<Message> {
        let Some(LayerSetMenu {
            window,
            card: LayerSetCard::Naming(name),
            ..
        }) = self.interaction_state.pcb_layer_set_menu.as_ref()
        else {
            return Task::none();
        };
        if name.trim().is_empty() {
            return Task::none();
        }
        let (window, name) = (*window, name.clone());
        self.interaction_state.pcb_layer_set_menu = None;
        self.handle_layer_set(LayerSetMsg::Saved { window, name })
    }

    /// Change the strip's tab names, then keep them for the next launch.
    pub(crate) fn apply_layer_strip_setting(&mut self, setting: LayerStripMsg) -> Task<Message> {
        let strip = &mut self.ui_state.layer_strip;
        match setting {
            LayerStripMsg::NameStyleSet(style) => strip.name_style = style,
            LayerStripMsg::LayerNumbersToggled => strip.show_numbers = !strip.show_numbers,
        }
        self.persist_layer_strip()
    }

    /// Save, show or delete a saved layer set. A save or a delete is kept
    /// for the next launch.
    pub(crate) fn handle_layer_set(&mut self, event: LayerSetMsg) -> Task<Message> {
        match event {
            LayerSetMsg::Saved { window, name } => {
                self.save_layer_set(&BoardViewTarget::Window(window), &name)
            }
            LayerSetMsg::Shown { window, name } => {
                self.show_saved_layer_set(&BoardViewTarget::Window(window), &name);
                Task::none()
            }
            LayerSetMsg::Deleted(name) => {
                if !self.ui_state.layer_strip.delete_set(&name) {
                    return Task::none();
                }
                // A view that shows the deleted set lists every layer again.
                let gone = LayerSetId::Saved(name.trim().to_string());
                for view in self.ui_state.pcb_layer_views.values_mut() {
                    if view.layer_set().is_some_and(|set| set.id == gone) {
                        view.forget_layer_set();
                    }
                }
                self.persist_layer_strip()
            }
        }
    }

    /// Edit the view of the board `target` names. Whether there is one.
    fn edit_target_view(
        &mut self,
        target: &BoardViewTarget,
        edit: impl FnOnce(&mut PcbLayerView, &[LayerDef]),
    ) -> bool {
        match target {
            BoardViewTarget::Window(window) => {
                let mut edited = false;
                self.edit_pcb_view(*window, |view, layers| {
                    edit(view, layers);
                    edited = true;
                });
                edited
            }
            BoardViewTarget::Board(path) => self.edit_pcb_view_at(path, edit),
        }
    }

    /// Save current as set...: save the layers the board view of `target`
    /// shows as the set `name`, which it then shows, and keep the set for
    /// the next launch. A blank name saves nothing.
    pub(crate) fn save_layer_set(&mut self, target: &BoardViewTarget, name: &str) -> Task<Message> {
        let name = name.trim().to_string();
        if name.is_empty() {
            return Task::none();
        }
        let mut saved: Option<Vec<PcbLayer>> = None;
        self.edit_target_view(target, |view, defs| {
            let listed = listed_layers(defs);
            let shown: Vec<PcbLayer> = listed
                .iter()
                .copied()
                .filter(|&layer| !view.is_hidden(layer))
                .collect();
            let set = ShownLayerSet {
                id: LayerSetId::Saved(name.clone()),
                layers: shown.clone(),
            };
            view.show_set(&listed, set);
            saved = Some(shown);
        });
        let Some(layers) = saved else {
            return Task::none();
        };
        self.ui_state.layer_strip.save_set(&name, layers);
        self.persist_layer_strip()
    }

    /// Show the saved set `name` on the board view of `target`. Whether
    /// there was such a set and such a board.
    pub(crate) fn show_saved_layer_set(&mut self, target: &BoardViewTarget, name: &str) -> bool {
        let Some(set) = self.ui_state.layer_strip.saved_set(name).cloned() else {
            return false;
        };
        self.edit_target_view(target, |view, defs| {
            let shown = ShownLayerSet {
                id: LayerSetId::Saved(set.name),
                layers: set.layers,
            };
            view.show_set(&listed_layers(defs), shown);
        })
    }

    /// Whether a saved set called `name` exists: what `show_layer_set`
    /// accepts besides the built-in sets.
    pub(crate) fn has_saved_layer_set(&self, name: &str) -> bool {
        self.ui_state.layer_strip.saved_set(name).is_some()
    }

    /// Write the strip's settings in the background.
    fn persist_layer_strip(&self) -> Task<Message> {
        let prefs = self.ui_state.layer_strip.clone();
        crate::fonts::background_pref_write(crate::fonts::LAYER_STRIP_PREF_KEY, move || {
            crate::fonts::write_layer_strip_pref(&prefs)
        })
    }
}
