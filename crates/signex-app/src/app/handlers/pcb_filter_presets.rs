//! The board Selection Filter's saved presets (#2383): the schematic's
//! preset mechanism (`active_bar::presets`) keyed on the board's kinds,
//! kept in `prefs.json` under its own key. Add saves the chips that are on;
//! a click on a preset makes its kinds the filter and selects it for
//! editing, so each chip click then changes it in place (#2873).

use iced::Task;
use pcb_geom::PickKind;

use crate::active_bar::presets::{self, PresetClick};
use crate::app::{Message, Signex};

impl Signex {
    pub(super) fn add_pcb_filter_preset(&mut self) -> Task<Message> {
        let ui = &mut self.ui_state;
        let filter = &ui.pcb_pick_filter;
        let Some(idx) = presets::add(&mut ui.pcb_filter_presets, |kind| filter.is_on(kind)) else {
            return Task::none();
        };
        ui.pcb_filter_preset_editing = Some(idx);
        self.persist_pcb_filter_presets()
    }

    /// A click on a preset: its kinds become the filter and it is selected
    /// for editing; on the selected one, editing ends and the filter stays.
    pub(super) fn click_pcb_filter_preset(&mut self, idx: usize) {
        let ui = &mut self.ui_state;
        let len = ui.pcb_filter_presets.len();
        let clicked = presets::click(&mut ui.pcb_filter_preset_editing, idx, len);
        if clicked != Some(PresetClick::Selected) {
            return;
        }
        let kinds = ui.pcb_filter_presets[idx].filters.iter().copied();
        ui.pcb_pick_filter.set_on_kinds(kinds);
        self.refresh_pcb_highlight();
    }

    /// A kind chip: the kind turns on or off, and the preset selected for
    /// editing takes the change.
    pub(super) fn toggle_pcb_filter_kind(&mut self, kind: PickKind) -> Task<Message> {
        self.ui_state.pcb_pick_filter.toggle(kind);
        self.refresh_pcb_highlight();
        let ui = &mut self.ui_state;
        let Some(idx) = ui.pcb_filter_preset_editing else {
            return Task::none();
        };
        let filter = &ui.pcb_pick_filter;
        if !presets::capture(&mut ui.pcb_filter_presets, idx, |k| filter.is_on(k)) {
            return Task::none();
        }
        self.persist_pcb_filter_presets()
    }

    /// `All - On`, All or None: every kind on (`true`) or off. Editing a
    /// preset ends; the preset keeps its kinds.
    pub(super) fn set_all_pcb_filter_kinds(&mut self, on: bool) {
        self.ui_state.pcb_pick_filter.set_all(on);
        self.ui_state.pcb_filter_preset_editing = None;
        self.refresh_pcb_highlight();
    }

    pub(super) fn rename_pcb_filter_preset(&mut self, idx: usize, name: String) -> Task<Message> {
        if !presets::rename(&mut self.ui_state.pcb_filter_presets, idx, name) {
            return Task::none();
        }
        self.persist_pcb_filter_presets()
    }

    pub(super) fn remove_pcb_filter_preset(&mut self, idx: usize) -> Task<Message> {
        let ui = &mut self.ui_state;
        if !presets::remove(&mut ui.pcb_filter_presets, idx) {
            return Task::none();
        }
        ui.pcb_filter_preset_editing =
            presets::edited_after_removal(ui.pcb_filter_preset_editing, idx);
        self.persist_pcb_filter_presets()
    }

    fn persist_pcb_filter_presets(&self) -> Task<Message> {
        let saved = self.ui_state.pcb_filter_presets.clone();
        let key = crate::fonts::BOARD_FILTER_PRESETS;
        crate::fonts::background_pref_write(key, move || {
            crate::fonts::write_filter_presets(key, &saved)
        })
    }
}

#[cfg(test)]
#[path = "pcb_filter_presets_tests.rs"]
mod tests;
