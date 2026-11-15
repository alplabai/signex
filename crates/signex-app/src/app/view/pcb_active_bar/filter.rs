//! The PCB Active Bar's Selection Filter (#2373): a click opens a dropdown
//! of the object kinds a click or a box picks. It edits the one filter the
//! board Properties panel's chips edit (#2353), with the same messages, so
//! the two always agree. A row toggles its kind and the dropdown stays open
//! for the next. Saved presets (#2383) are listed under the kinds.

use pcb_geom::{PickFilter, PickKind};
use signex_i18n::{I18n, ids, t};
use signex_types::theme::ThemeId;
use signex_widgets::active_bar::{ActiveBarIcon, ActiveBarItem};
use signex_widgets::active_bar_dropdown::{DropdownEntry, DropdownItem};

use super::dropdown_button;
use crate::active_bar::CustomFilterPreset;
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

/// `All - On`, then a row per kind, checked when on. A kind the board
/// editor cannot pick is greyed with why, as its chip is. Then a row per
/// saved preset (#2383), which applies it.
pub(super) fn entries(
    i18n: &I18n,
    filter: &PickFilter,
    presets: &[CustomFilterPreset<PickKind>],
) -> Vec<DropdownEntry<Message>> {
    let all = DropdownItem::new(
        t!(i18n, ids::PCB_BOARD_FILTER_ALL_ON).into_string(),
        panel(PcbBoardMsg::FilterAllToggled),
    )
    .checked(filter.is_all_on());
    let kinds = PickKind::ALL.into_iter().map(|kind| {
        let label = t!(i18n, pick_kind_label(kind)).into_string();
        let row = DropdownItem::new(label, panel(PcbBoardMsg::FilterKindToggled(kind)));
        let row = if kind.is_pickable() {
            row.checked(filter.is_on(kind))
        } else {
            row.disabled(true)
                .tooltip(t!(i18n, ids::PCB_BOARD_FILTER_UNAVAILABLE).into_string())
        };
        DropdownEntry::Item(row)
    });
    let mut entries: Vec<DropdownEntry<Message>> = std::iter::once(DropdownEntry::Item(all))
        .chain(kinds)
        .collect();
    if !presets.is_empty() {
        entries.push(DropdownEntry::Separator);
        entries.push(DropdownEntry::Header(
            t!(i18n, ids::PCB_BOARD_FILTER_PRESETS).into_string(),
        ));
    }
    for (idx, preset) in presets.iter().enumerate() {
        let label = if preset.name.trim().is_empty() {
            t!(i18n, ids::PANELS_FILTERS_UNNAMED, "number" => idx + 1).into_string()
        } else {
            preset.name.clone()
        };
        let row = DropdownItem::new(label, panel(PcbBoardMsg::FilterPresetApplied(idx)))
            .tooltip(t!(i18n, ids::PCB_BOARD_FILTER_PRESET_APPLY_TIP).into_string());
        entries.push(DropdownEntry::Item(row));
    }
    entries
}
