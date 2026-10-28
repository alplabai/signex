//! What the status bar says about the gesture in flight: a drag's wire
//! behaviour (#1266) or the Measure tool's prompt (#1256).

use super::super::super::*;
use crate::canvas::FLIP_WIRES_MODIFIER;

impl Signex {
    /// How the drag in flight in `window` treats wires, `None` outside a drag.
    pub(crate) fn move_status(&self, window_id: iced::window::Id) -> Option<String> {
        let keeps = self
            .interaction_state
            .canvas_for_window(window_id)
            .move_behaviour
            .get()?;
        Some(if keeps {
            format!("Dragging with wires ({FLIP_WIRES_MODIFIER}: plain move)")
        } else {
            format!("Moving without wires ({FLIP_WIRES_MODIFIER}: drag with wires)")
        })
    }

    /// The one activity segment the status bar shows.
    pub(crate) fn status_activity(&self, window_id: iced::window::Id) -> Option<String> {
        self.move_status(window_id)
            .or_else(|| self.measure_status(window_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_status_names_the_drag_behaviour_and_the_flip_modifier() {
        let (app, _task) = Signex::new();
        let window = iced::window::Id::unique();
        assert!(
            app.status_activity(window).is_none(),
            "no gesture, no segment"
        );

        let slot = app.interaction_state.canvas_for_window(window);
        slot.move_behaviour.set(Some(true));
        assert_eq!(
            app.status_activity(window).as_deref(),
            Some("Dragging with wires (Alt: plain move)")
        );
        slot.move_behaviour.set(Some(false));
        assert_eq!(
            app.status_activity(window).as_deref(),
            Some("Moving without wires (Alt: drag with wires)")
        );
    }
}
