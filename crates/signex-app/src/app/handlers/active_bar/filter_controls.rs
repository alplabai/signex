use iced::Task;

use super::super::super::*;

impl Signex {
    pub(super) fn handle_active_bar_filter_toggle(
        &mut self,
        filter: crate::active_bar::SelectionFilter,
    ) -> Task<Message> {
        if self.interaction_state.selection_filters.contains(&filter) {
            self.interaction_state.selection_filters.remove(&filter);
        } else {
            self.interaction_state.selection_filters.insert(filter);
        }
        Task::none()
    }

    pub(super) fn handle_active_bar_all_filters_toggle(&mut self) -> Task<Message> {
        if self.interaction_state.selection_filters.len()
            == crate::active_bar::SelectionFilter::ALL.len()
        {
            self.interaction_state.selection_filters.clear();
        } else {
            self.interaction_state.selection_filters =
                crate::active_bar::SelectionFilter::ALL.iter().copied().collect();
        }
        Task::none()
    }
}