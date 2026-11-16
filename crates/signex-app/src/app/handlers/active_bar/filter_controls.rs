use iced::Task;

use super::super::super::*;
use crate::active_bar::SelectionFilter;
use crate::active_bar::presets;
use crate::panels::SchFilterMsg;

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
        let all_on = self.interaction_state.selection_filters.len() == SelectionFilter::ALL.len();
        self.set_all_selection_filters(!all_on)
    }

    /// Turn every selection-filter category on (`all`) or off.
    pub(crate) fn set_all_selection_filters(&mut self, all: bool) -> Task<Message> {
        if all {
            self.set_selection_filters(SelectionFilter::ALL.iter().copied().collect())
        } else {
            self.set_selection_filters(std::collections::HashSet::new())
        }
    }

    /// Make `filters` the active selection filter set: the
    /// `set_selection_filters` command (#2417).
    pub(crate) fn set_selection_filters(
        &mut self,
        filters: std::collections::HashSet<SelectionFilter>,
    ) -> Task<Message> {
        self.interaction_state.selection_filters = filters;
        self.document_state.panel_ctx.selection_filters =
            self.interaction_state.selection_filters.clone();
        Task::none()
    }

    /// A message of the SCH Filter panel (#2417): the Active Bar dropdown's
    /// handlers, so the panel and the dropdown edit one filter.
    pub(crate) fn handle_sch_filter(&mut self, msg: SchFilterMsg) -> Task<Message> {
        match msg {
            SchFilterMsg::KindToggled(filter) => self.handle_active_bar_filter_toggle(filter),
            SchFilterMsg::AllPicked => self.set_all_selection_filters(true),
            SchFilterMsg::NonePicked => self.set_all_selection_filters(false),
            SchFilterMsg::PresetApplied(idx) => self.handle_apply_custom_filter_preset(idx),
        }
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

    /// A new preset with every kind on, focused for editing.
    pub(crate) fn handle_add_custom_filter_preset(&mut self) -> Task<Message> {
        let state = &mut self.interaction_state;
        let added = presets::add(
            &mut state.custom_filter_presets,
            &mut state.active_custom_filter_tab,
            |_| true,
        );
        if !added {
            return Task::none();
        }
        self.sync_and_persist_custom_filter_presets()
    }

    pub(crate) fn handle_remove_custom_filter_preset(&mut self, idx: usize) -> Task<Message> {
        let state = &mut self.interaction_state;
        let removed = presets::remove(
            &mut state.custom_filter_presets,
            &mut state.active_custom_filter_tab,
            idx,
        );
        if !removed {
            return Task::none();
        }
        self.sync_and_persist_custom_filter_presets()
    }

    pub(crate) fn handle_select_custom_filter_tab(&mut self, idx: usize) -> Task<Message> {
        let len = self.interaction_state.custom_filter_presets.len();
        if idx >= len {
            return Task::none();
        }
        self.interaction_state.active_custom_filter_tab = idx;
        self.document_state.panel_ctx.active_custom_filter_tab = idx;
        Task::none()
    }

    pub(crate) fn handle_rename_custom_filter_preset(
        &mut self,
        idx: usize,
        name: String,
    ) -> Task<Message> {
        if !presets::rename(&mut self.interaction_state.custom_filter_presets, idx, name) {
            return Task::none();
        }
        self.sync_and_persist_custom_filter_presets()
    }

    pub(crate) fn handle_toggle_custom_filter_preset_member(
        &mut self,
        idx: usize,
        filter: SelectionFilter,
    ) -> Task<Message> {
        if !presets::toggle_member(
            &mut self.interaction_state.custom_filter_presets,
            idx,
            filter,
        ) {
            return Task::none();
        }
        self.sync_and_persist_custom_filter_presets()
    }

    pub(crate) fn handle_capture_custom_filter_preset(&mut self, idx: usize) -> Task<Message> {
        let state = &mut self.interaction_state;
        let active = &state.selection_filters;
        if !presets::capture(&mut state.custom_filter_presets, idx, |f| {
            active.contains(&f)
        }) {
            return Task::none();
        }
        self.sync_and_persist_custom_filter_presets()
    }

    /// Mirror the in-memory preset list + active-tab index into the
    /// panel context (so the Properties panel re-renders) and persist
    /// to disk.
    fn sync_and_persist_custom_filter_presets(&mut self) -> Task<Message> {
        let presets = self.interaction_state.custom_filter_presets.clone();
        self.document_state.panel_ctx.custom_filter_presets = presets.clone();
        self.document_state.panel_ctx.active_custom_filter_tab =
            self.interaction_state.active_custom_filter_tab;
        crate::fonts::background_pref_write(crate::fonts::SCHEMATIC_FILTER_PRESETS, move || {
            crate::fonts::write_filter_presets(crate::fonts::SCHEMATIC_FILTER_PRESETS, &presets)
        })
    }
}
