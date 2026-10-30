//! One press + release of a click-to-place tool over an item must publish
//! exactly one `Clicked` (#1764): the select path's deferred click on
//! release used to be a second placement.

use super::move_pick::tests::{BOUNDS, screen, slot_with_wire};
use super::tests::test_prefs;
use super::*;
use crate::app::Tool;
use iced::widget::canvas::Program as _;

/// Every message one press + release at the wire's midpoint publishes.
fn gesture_messages(tool: Tool, with_ghost_symbol: bool) -> Vec<Message> {
    let mut slot = slot_with_wire();
    if with_ghost_symbol {
        slot.ghost_symbol = Some(signex_types::schematic::Symbol::default());
    }
    let at = screen(&slot, 20.0, 10.0);
    let all: std::collections::HashSet<_> = crate::active_bar::SelectionFilter::ALL
        .iter()
        .copied()
        .collect();
    let overrides = std::collections::HashMap::new();
    let mut state = CanvasState::default();
    let mut out = Vec::new();
    for pressed in [true, false] {
        let mut prefs = test_prefs(&overrides, &all);
        prefs.current_tool = tool;
        let view = SchematicCanvas::new(&slot, prefs);
        let button = mouse::Button::Left;
        let event = Event::Mouse(if pressed {
            mouse::Event::ButtonPressed(button)
        } else {
            mouse::Event::ButtonReleased(button)
        });
        let action = view.update(&mut state, &event, BOUNDS, mouse::Cursor::Available(at));
        if let Some((Some(message), _, _)) = action.map(|a| a.into_inner()) {
            out.push(message);
        }
    }
    out
}

fn clicks(messages: &[Message]) -> usize {
    messages
        .iter()
        .filter(|m| matches!(m, Message::CanvasEvent(CanvasEvent::Clicked { .. })))
        .count()
}

#[test]
fn a_click_to_place_tool_over_an_item_publishes_one_click() {
    for tool in [
        Tool::Label,
        Tool::Text,
        Tool::NoConnect,
        Tool::Junction,
        Tool::NoErcDirective,
        Tool::BusEntry,
        Tool::SheetEntry,
        Tool::Sheet,
        Tool::Measure,
        Tool::BreakWire,
        Tool::Line,
        Tool::Rectangle,
        Tool::Circle,
        Tool::Arc,
        Tool::Polyline,
        Tool::Wire,
        Tool::Bus,
    ] {
        let messages = gesture_messages(tool, false);
        assert_eq!(clicks(&messages), 1, "{tool:?} over a wire: {messages:?}");
    }
}

#[test]
fn a_power_port_on_the_cursor_over_an_item_publishes_one_click() {
    let messages = gesture_messages(Tool::Component, true);
    assert_eq!(clicks(&messages), 1, "{messages:?}");
}

#[test]
fn the_component_tool_without_a_part_still_selects_like_select() {
    let messages = gesture_messages(Tool::Component, false);
    assert_eq!(clicks(&messages), 2, "{messages:?}");
}

#[test]
fn the_select_tool_still_defers_its_click_on_an_item_to_the_release() {
    let messages = gesture_messages(Tool::Select, false);
    assert_eq!(clicks(&messages), 2, "{messages:?}");
}
