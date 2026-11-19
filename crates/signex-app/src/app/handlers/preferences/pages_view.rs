//! View state of the Appearance and Editing pages (#144, #147): help texts
//! opened with More…, a preview filling the page, the Increment step
//! stepper and the pitch labels on the snap preview. A stepper press or a
//! pitch set on the drawing goes through the same draft message as its
//! control, so validation, the dirty flag and Save stay on one path.

use iced::Task;
use signex_widgets::dimension_label::DimensionLabelMessage;

use super::super::super::*;
use crate::preferences::{PagesViewMsg, PagesViewState, PrefMsg};

impl Signex {
    pub(crate) fn handle_preferences_pages_view(&mut self, msg: PagesViewMsg) -> Task<Message> {
        match msg {
            PagesViewMsg::HelpExpandToggled(topic) => {
                self.ui_state
                    .preferences_pages
                    .pages_view
                    .toggle_help(topic);
                Task::none()
            }
            PagesViewMsg::PreviewExpandToggled(kind) => {
                self.ui_state
                    .preferences_pages
                    .pages_view
                    .toggle_preview(kind);
                Task::none()
            }
            PagesViewMsg::IncrementStepNudged(delta) => {
                let step = PagesViewState::nudged_increment_step(
                    self.ui_state.preferences_draft_increment_step,
                    delta,
                );
                self.handle_preferences_message(PrefMsg::DraftIncrementStep(step))
            }
            PagesViewMsg::SnapLabel(message) => self.handle_preferences_snap_label(message),
        }
    }

    /// A pitch label event: a value set on the drawing becomes that fast
    /// grid's field text, and an opened label takes the keyboard.
    fn handle_preferences_snap_label(&mut self, message: DimensionLabelMessage) -> Task<Message> {
        let drafts = self.ui_state.preferences_draft_fast_grids.clone();
        let i18n = std::sync::Arc::clone(&self.ui_state.i18n);
        let step = self
            .ui_state
            .preferences_pages
            .pages_view
            .snap_label_event(message, &drafts, &i18n);
        let committed = match step.committed {
            Some((index, text)) => {
                self.handle_preferences_message(PrefMsg::DraftFastGrid(index, text))
            }
            None => Task::none(),
        };
        let focus = if step.focus_field {
            PagesViewState::focus_snap_field()
        } else {
            Task::none()
        };
        Task::batch([committed, focus])
    }
}
