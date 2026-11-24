//! The Component Classes page's view state: filter, Add Class focus, the
//! Reset confirmation, row hover and drag reorder, and the new row's key
//! generated from its label. List edits reuse the `PrefMsg::ComponentClass*`
//! arms, so the draft and the dirty flag change in one place.

use super::super::super::*;

use crate::preferences::{ClassesViewMsg, ClassesViewState, PrefMsg};
use iced::Task;
use iced::widget::operation;

impl Signex {
    pub(crate) fn handle_preferences_classes_view(&mut self, msg: ClassesViewMsg) -> Task<Message> {
        match msg {
            ClassesViewMsg::FilterChanged(query) => {
                self.ui_state.preferences_pages.classes_view.filter = query;
            }
            ClassesViewMsg::LabelEdited { index, label } => {
                return self.relabel_component_class(index, label);
            }
            ClassesViewMsg::AddRequested => return self.add_component_class_row(),
            ClassesViewMsg::ResetRequested => {
                self.ui_state.preferences_pages.classes_view.reset_pending = true;
            }
            ClassesViewMsg::ResetCancelled => {
                self.ui_state.preferences_pages.classes_view.reset_pending = false;
            }
            ClassesViewMsg::ResetConfirmed => {
                // Only an asked Reset replaces the list.
                if !self.ui_state.preferences_pages.classes_view.reset_pending {
                    return Task::none();
                }
                self.ui_state.preferences_pages.classes_view = ClassesViewState::default();
                return self.handle_preferences_message(PrefMsg::ComponentClassResetDefaults);
            }
            ClassesViewMsg::RemoveRequested(index) => {
                if index >= self.ui_state.preferences_draft_component_classes.len() {
                    return Task::none();
                }
                self.ui_state
                    .preferences_pages
                    .classes_view
                    .row_removed(index);
                return self.handle_preferences_message(PrefMsg::ComponentClassRemove { index });
            }
            ClassesViewMsg::RowEntered(index) => {
                self.ui_state.preferences_pages.classes_view.hovered = Some(index);
            }
            ClassesViewMsg::RowLeft(index) => {
                let view = &mut self.ui_state.preferences_pages.classes_view;
                if view.hovered == Some(index) {
                    view.hovered = None;
                }
            }
            ClassesViewMsg::DragStarted(index) => {
                self.ui_state.preferences_pages.classes_view.dragging = Some(index);
            }
            ClassesViewMsg::RowMoved { from, to } => self.move_component_class_row(from, to),
            ClassesViewMsg::DragEnded => {
                self.ui_state.preferences_pages.classes_view.dragging = None;
            }
        }
        Task::none()
    }

    /// Append an empty class through the shared Add arm, mark it as the new
    /// row, clear a filter that would hide it, and focus its key field.
    fn add_component_class_row(&mut self) -> Task<Message> {
        let added = self.handle_preferences_message(PrefMsg::ComponentClassAdd);
        let view = &mut self.ui_state.preferences_pages.classes_view;
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

    /// Set row `index`'s label through the shared arm; the row Add Class
    /// appended also takes the key generated from it while its key was not
    /// typed by hand (mockup row 150). The key column stays editable.
    fn relabel_component_class(&mut self, index: usize, label: String) -> Task<Message> {
        let key = self
            .ui_state
            .preferences_draft_component_classes
            .get(index)
            .and_then(|entry| {
                self.ui_state
                    .preferences_pages
                    .classes_view
                    .generated_key(index, entry, &label)
            });
        let relabelled =
            self.handle_preferences_message(PrefMsg::ComponentClassEditLabel { index, label });
        match key {
            Some(key) => Task::batch([
                relabelled,
                self.handle_preferences_message(PrefMsg::ComponentClassEditKey { index, key }),
            ]),
            None => relabelled,
        }
    }

    /// Drop row `from` on row `to`: it takes that place, the rows between
    /// shift by one. The order is the class picker's order.
    fn move_component_class_row(&mut self, from: usize, to: usize) {
        let classes = &mut self.ui_state.preferences_draft_component_classes;
        if from == to || from >= classes.len() || to >= classes.len() {
            self.ui_state.preferences_pages.classes_view.dragging = None;
            return;
        }
        let moved = classes.remove(from);
        classes.insert(to, moved);
        self.ui_state
            .preferences_pages
            .classes_view
            .row_moved(from, to);
        self.recompute_preferences_dirty();
    }
}

#[cfg(test)]
mod tests {
    use crate::app::{Message, PreferencesMsg, Signex};
    use crate::fonts::ComponentClassEntry;
    use crate::preferences::{ClassesViewMsg, PrefMsg};

    fn send(app: &mut Signex, msg: PrefMsg) {
        let _ = app.update(Message::Preferences(PreferencesMsg::Inner(msg)));
    }

    fn label(app: &mut Signex, index: usize, label: &str) {
        let label = label.to_owned();
        send(
            app,
            PrefMsg::ClassesView(ClassesViewMsg::LabelEdited { index, label }),
        );
    }

    fn row(app: &Signex, index: usize) -> (&str, &str) {
        let entry = &app.ui_state.preferences_draft_component_classes[index];
        (entry.key.as_str(), entry.label.as_str())
    }

    /// One seed class, then the row Add Class appended (index 1).
    fn added() -> Signex {
        let (mut app, _) = Signex::new();
        let _ = app.update(Message::Preferences(PreferencesMsg::Open));
        app.ui_state.preferences_draft_component_classes = vec![ComponentClassEntry {
            key: "resistor".into(),
            label: "Resistor".into(),
        }];
        send(&mut app, PrefMsg::ClassesView(ClassesViewMsg::AddRequested));
        assert_eq!(app.ui_state.preferences_pages.classes_view.new_row, Some(1));
        app
    }

    /// Mockup row 150: Add Class generates the key from the label in lower
    /// snake case, and the key stays editable: once typed by hand it no
    /// longer follows.
    #[test]
    fn the_added_row_key_follows_its_label_until_typed_by_hand() {
        let mut app = added();
        label(&mut app, 1, "Ferrite");
        label(&mut app, 1, "Ferrite bead");
        assert_eq!(row(&app, 1), ("ferrite_bead", "Ferrite bead"));

        let key = "fb".to_owned();
        send(&mut app, PrefMsg::ComponentClassEditKey { index: 1, key });
        label(&mut app, 1, "Ferrite bead (SMD)");
        assert_eq!(row(&app, 1), ("fb", "Ferrite bead (SMD)"));
    }

    /// Only the added row generates its key: relabelling a seed class never
    /// changes the key parts store.
    #[test]
    fn relabelling_an_existing_class_keeps_its_key() {
        let mut app = added();
        label(&mut app, 0, "Resistor (thick film)");
        assert_eq!(row(&app, 0), ("resistor", "Resistor (thick film)"));
        assert_eq!(
            app.ui_state.preferences_draft_component_classes.len(),
            2,
            "a label edit adds no row"
        );
    }
}
