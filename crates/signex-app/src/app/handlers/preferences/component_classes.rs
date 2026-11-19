//! The Component Classes page's view state: filter, Add Class focus, the
//! Reset confirmation, row hover and drag reorder. List edits reuse the
//! `PrefMsg::ComponentClass*` arms, so the draft and the dirty flag change in
//! one place.

use super::super::super::*;

use crate::preferences::{ClassesViewMsg, ClassesViewState, PrefMsg};
use iced::Task;
use iced::widget::operation;

impl Signex {
    pub(crate) fn handle_preferences_classes_view(&mut self, msg: ClassesViewMsg) -> Task<Message> {
        match msg {
            ClassesViewMsg::FilterChanged(query) => {
                self.ui_state.preferences_classes_view.filter = query;
            }
            ClassesViewMsg::AddRequested => return self.add_component_class_row(),
            ClassesViewMsg::ResetRequested => {
                self.ui_state.preferences_classes_view.reset_pending = true;
            }
            ClassesViewMsg::ResetCancelled => {
                self.ui_state.preferences_classes_view.reset_pending = false;
            }
            ClassesViewMsg::ResetConfirmed => {
                // Only an asked Reset replaces the list.
                if !self.ui_state.preferences_classes_view.reset_pending {
                    return Task::none();
                }
                self.ui_state.preferences_classes_view = ClassesViewState::default();
                return self.handle_preferences_message(PrefMsg::ComponentClassResetDefaults);
            }
            ClassesViewMsg::RemoveRequested(index) => {
                if index >= self.ui_state.preferences_draft_component_classes.len() {
                    return Task::none();
                }
                self.ui_state.preferences_classes_view.row_removed(index);
                return self.handle_preferences_message(PrefMsg::ComponentClassRemove { index });
            }
            ClassesViewMsg::RowEntered(index) => {
                self.ui_state.preferences_classes_view.hovered = Some(index);
            }
            ClassesViewMsg::RowLeft(index) => {
                let view = &mut self.ui_state.preferences_classes_view;
                if view.hovered == Some(index) {
                    view.hovered = None;
                }
            }
            ClassesViewMsg::DragStarted(index) => {
                self.ui_state.preferences_classes_view.dragging = Some(index);
            }
            ClassesViewMsg::RowMoved { from, to } => self.move_component_class_row(from, to),
            ClassesViewMsg::DragEnded => {
                self.ui_state.preferences_classes_view.dragging = None;
            }
        }
        Task::none()
    }

    /// Append an empty class through the shared Add arm, mark it as the new
    /// row, clear a filter that would hide it, and focus its key field.
    fn add_component_class_row(&mut self) -> Task<Message> {
        let added = self.handle_preferences_message(PrefMsg::ComponentClassAdd);
        let view = &mut self.ui_state.preferences_classes_view;
        view.filter.clear();
        view.reset_pending = false;
        view.new_row = self
            .ui_state
            .preferences_draft_component_classes
            .len()
            .checked_sub(1);
        Task::batch([
            added,
            operation::scrollable::snap_to_end(
                ClassesViewState::table_scroll_id(),
                operation::Animation::Instant,
            ),
            operation::focus(ClassesViewState::new_key_input_id()),
        ])
    }

    /// Drop row `from` on row `to`: it takes that place, the rows between
    /// shift by one. The order is the class picker's order.
    fn move_component_class_row(&mut self, from: usize, to: usize) {
        let classes = &mut self.ui_state.preferences_draft_component_classes;
        if from == to || from >= classes.len() || to >= classes.len() {
            self.ui_state.preferences_classes_view.dragging = None;
            return;
        }
        let moved = classes.remove(from);
        classes.insert(to, moved);
        self.ui_state.preferences_classes_view.row_moved(from, to);
        self.recompute_preferences_dirty();
    }
}
