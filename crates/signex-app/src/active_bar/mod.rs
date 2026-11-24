//! Active Bar — floating toolbar centred at the top of the canvas.
//!
//! 13 group buttons in four groups (Select / Place / Annotate), each with a
//! dropdown menu behind its corner triangle.

use iced::{Element, Widget as _};
use signex_i18n::I18n;
use signex_types::theme::{ThemeId, ThemeTokens};

mod action;
mod bar;
mod dropdown;
mod filter;
#[cfg(test)]
mod glyph_tests;
mod glyphs;
mod layout;
mod planned;
pub mod presets;

pub use action::{ActiveBarAction, requires_net_color, requires_selection};
pub(crate) use action::{DIRECTIVES_DEFAULT_ACTION, action_enabled};
use bar::bar_items;
#[cfg(test)]
pub(crate) use bar::{
    drawn_trigger_centres, drawn_trigger_centres_for, drawn_trigger_centres_with,
};
pub use filter::SelectionFilter;

// Re-exported so `crate::active_bar::view_dropdown` / `dropdown_x_offset`
// paths (overlays/mod.rs) resolve unchanged after the folder split.
pub(crate) use dropdown::{FilterChip, FilterMenu, filter_menu_entry, preset_label};
pub use dropdown::{dropdown_x_offset, view_dropdown};
#[cfg(test)]
pub(crate) use dropdown::filter_card_width;
pub(crate) use layout::{
    BAR_DROPDOWN_GAP_PX, bar_height_for, bar_left_in_window, bar_top_margin, bar_width_for,
    bar_zone_height_for, menu_at, shows_draw_mode_pill, symbol_editor_bar_drop,
};
pub use planned::{PLANNED_TOOLS, is_planned, planned_name, refusal_line};

// ─── Messages ────────────────────────────────────────────────

/// Which Active Bar dropdown menu is open (by button index).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveBarMenu {
    Filter,
    SelectMode, // Lasso, Inside Area, etc.
    Select,     // Move/transform
    Align,
    Wiring,
    Power,
    Harness,
    SheetSymbol, // Sheet Symbol, Sheet Entry, Device Sheet Symbol
    Port,
    Directives,
    TextTools,
    Shapes,
    NetColor, // 10
}

#[derive(Debug, Clone)]
pub enum ActiveBarMsg {
    ToggleMenu(ActiveBarMenu),
    CloseMenus,
    /// A bar button took a right press and opens `menu`; the release of that
    /// press stays off the canvas.
    MenuRightPressed(ActiveBarMenu),
    /// A right press outside the open flyout closed it; its release stays
    /// off the canvas.
    FlyoutRightPressed,
    /// A part of the bar or of its open flyout that opens no menu took a
    /// right press: nothing changes, and its release stays off the canvas.
    RightPressTaken,
    /// A canvas saw the held-back right release, or a new right press after
    /// that release was lost.
    RightReleaseSwallowed,
    Action(ActiveBarAction),
    ToggleFilter(SelectionFilter),
    ToggleAllFilters,
    /// The All cell of the Graphite filter's All / None control was picked:
    /// every category on.
    AllFiltersPicked,
    /// The None cell was picked: every category off.
    NoFiltersPicked,
    /// Replace the active filter set with the user-defined preset at
    /// `index` (0..N). No-op if the index is out of range or the slot is
    /// empty. Source: shortcut buttons in the Filter dropdown.
    ApplyCustomFilter(usize),
    /// "Save current" in the Filter dropdown was picked: the active filter
    /// set becomes a new preset, while there is room for one.
    FilterPresetSavePicked,
    /// A cell of the corner-mode pill was picked: wires and buses bend by
    /// this mode from now on.
    CornerModePicked(signex_types::corner::CornerMode),
    /// `message` came from an overlay painted in the undocked window
    /// `window`: it acts on the sheet that window shows, not on the main
    /// window's (#1910).
    ScopedToWindow(iced::window::Id, Box<ActiveBarMsg>),
}

pub use presets::{CUSTOM_FILTER_PRESET_LIMIT, CustomFilterPreset, PresetKind};

/// A footprint-editor selection-filter preset. Parallel to
/// `CustomFilterPreset` but keyed on `SelectionFilterKind` (footprint
/// categories) instead of the schematic `SelectionFilter`. Persisted to
/// `prefs.json` under `footprint_filter_presets` (Task 6).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FootprintFilterPreset {
    pub name: String,
    pub kinds: Vec<crate::library::editor::footprint::state::selection_filter::SelectionFilterKind>,
}

// ─── View: Active Bar ────────────────────────────────────────

/// Render the Active Bar (the floating toolbar strip).
#[expect(
    clippy::too_many_arguments,
    reason = "8 arguments: the bar reads independent slices of app state and owns none of them"
)]
pub fn view_bar<'a>(
    current_tool: crate::app::Tool,
    corner_mode: signex_types::corner::CornerMode,
    last_tool: &std::collections::HashMap<String, ActiveBarAction>,
    tokens: &'a ThemeTokens,
    tid: ThemeId,
    has_selection: bool,
    has_net_colors: bool,
    i18n: &I18n,
) -> Element<'a, ActiveBarMsg> {
    let state = bar::BarState {
        current_tool,
        corner_mode,
        last_tool,
        tid,
        has_selection,
        has_net_colors,
    };
    let mut items = bar_items(&state, tokens, i18n);
    for item in &mut items {
        if let signex_widgets::active_bar::ActiveBarItem::Button(button) = item
            && let Some(ActiveBarMsg::ToggleMenu(menu)) = button.on_right_press
        {
            button.on_right_press = Some(ActiveBarMsg::MenuRightPressed(menu));
        }
    }
    // Buttons take their own right press first; this takes the rest of the
    // bar (the corner pill, separators, padding).
    iced::widget::mouse_area(signex_widgets::active_bar::view(items, tokens))
        .on_right_press(ActiveBarMsg::RightPressTaken)
        .boxed()
}

#[cfg(test)]
mod layout_tests;
