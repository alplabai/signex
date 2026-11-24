//! The PCB Active Bar's Selection Filter (#2373): a click opens a dropdown
//! of the object kinds a click or a box picks. It edits the one filter the
//! board Properties panel's chips edit (#2353), with the same messages, so
//! the two always agree. It is the schematic's Selection Filter dropdown
//! with the board's kinds and presets (#2383); a chip toggles its kind and
//! the dropdown stays open for the next.

use pcb_geom::{PickFilter, PickKind};
use signex_i18n::{I18n, ids, t};
use signex_types::theme::{ThemeId, ThemeTokens};
use signex_widgets::active_bar::{ActiveBarIcon, ActiveBarItem};
use signex_widgets::active_bar_dropdown::DropdownEntry;

use super::dropdown_button;
use crate::active_bar::{
    CustomFilterPreset, FilterChip, FilterMenu, editing_line, filter_menu_entry, preset_chips,
};
use crate::app::{Message, PcbBarMenu};
use crate::dock::DockMessage;
use crate::icons as ic;
use crate::panels::pcb_board_properties::pick_kind_label;
use crate::panels::{PanelMsg, PcbBoardMsg};

/// The Selection Filter button: a click or a right-click opens its dropdown.
pub(super) fn filter_button(
    open: Option<PcbBarMenu>,
    tid: ThemeId,
    i18n: &I18n,
) -> ActiveBarItem<Message> {
    dropdown_button(
        PcbBarMenu::Filter,
        open,
        ActiveBarIcon::Svg(ic::icon_filter(tid)),
        t!(i18n, ids::PCB_BAR_FILTER_TOOLTIP).into_string(),
        tid,
    )
}

fn panel(msg: PcbBoardMsg) -> Message {
    Message::Dock(DockMessage::Panel(PanelMsg::PcbBoard(msg)))
}

/// The board's Selection Filter menu: the schematic's dropdown
/// (`active_bar::filter_menu_entry`) with the board's kinds and presets.
/// A kind the board editor cannot pick is an inert chip with why on hover.
/// The preset chips light the one selected for editing, with "Editing
/// <name>" under them (#2873).
pub(crate) fn entries(
    i18n: &I18n,
    tokens: &ThemeTokens,
    filter: &PickFilter,
    presets: &[CustomFilterPreset<PickKind>],
    editing: Option<usize>,
) -> Vec<DropdownEntry<Message>> {
    let why = t!(i18n, ids::PCB_BOARD_FILTER_UNAVAILABLE);
    let chips = PickKind::ALL
        .into_iter()
        .map(|kind| {
            let live = kind.is_pickable();
            FilterChip {
                label: t!(i18n, pick_kind_label(kind)).into_string(),
                on: filter.is_on(kind),
                msg: live.then(|| panel(PcbBoardMsg::FilterKindToggled(kind))),
                why: (!live).then(|| why.clone()),
            }
        })
        .collect();
    let menu = FilterMenu {
        chips,
        presets: preset_chips(presets, editing, |idx| {
            panel(PcbBoardMsg::FilterPresetApplied(idx))
        }),
        editing: editing_line(i18n, presets, editing),
        on: filter.on_kinds().count(),
        all: panel(PcbBoardMsg::FilterAllSet(true)),
        none: panel(PcbBoardMsg::FilterAllSet(false)),
    };
    vec![filter_menu_entry(tokens, menu)]
}
