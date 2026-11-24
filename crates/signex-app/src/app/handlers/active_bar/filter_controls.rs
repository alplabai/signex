use iced::Task;

use super::super::super::*;
use crate::active_bar::SelectionFilter;
use crate::active_bar::presets::{self, PresetClick};
use crate::panels::SchFilterMsg;

impl Signex {
    /// A kind chip: the kind turns on or off, and the preset selected for
    /// editing takes the change (#2873).
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
        self.save_edited_custom_filter_preset()
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
    /// `set_selection_filters` command (#2417), All and None. Editing a
    /// preset ends: the set replaces the filter, not the preset (#2873).
    pub(crate) fn set_selection_filters(
        &mut self,
        filters: std::collections::HashSet<SelectionFilter>,
    ) -> Task<Message> {
        self.interaction_state.selection_filters = filters;
        self.document_state.panel_ctx.selection_filters =
            self.interaction_state.selection_filters.clone();
        self.interaction_state.editing_custom_filter = None;
        self.mirror_edited_custom_filter();
        Task::none()
    }

    /// A message of the SCH Filter or Filter panel (#2417, Graphite
    /// mockup #55, Graphite mockup #56): the Active Bar dropdown's and
    /// Properties' handlers, so the panels, the dropdown and Properties edit
    /// one filter and one preset list.
    pub(crate) fn handle_sch_filter(&mut self, msg: SchFilterMsg) -> Task<Message> {
        match msg {
            SchFilterMsg::KindToggled(filter) => self.handle_active_bar_filter_toggle(filter),
            SchFilterMsg::AllPicked => self.set_all_selection_filters(true),
            SchFilterMsg::NonePicked => self.set_all_selection_filters(false),
            SchFilterMsg::PresetApplied(idx) => self.handle_click_custom_filter_preset(idx),
            SchFilterMsg::PresetSaved => self.handle_save_current_filter_preset(),
            SchFilterMsg::PresetRenameStarted(idx) => {
                let name = self
                    .interaction_state
                    .custom_filter_presets
                    .get(idx)
                    .map(|preset| preset.name.clone());
                self.interaction_state.filter_preset_rename = name.map(|n| (idx, n));
                Task::none()
            }
            SchFilterMsg::PresetRenameTyped(typed) => {
                if let Some((_, name)) = &mut self.interaction_state.filter_preset_rename {
                    *name = typed;
                }
                Task::none()
            }
            SchFilterMsg::PresetRenamed(idx, name) => {
                self.interaction_state.filter_preset_rename = None;
                self.handle_rename_custom_filter_preset(idx, name)
            }
            SchFilterMsg::PresetDeleted(idx) => {
                self.interaction_state.filter_preset_rename = None;
                self.handle_remove_custom_filter_preset(idx)
            }
        }
    }

    /// Save current, from SCH Filter or the Active Bar's Filter dropdown: a
    /// new preset holding the kinds that are on, selected for editing.
    /// Nothing at the preset limit.
    pub(crate) fn handle_save_current_filter_preset(&mut self) -> Task<Message> {
        let state = &mut self.interaction_state;
        let active = &state.selection_filters;
        let Some(idx) = presets::add(&mut state.custom_filter_presets, |f| active.contains(&f))
        else {
            return Task::none();
        };
        state.active_custom_filter_tab = idx;
        state.editing_custom_filter = Some(idx);
        self.sync_and_persist_custom_filter_presets()
    }

    /// A click on a preset chip (SCH Filter, the Active Bar dropdown, the
    /// `apply_selection_filter_preset` command): its kinds become the filter
    /// and it is selected for editing; a click on the selected one ends
    /// editing and leaves the filter as it is (#2873).
    pub(crate) fn handle_click_custom_filter_preset(&mut self, idx: usize) -> Task<Message> {
        let state = &mut self.interaction_state;
        let len = state.custom_filter_presets.len();
        let clicked = presets::click(&mut state.editing_custom_filter, idx, len);
        self.mirror_edited_custom_filter();
        if clicked != Some(PresetClick::Selected) {
            return Task::none();
        }
        let state = &mut self.interaction_state;
        state.selection_filters = state.custom_filter_presets[idx].as_set();
        state.active_custom_filter_tab = idx;
        self.document_state.panel_ctx.selection_filters = state.selection_filters.clone();
        self.document_state.panel_ctx.active_custom_filter_tab = idx;
        Task::none()
    }

    /// A new preset with every kind on, focused for editing.
    pub(crate) fn handle_add_custom_filter_preset(&mut self) -> Task<Message> {
        let state = &mut self.interaction_state;
        let Some(idx) = presets::add(&mut state.custom_filter_presets, |_| true) else {
            return Task::none();
        };
        state.active_custom_filter_tab = idx;
        self.sync_and_persist_custom_filter_presets()
    }

    pub(crate) fn handle_remove_custom_filter_preset(&mut self, idx: usize) -> Task<Message> {
        let state = &mut self.interaction_state;
        if !presets::remove(&mut state.custom_filter_presets, idx) {
            return Task::none();
        }
        let last = state.custom_filter_presets.len().saturating_sub(1);
        state.active_custom_filter_tab = state.active_custom_filter_tab.min(last);
        state.editing_custom_filter =
            presets::edited_after_removal(state.editing_custom_filter, idx);
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

    /// Document Options' member chip. On the preset selected for editing
    /// the filter follows, so the filter and that preset stay one set.
    pub(crate) fn handle_toggle_custom_filter_preset_member(
        &mut self,
        idx: usize,
        filter: SelectionFilter,
    ) -> Task<Message> {
        let state = &mut self.interaction_state;
        if !presets::toggle_member(&mut state.custom_filter_presets, idx, filter) {
            return Task::none();
        }
        if state.editing_custom_filter == Some(idx) {
            state.selection_filters = state.custom_filter_presets[idx].as_set();
            self.document_state.panel_ctx.selection_filters = state.selection_filters.clone();
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

    /// The preset selected for editing takes the kinds that are on, and is
    /// saved; nothing while none is selected (#2873).
    fn save_edited_custom_filter_preset(&mut self) -> Task<Message> {
        match self.interaction_state.editing_custom_filter {
            Some(idx) => self.handle_capture_custom_filter_preset(idx),
            None => Task::none(),
        }
    }

    /// Mirror the edited preset into the panel context now: the Active Bar
    /// dispatch runs no `finish_update`, so its mirror sync would leave the
    /// panels a step behind (#2873).
    fn mirror_edited_custom_filter(&mut self) {
        self.document_state.panel_ctx.selection_filter.editing = self.edited_custom_filter();
    }

    /// Mirror the in-memory preset list + active-tab index into the
    /// panel context (so the Properties panel re-renders) and persist
    /// to disk.
    fn sync_and_persist_custom_filter_presets(&mut self) -> Task<Message> {
        let presets = self.interaction_state.custom_filter_presets.clone();
        self.document_state.panel_ctx.custom_filter_presets = presets.clone();
        self.document_state.panel_ctx.active_custom_filter_tab =
            self.interaction_state.active_custom_filter_tab;
        self.mirror_edited_custom_filter();
        crate::fonts::background_pref_write(crate::fonts::SCHEMATIC_FILTER_PRESETS, move || {
            crate::fonts::write_filter_presets(crate::fonts::SCHEMATIC_FILTER_PRESETS, &presets)
        })
    }
}
