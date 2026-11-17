//! The board Selection Filter's saved presets (#2383): the schematic's
//! preset mechanism (`active_bar::presets`) keyed on the board's kinds,
//! kept in `prefs.json` under its own key. Add saves the chips that are on;
//! a click on a preset makes its kinds the filter.

use iced::Task;

use crate::active_bar::presets;
use crate::app::{Message, Signex};

impl Signex {
    pub(super) fn add_pcb_filter_preset(&mut self) -> Task<Message> {
        let ui = &mut self.ui_state;
        let filter = &ui.pcb_pick_filter;
        let added = presets::add(
            &mut ui.pcb_filter_presets,
            &mut ui.pcb_filter_preset_tab,
            |kind| filter.is_on(kind),
        );
        if !added {
            return Task::none();
        }
        self.persist_pcb_filter_presets()
    }

    pub(super) fn apply_pcb_filter_preset(&mut self, idx: usize) {
        let ui = &mut self.ui_state;
        let Some(preset) = ui.pcb_filter_presets.get(idx) else {
            return;
        };
        ui.pcb_pick_filter
            .set_on_kinds(preset.filters.iter().copied());
        ui.pcb_filter_preset_tab = idx;
        self.refresh_pcb_highlight();
    }

    pub(super) fn rename_pcb_filter_preset(&mut self, idx: usize, name: String) -> Task<Message> {
        if !presets::rename(&mut self.ui_state.pcb_filter_presets, idx, name) {
            return Task::none();
        }
        self.persist_pcb_filter_presets()
    }

    pub(super) fn update_pcb_filter_preset(&mut self, idx: usize) -> Task<Message> {
        let ui = &mut self.ui_state;
        let filter = &ui.pcb_pick_filter;
        if !presets::capture(&mut ui.pcb_filter_presets, idx, |kind| filter.is_on(kind)) {
            return Task::none();
        }
        self.persist_pcb_filter_presets()
    }

    pub(super) fn remove_pcb_filter_preset(&mut self, idx: usize) -> Task<Message> {
        let ui = &mut self.ui_state;
        if !presets::remove(
            &mut ui.pcb_filter_presets,
            &mut ui.pcb_filter_preset_tab,
            idx,
        ) {
            return Task::none();
        }
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
