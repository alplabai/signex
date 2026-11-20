//! Messages and state of the layer strip under a board canvas: the layer
//! tab menu (#2379, #2394), the layer set button and its menu, and the
//! per-user strip settings they change (#2394).

use signex_renderer::pcb::BuiltinLayerSet;
use signex_types::pcb_layer::PcbLayer;

use super::PcbViewMsg;
use crate::layer_strip_prefs::LayerNameStyle;

/// The name field of Save current as set… (#2394), focused when it opens.
pub static LAYER_SET_NAME_INPUT_ID: std::sync::LazyLock<iced::widget::Id> =
    std::sync::LazyLock::new(|| iced::widget::Id::new("signex.layer_set_name"));

/// The right-click menu of a layer tab under the board canvas (#2379).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerTabMenuMsg {
    /// A layer tab was right-clicked in `window` (`None`: the main window):
    /// its menu opens at the pointer.
    Opened {
        window: Option<iced::window::Id>,
        layer: PcbLayer,
    },
    /// The user clicked outside the menu or pressed Esc.
    Dismissed,
    /// A row was picked: the menu closes and `event` changes the view of the
    /// board the menu was opened on.
    Picked(PcbViewMsg),
    /// Hide layers ▸ or Show layers ▸ was pointed at or clicked: its flyout
    /// opens (#2394).
    SubmenuOpened(LayerTabSubmenu),
    /// A tab-name row was picked: the menu closes and `setting` changes the
    /// strip's settings (#2394).
    SettingPicked(LayerStripMsg),
    /// Layer stack... was picked: the menu closes and `board_layer_stack`
    /// (#2401) runs from the window the menu was opened in.
    LayerStackPicked,
    /// Layer colour... on the tab of a layer, or `edit_layer_color` for the
    /// active layer (`None`): the menu closes and the Layers panel opens with
    /// that layer's colours (2026-11-18).
    ColourEditRequested(Option<PcbLayer>),
}

/// A flyout of the layer tab menu (#2394).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerTabSubmenu {
    /// Hide layers ▸: one row per kind of layer.
    Hide,
    /// Show layers ▸: one row per kind of layer.
    Show,
}

/// The open layer tab menu: where it is, the layer it is for, the window
/// whose board it acts on (`None`: the main window) and its open flyout.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayerTabMenu {
    pub x: f32,
    pub y: f32,
    pub layer: PcbLayer,
    pub window: Option<iced::window::Id>,
    pub submenu: Option<LayerTabSubmenu>,
}

/// What the user changed about the strip's per-user settings (#2394).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerStripMsg {
    /// Tab names: Short, Medium or Long.
    NameStyleSet(LayerNameStyle),
    /// Show layer numbers was turned on or off.
    LayerNumbersToggled,
}

/// What the user did with a saved layer set (#2394).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayerSetMsg {
    /// The layers the board view of `window` shows were saved as `name`.
    Saved {
        window: Option<iced::window::Id>,
        name: String,
    },
    /// The saved set `name` was picked for the board view of `window`.
    Shown {
        window: Option<iced::window::Id>,
        name: String,
    },
    /// The saved set `name` was deleted.
    Deleted(String),
}

/// The menu of the layer set button at the left end of the strip (#2394).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayerSetMenuMsg {
    /// The button was clicked in `window` (`None`: the main window).
    Opened { window: Option<iced::window::Id> },
    /// The user clicked outside the menu or pressed Esc.
    Dismissed,
    /// A built-in set was picked.
    BuiltinPicked(BuiltinLayerSet),
    /// A saved set was picked.
    SavedPicked(String),
    /// Save current as set…: the card shows a name field.
    SaveStarted,
    /// The text in the name field changed.
    NameEdited(String),
    /// Enter or Save in the name field.
    NameSubmitted,
    /// Manage sets…: the card lists the saved sets with Delete.
    ManageStarted,
    /// Delete on a saved set's row.
    DeletePicked(String),
}

/// The open layer set menu: where it is, the window whose board it acts
/// on (`None`: the main window) and what its card shows.
#[derive(Debug, Clone, PartialEq)]
pub struct LayerSetMenu {
    pub x: f32,
    pub y: f32,
    pub window: Option<iced::window::Id>,
    pub card: LayerSetCard,
}

/// What the layer set menu's card shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayerSetCard {
    /// The sets to pick from, then Save current as set… and Manage sets….
    Sets,
    /// The name field of Save current as set…, with what was typed.
    Naming(String),
    /// The saved sets, each with Delete.
    Managing,
}
