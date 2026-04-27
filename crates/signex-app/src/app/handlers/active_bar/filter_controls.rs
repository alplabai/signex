use iced::Task;

use super::super::super::*;
use crate::active_bar::{CUSTOM_FILTER_PRESET_LIMIT, CustomFilterPreset, SelectionFilter};

impl Signex {
    pub(crate) fn handle_active_bar_filter_toggle(
        &mut self,
        filter: SelectionFilter,
    ) -> Task<Message> {
        if self.interaction_state.selection_filters.contains(&filter) {
            self.interaction_state.selection_filters.remove(&filter);
        } else {
            self.interaction_state.selection_filters.insert(filter);
        }
        self.document_state.panel_ctx.selection_filters =
            self.interaction_state.selection_filters.clone();
        Task::none()
    }

    pub(crate) fn handle_active_bar_all_filters_toggle(&mut self) -> Task<Message> {
        if self.interaction_state.selection_filters.len() == SelectionFilter::ALL.len() {
            self.interaction_state.selection_filters.clear();
        } else {
            self.interaction_state.selection_filters =
                SelectionFilter::ALL.iter().copied().collect();
        }
        self.document_state.panel_ctx.selection_filters =
            self.interaction_state.selection_filters.clone();
        Task::none()
    }

    /// Apply a saved preset — replaces the active filter set.
    pub(crate) fn handle_apply_custom_filter_preset(&mut self, idx: usize) -> Task<Message> {
        let Some(preset) = self.interaction_state.custom_filter_presets.get(idx) else {
            return Task::none();
        };
        self.interaction_state.selection_filters = preset.as_set();
        self.document_state.panel_ctx.selection_filters =
            self.interaction_state.selection_filters.clone();
        Task::none()
    }

    pub(crate) fn handle_add_custom_filter_preset(&mut self) {
        let presets = &mut self.interaction_state.custom_filter_presets;
        if presets.len() >= CUSTOM_FILTER_PRESET_LIMIT {
            return;
        }
        let n = presets.len() + 1;
        presets.push(CustomFilterPreset {
            name: format!("Filter {n}"),
            filters: SelectionFilter::ALL.to_vec(),
        });
        // Auto-focus the freshly added tab so the user can edit it
        // without an extra click.
        self.interaction_state.active_custom_filter_tab = self
            .interaction_state
            .custom_filter_presets
            .len()
            .saturating_sub(1);
        self.sync_and_persist_custom_filter_presets();
    }

    pub(crate) fn handle_remove_custom_filter_preset(&mut self, idx: usize) {
        let presets = &mut self.interaction_state.custom_filter_presets;
        if idx >= presets.len() {
            return;
        }
        presets.remove(idx);
        // Clamp the active tab so we never point past the new end.
        let len = self.interaction_state.custom_filter_presets.len();
        if len == 0 {
            self.interaction_state.active_custom_filter_tab = 0;
        } else if self.interaction_state.active_custom_filter_tab >= len {
            self.interaction_state.active_custom_filter_tab = len - 1;
        }
        self.sync_and_persist_custom_filter_presets();
    }

    pub(crate) fn handle_select_custom_filter_tab(&mut self, idx: usize) {
        let len = self.interaction_state.custom_filter_presets.len();
        if idx >= len {
            return;
        }
        self.interaction_state.active_custom_filter_tab = idx;
        self.document_state.panel_ctx.active_custom_filter_tab = idx;
    }

    pub(crate) fn handle_rename_custom_filter_preset(&mut self, idx: usize, name: String) {
        let Some(preset) = self.interaction_state.custom_filter_presets.get_mut(idx) else {
            return;
        };
        preset.name = name;
        self.sync_and_persist_custom_filter_presets();
    }

    pub(crate) fn handle_toggle_custom_filter_preset_member(
        &mut self,
        idx: usize,
        filter: SelectionFilter,
    ) {
        let Some(preset) = self.interaction_state.custom_filter_presets.get_mut(idx) else {
            return;
        };
        if let Some(pos) = preset.filters.iter().position(|f| *f == filter) {
            preset.filters.remove(pos);
        } else {
            // Re-insert keeping `SelectionFilter::ALL` order so the
            // serialised list stays stable.
            let mut next: Vec<SelectionFilter> = SelectionFilter::ALL
                .iter()
                .copied()
                .filter(|f| preset.filters.contains(f) || *f == filter)
                .collect();
            std::mem::swap(&mut preset.filters, &mut next);
        }
        self.sync_and_persist_custom_filter_presets();
    }

    pub(crate) fn handle_capture_custom_filter_preset(&mut self, idx: usize) {
        let active = self.interaction_state.selection_filters.clone();
        let Some(preset) = self.interaction_state.custom_filter_presets.get_mut(idx) else {
            return;
        };
        let captured = CustomFilterPreset::capture(preset.name.clone(), &active);
        preset.filters = captured.filters;
        self.sync_and_persist_custom_filter_presets();
    }

    /// Mirror the in-memory preset list + active-tab index into the
    /// panel context (so the Properties panel re-renders) and persist
    /// to disk.
    fn sync_and_persist_custom_filter_presets(&mut self) {
        let presets = self.interaction_state.custom_filter_presets.clone();
        self.document_state.panel_ctx.custom_filter_presets = presets.clone();
        self.document_state.panel_ctx.active_custom_filter_tab =
            self.interaction_state.active_custom_filter_tab;
        crate::fonts::write_custom_filter_presets(&presets);
    }
}
