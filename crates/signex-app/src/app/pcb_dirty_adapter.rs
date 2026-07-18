use super::*;
use signex_renderer::pcb::{PcbAppEvent, dirty_flags_for_events};

const PCB_EVENTS_NONE: &[PcbAppEvent] = &[];
const PCB_EVENTS_THEME: &[PcbAppEvent] = &[PcbAppEvent::ThemeChanged];
const PCB_EVENTS_CAMERA: &[PcbAppEvent] = &[PcbAppEvent::CameraMoved];
const PCB_EVENTS_FOOTPRINT_MOVE: &[PcbAppEvent] = &[PcbAppEvent::FootprintMoved];
const PCB_EVENTS_WIDE_MUTATION: &[PcbAppEvent] = &[
    PcbAppEvent::TraceEdited,
    PcbAppEvent::ViaEdited,
    PcbAppEvent::PadEdited,
    PcbAppEvent::ZoneRefilled,
    PcbAppEvent::RuleAreaUpdated,
    PcbAppEvent::RatsnestRebuilt,
    PcbAppEvent::DrcResultsUpdated,
];

pub(crate) fn pcb_renderer_events_for_message(message: &Message) -> &'static [PcbAppEvent] {
    match message {
        Message::Ui(UiMsg::ThemeChanged(_)) => PCB_EVENTS_THEME,
        Message::Edit(EditMsg::Undo | EditMsg::Redo) => PCB_EVENTS_WIDE_MUTATION,
        Message::CanvasEvent(CanvasEvent::MoveSelected { .. })
        | Message::CanvasEventInWindow {
            event: CanvasEvent::MoveSelected { .. },
            ..
        } => PCB_EVENTS_FOOTPRINT_MOVE,
        Message::CanvasEvent(CanvasEvent::CursorMoved)
        | Message::CanvasEvent(CanvasEvent::CursorAt { .. })
        | Message::CanvasEvent(CanvasEvent::FitAll)
        | Message::CanvasEventInWindow {
            event: CanvasEvent::CursorMoved,
            ..
        }
        | Message::CanvasEventInWindow {
            event: CanvasEvent::CursorAt { .. },
            ..
        }
        | Message::CanvasEventInWindow {
            event: CanvasEvent::FitAll,
            ..
        } => PCB_EVENTS_CAMERA,
        _ => PCB_EVENTS_NONE,
    }
}

impl Signex {
    pub(crate) fn apply_pcb_renderer_dirty_hint(&mut self, message: &Message) {
        if !self.has_active_pcb() {
            return;
        }

        let events = pcb_renderer_events_for_message(message);
        if events.is_empty() {
            return;
        }

        let dirty = dirty_flags_for_events(events);
        if dirty.is_empty() {
            return;
        }

        self.interaction_state.pcb_canvas.clear_content_cache();

        if events
            .iter()
            .any(|event| matches!(event, PcbAppEvent::ThemeChanged))
        {
            self.interaction_state.pcb_canvas.clear_bg_cache();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_renderer::pcb::dirty_flags_for_events;
    use signex_types::theme::ThemeId;

    #[test]
    fn move_selected_canvas_event_maps_to_footprint_move_dirty() {
        let message = Message::CanvasEvent(CanvasEvent::MoveSelected { dx: 1.0, dy: -2.0 });
        let events = pcb_renderer_events_for_message(&message);

        assert_eq!(events, &[PcbAppEvent::FootprintMoved]);
        assert!(!dirty_flags_for_events(events).is_empty());
    }

    #[test]
    fn theme_change_message_maps_to_theme_dirty_event() {
        let message = Message::Ui(UiMsg::ThemeChanged(ThemeId::Signex));
        let events = pcb_renderer_events_for_message(&message);

        assert_eq!(events, &[PcbAppEvent::ThemeChanged]);
        assert!(!dirty_flags_for_events(events).is_empty());
    }

    #[test]
    fn cursor_events_map_to_camera_without_geometry_dirty() {
        let message = Message::CanvasEvent(CanvasEvent::CursorMoved);
        let events = pcb_renderer_events_for_message(&message);

        assert_eq!(events, &[PcbAppEvent::CameraMoved]);
        assert!(dirty_flags_for_events(events).is_empty());
    }

    #[test]
    fn unrelated_message_has_no_pcb_dirty_hint() {
        let message = Message::Ui(UiMsg::GridToggle);
        let events = pcb_renderer_events_for_message(&message);

        assert!(events.is_empty());
    }
}
