//! Interactive click-click move (#271): the Active Bar's "Move" and
//! "Move Selection" gestures.
//!
//! Both are armed from the Active Bar with no mouse button held. The next
//! left press picks (Move: the object under the cursor; Move Selection: a
//! reference point anywhere), the picked items then follow the cursor, and
//! the press after that drops them. The drop commits through the same
//! [`CanvasEvent::MoveSelected`] a drag-move uses, so undo and dirty
//! tracking stay on one path.

use super::*;

/// Which click-click move the Active Bar armed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MovePick {
    /// Move: the next press on an object selects it alone and lifts it.
    Object,
    /// Move Selection: the next press anywhere is the reference point the
    /// current selection is lifted from.
    Selection,
}

impl CanvasState {
    /// Whether a click-click move is floating.
    pub(in crate::canvas) fn is_floating(&self) -> bool {
        self.move_float.is_some()
    }

    /// Start a floating move anchored at `origin` under `epoch`: the
    /// selection now follows the cursor until the next press drops it, or
    /// until the slot's epoch moves on.
    pub(in crate::canvas) fn start_floating_move(&mut self, origin: (f64, f64), epoch: u64) {
        self.click_on_selected = false;
        self.select_drag_start = None;
        self.move_origin = Some(origin);
        self.move_current = Some(origin);
        self.move_dragging = true;
        self.move_float = Some(epoch);
    }

    /// Drop every move / drag-select gesture in flight (drop, Esc, right-click).
    pub(in crate::canvas) fn clear_move(&mut self) {
        self.click_on_selected = false;
        self.move_origin = None;
        self.move_dragging = false;
        self.move_current = None;
        self.move_float = None;
        self.select_drag_start = None;
    }
}

impl SchematicCanvas<'_> {
    /// True when `state` floats a move that the app has since cancelled
    /// (tab switch, reload, undo, tool change, Esc the canvas never saw).
    /// Floating lives in widget-local state that the same widget position
    /// reuses across tabs, so without this a float started on sheet A could
    /// drop on sheet B.
    pub(in crate::canvas) fn float_is_stale(&self, state: &CanvasState) -> bool {
        state
            .move_float
            .is_some_and(|epoch| epoch != self.move_epoch)
    }

    /// Silently end a stale float: no `MoveSelected`, no preview.
    pub(in crate::canvas) fn drop_stale_float(&self, state: &mut CanvasState) {
        if self.float_is_stale(state) {
            state.clear_move();
        }
    }

    /// Left press while a click-click move is armed or floating. `None`
    /// means no click-click move is involved and the normal press path runs.
    pub(in crate::canvas) fn update_move_pick_press(
        &self,
        state: &mut CanvasState,
        wx: f64,
        wy: f64,
    ) -> Option<canvas::Action<Message>> {
        self.drop_stale_float(state);
        // Drop: the selection has been following the cursor.
        if state.is_floating() {
            let delta = state
                .move_origin
                .zip(state.move_current)
                .map(|((ox, oy), (cx, cy))| (cx - ox, cy - oy));
            state.clear_move();
            return Some(match delta {
                Some((dx, dy)) if dx.abs() > 0.01 || dy.abs() > 0.01 => {
                    canvas::Action::publish(Message::CanvasEvent(CanvasEvent::MoveSelected {
                        dx,
                        dy,
                    }))
                }
                _ => canvas::Action::capture(),
            });
        }
        let pick = self.move_pick?;
        // Defence in depth: an arm that outlived a switch to a placement
        // tool must not swallow that tool's clicks.
        if self.drawing_mode
            || self.tool_preview.is_some()
            || self.ghost_label.is_some()
            || self.ghost_symbol.is_some()
            || self.ghost_text.is_some()
        {
            return None;
        }
        // Move needs an object under the cursor; empty space stays armed.
        if pick == MovePick::Object {
            let hit = self.active_snapshot().and_then(|snapshot| {
                crate::schematic_runtime::hit_test::hit_test(snapshot, wx, wy).filter(|h| {
                    crate::app::passes_filter(h, snapshot, self.prefs.selection_filters)
                })
            });
            if hit.is_none() {
                return Some(canvas::Action::capture());
            }
        }
        state.start_floating_move((wx, wy), self.move_epoch);
        Some(
            canvas::Action::publish(Message::CanvasEvent(CanvasEvent::MovePicked {
                world_x: wx,
                world_y: wy,
            }))
            .and_capture(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::test_prefs;
    use super::*;
    use iced::event::Status;
    use iced::keyboard;
    use iced::widget::canvas::Program as _;

    const BOUNDS: Rectangle = Rectangle {
        x: 0.0,
        y: 0.0,
        width: 800.0,
        height: 600.0,
    };

    /// A slot whose snapshot holds one horizontal wire, (10,10)-(30,10) mm.
    fn slot_with_wire() -> CanvasSlot {
        let sheet = signex_types::schematic::SchematicSheet {
            unknown: Default::default(),
            uuid: uuid::Uuid::new_v4(),
            version: 0,
            generator: String::new(),
            generator_version: String::new(),
            paper_size: "A4".to_string(),
            root_sheet_page: "1".to_string(),
            symbols: Vec::new(),
            wires: vec![signex_types::schematic::Wire {
                uuid: uuid::Uuid::new_v4(),
                start: signex_types::schematic::Point::new(10.0, 10.0),
                end: signex_types::schematic::Point::new(30.0, 10.0),
                stroke_width: 0.0,
                unknown_columns: Default::default(),
            }],
            junctions: Vec::new(),
            labels: Vec::new(),
            child_sheets: Vec::new(),
            no_connects: Vec::new(),
            text_notes: Vec::new(),
            buses: Vec::new(),
            bus_entries: Vec::new(),
            drawings: Vec::new(),
            no_erc_directives: Vec::new(),
            title_block: Default::default(),
            lib_symbols: Default::default(),
        };
        let mut slot = CanvasSlot::new();
        slot.set_render_cache(Some(
            crate::schematic_runtime::SchematicRenderCache::from_sheet(&sheet),
        ));
        slot
    }

    /// Screen point that the slot's camera maps to world `(wx, wy)`.
    fn screen(slot: &CanvasSlot, wx: f32, wy: f32) -> iced::Point {
        let (ox, oy, scale) = slot.live_camera();
        iced::Point::new(wx * scale + ox, wy * scale + oy)
    }

    /// Deliver one event the way the iced runtime does and return the
    /// published message plus whether the event was captured.
    fn send(
        slot: &CanvasSlot,
        state: &mut CanvasState,
        event: Event,
        at: iced::Point,
    ) -> (Option<Message>, bool) {
        let all: std::collections::HashSet<_> = crate::active_bar::SelectionFilter::ALL
            .iter()
            .copied()
            .collect();
        send_filtered(slot, state, event, at, &all)
    }

    /// `send` with an explicit Selection Filter set.
    fn send_filtered(
        slot: &CanvasSlot,
        state: &mut CanvasState,
        event: Event,
        at: iced::Point,
        filters: &std::collections::HashSet<crate::active_bar::SelectionFilter>,
    ) -> (Option<Message>, bool) {
        let overrides = std::collections::HashMap::new();
        let view = SchematicCanvas::new(slot, test_prefs(&overrides, filters));
        let action = view.update(state, &event, BOUNDS, mouse::Cursor::Available(at));
        match action {
            Some(action) => {
                let (message, _, status) = action.into_inner();
                (message, status == Status::Captured)
            }
            None => (None, false),
        }
    }

    fn press(slot: &CanvasSlot, state: &mut CanvasState, at: iced::Point) -> Option<Message> {
        let event = Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left));
        send(slot, state, event, at).0
    }

    fn release(slot: &CanvasSlot, state: &mut CanvasState, at: iced::Point) -> Option<Message> {
        let event = Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left));
        send(slot, state, event, at).0
    }

    /// Move the cursor; the published `CursorAt` is irrelevant here, the
    /// point is the state it leaves behind.
    fn move_to(slot: &CanvasSlot, state: &mut CanvasState, at: iced::Point) {
        let event = Event::Mouse(mouse::Event::CursorMoved { position: at });
        let _ = send(slot, state, event, at);
    }

    fn escape(slot: &CanvasSlot, state: &mut CanvasState) {
        let event = Event::Keyboard(keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(keyboard::key::Named::Escape),
            modified_key: keyboard::Key::Named(keyboard::key::Named::Escape),
            physical_key: keyboard::key::Physical::Code(keyboard::key::Code::Escape),
            location: keyboard::Location::Standard,
            modifiers: keyboard::Modifiers::default(),
            text: None,
            repeat: false,
        });
        let _ = send(slot, state, event, iced::Point::ORIGIN);
    }

    fn is_picked(message: &Option<Message>) -> bool {
        matches!(
            message,
            Some(Message::CanvasEvent(CanvasEvent::MovePicked { .. }))
        )
    }

    fn moved_by(message: &Option<Message>) -> Option<(f64, f64)> {
        match message {
            Some(Message::CanvasEvent(CanvasEvent::MoveSelected { dx, dy })) => Some((*dx, *dy)),
            _ => None,
        }
    }

    #[test]
    fn move_picks_the_object_floats_it_and_the_second_press_drops_it() {
        let mut slot = slot_with_wire();
        slot.move_pick = Some(MovePick::Object);
        let mut state = CanvasState::default();
        let on_wire = screen(&slot, 20.0, 10.0);

        let picked = press(&slot, &mut state, on_wire);
        assert!(
            is_picked(&picked),
            "press on the wire must pick it: {picked:?}"
        );
        assert!(state.is_floating(), "the picked object must float");

        let released = release(&slot, &mut state, on_wire);
        assert!(
            released.is_none(),
            "the release after the picking press must not commit: {released:?}"
        );
        assert!(state.is_floating(), "release must not drop the float");

        // 12.7 mm = 10 grid steps, so the snapped delta is exact.
        let target = screen(&slot, 32.7, 10.0);
        move_to(&slot, &mut state, target);
        let dropped = press(&slot, &mut state, target);
        let (dx, dy) =
            moved_by(&dropped).unwrap_or_else(|| panic!("drop must commit: {dropped:?}"));
        assert!(
            (dx - 12.7).abs() < 0.01 && dy.abs() < 0.01,
            "delta ({dx}, {dy})"
        );
        assert!(!state.is_floating(), "the drop ends the float");
    }

    #[test]
    fn move_press_on_empty_space_does_nothing_and_does_not_float() {
        let mut slot = slot_with_wire();
        slot.move_pick = Some(MovePick::Object);
        let mut state = CanvasState::default();

        let message = press(&slot, &mut state, screen(&slot, 100.0, 100.0));

        assert!(
            message.is_none(),
            "empty space must publish nothing: {message:?}"
        );
        assert!(!state.is_floating(), "empty space must not float anything");
    }

    #[test]
    fn move_selection_press_anywhere_floats() {
        let mut slot = slot_with_wire();
        slot.move_pick = Some(MovePick::Selection);
        let mut state = CanvasState::default();

        let message = press(&slot, &mut state, screen(&slot, 100.0, 100.0));

        assert!(
            is_picked(&message),
            "any press must be the reference: {message:?}"
        );
        assert!(state.is_floating(), "the selection must float");
    }

    #[test]
    fn escape_while_floating_cancels_so_the_next_press_commits_nothing() {
        let mut slot = slot_with_wire();
        slot.move_pick = Some(MovePick::Selection);
        let mut state = CanvasState::default();
        let start = screen(&slot, 20.0, 10.0);
        let _ = press(&slot, &mut state, start);
        let target = screen(&slot, 32.7, 10.0);
        move_to(&slot, &mut state, target);

        escape(&slot, &mut state);
        assert!(!state.is_floating(), "Esc must end the float");
        slot.move_pick = None;
        let message = press(&slot, &mut state, target);

        assert!(
            moved_by(&message).is_none(),
            "a press after Esc must not commit a move: {message:?}"
        );
    }

    #[test]
    fn move_press_on_a_filtered_out_object_does_not_pick_it() {
        let mut slot = slot_with_wire();
        slot.move_pick = Some(MovePick::Object);
        let mut state = CanvasState::default();
        // Wires are not selectable under this filter set.
        let no_wires: std::collections::HashSet<_> = [crate::active_bar::SelectionFilter::Texts]
            .into_iter()
            .collect();
        let event = Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left));

        let (message, _) = send_filtered(
            &slot,
            &mut state,
            event,
            screen(&slot, 20.0, 10.0),
            &no_wires,
        );

        assert!(
            message.is_none(),
            "a filtered-out wire must not pick: {message:?}"
        );
        assert!(!state.is_floating());
    }

    #[test]
    fn right_click_that_cancels_a_floating_move_opens_no_context_menu() {
        let mut slot = slot_with_wire();
        slot.move_pick = Some(MovePick::Selection);
        let mut state = CanvasState::default();
        let at = screen(&slot, 20.0, 100.0);
        let _ = press(&slot, &mut state, at);

        let right_press = Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right));
        let right_release = Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Right));
        let _ = send(&slot, &mut state, right_press, at);
        let (message, _) = send(&slot, &mut state, right_release, at);

        assert!(!state.is_floating(), "right-press cancels the float");
        assert!(
            !matches!(message, Some(Message::ContextMenu(_))),
            "the cancelling click must not also open the menu: {message:?}"
        );
    }

    #[test]
    fn a_float_cancelled_by_the_app_epoch_does_not_drop_on_the_next_press() {
        let mut slot = slot_with_wire();
        slot.move_pick = Some(MovePick::Selection);
        let mut state = CanvasState::default();
        let _ = press(&slot, &mut state, screen(&slot, 20.0, 10.0));
        let target = screen(&slot, 32.7, 10.0);
        move_to(&slot, &mut state, target);
        assert!(state.is_floating(), "precondition: floating");

        // The app switched tabs (or reloaded, or undid): it bumps the epoch.
        slot.move_epoch += 1;
        slot.move_pick = None;
        let message = press(&slot, &mut state, target);

        assert!(
            moved_by(&message).is_none(),
            "a stale float must not commit on another document: {message:?}"
        );
        assert!(!state.is_floating());
    }

    #[test]
    fn an_armed_pick_does_not_swallow_clicks_of_a_placement_tool() {
        let mut slot = slot_with_wire();
        slot.move_pick = Some(MovePick::Object);
        slot.tool_preview = Some("Wire".to_string());
        let mut state = CanvasState::default();

        let message = press(&slot, &mut state, screen(&slot, 20.0, 10.0));

        assert!(
            !is_picked(&message),
            "placement tool owns the click: {message:?}"
        );
        assert!(!state.is_floating());
    }

    #[test]
    fn right_click_that_cancels_a_drag_opens_no_context_menu() {
        let slot = slot_with_wire();
        let mut state = CanvasState::default();
        let at = screen(&slot, 20.0, 100.0);
        state.click_on_selected = true;
        state.move_dragging = true;
        state.move_origin = Some((20.0, 100.0));
        state.move_current = Some((25.0, 100.0));

        let right_press = Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right));
        let right_release = Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Right));
        let _ = send(&slot, &mut state, right_press, at);
        let (message, _) = send(&slot, &mut state, right_release, at);

        assert!(!state.move_dragging, "right-press cancels the drag");
        assert!(
            !matches!(message, Some(Message::ContextMenu(_))),
            "the cancelling click must not also open the menu: {message:?}"
        );
    }
}
