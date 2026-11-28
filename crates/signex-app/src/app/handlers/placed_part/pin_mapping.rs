//! Change Component's pin mapping by hand (#2954): an old pin remapped to
//! another pin of the new part, or to none, re-judges the swap with the
//! edited mapping, so the comparison, the warnings and what Change applies
//! all follow it. The mapping starts as the one the swap would make by
//! number, then name; the first edit makes it the hand mapping. Tests:
//! `change_session_tests`, "Pin mapping".

use iced::Task;
use signex_engine::change_component_preview;

use super::change_component::replacement_of;
use crate::app::contracts::Message;
use crate::app::state::Signex;
use crate::library::state::PickerMode;

impl Signex {
    /// Map old pin `old` to the new pin `new` (or to none). A pin the
    /// mapping does not list, or a new pin the part does not have, changes
    /// nothing.
    pub(super) fn remap_change_pin(&mut self, old: String, new: Option<String>) -> Task<Message> {
        let Some(PickerMode::ChangePreview(change)) = self.library.picker.as_ref().map(|p| &p.mode)
        else {
            return Task::none();
        };
        if let Some(window) = self.undocked_window_of_change(&change.session.target) {
            return self.run_in_window_document(window, |app| app.remap_change_pin(old, new));
        }
        let listed = change.pins.iter().any(|row| row.old == old);
        let offered = new
            .as_ref()
            .is_none_or(|key| change.pin_choices.iter().any(|c| c.key == *key));
        if !listed || !offered {
            return Task::none();
        }
        let change = (**change).clone();
        let pin_map = change
            .pin_map
            .clone()
            .unwrap_or_else(|| change.preview.mapping.clone())
            .remapped(&old, new.as_deref());
        let mut replacement = replacement_of(&change.resolved);
        replacement.pin_map = Some(pin_map.clone());
        let Some(doc) = self.target_document(&change.session.target) else {
            let name = change.session.reference.clone();
            self.drop_change(&name, "the sheet or part changed");
            return Task::none();
        };
        let preview = change_component_preview(doc, change.session.target.symbol_id, &replacement);
        self.show_preview(
            change.session,
            change.resolved,
            preview,
            change.dropped,
            change.scope,
            Some(pin_map),
            change.step,
        );
        Task::none()
    }
}
