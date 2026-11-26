//! The GUI routes of Arrange in Room and Auto Place in Rooms (#2457,
//! #2941): the board in view and the room Properties shows become the
//! `document` and `room` operands, and the same operand handler the CLI
//! runs makes the edit (`command::translator`). Messages says what it did.

use iced::Task;
use serde_json::Value;
use signex_i18n::{ids, t};

use super::RoomToolMsg;
use crate::app::command::{CommandOutcome, OperandValue, translator};
use crate::app::{Message, Signex};

fn count(answer: &Value, key: &str) -> usize {
    answer[key].as_array().map_or(0, Vec::len)
}

impl Signex {
    pub(crate) fn handle_room_tool(&mut self, msg: RoomToolMsg) -> Task<Message> {
        let Some(path) = self.active_tab_path().filter(|_| self.has_active_pcb()) else {
            return Task::none();
        };
        let (command, operands) = match msg {
            RoomToolMsg::ArrangeRequested => {
                let Some(room) = super::shown_room(&self.interaction_state.pcb_selection) else {
                    let line = t!(self.ui_state.i18n, ids::ROOMS_SELECT_ONE);
                    crate::diagnostics::log_warning(line.into_string());
                    return Task::none();
                };
                let room = OperandValue::Text(room.to_string());
                ("arrange_in_room", vec![("room", room)])
            }
            RoomToolMsg::AutoPlaceRequested => ("auto_place", Vec::new()),
        };
        let (outcome, task) = translator::run(self, command, &path, operands);
        match outcome {
            CommandOutcome::Answered(answer) => self.say_room_tool(msg, &answer),
            CommandOutcome::Disabled { reason } => {
                crate::diagnostics::log_warning(reason.render(&self.ui_state.i18n));
            }
            _ => {}
        }
        // The board changed: Properties and the tree show it.
        self.sync_pcb_board_summary();
        task
    }

    fn say_room_tool(&self, msg: RoomToolMsg, answer: &Value) {
        let i18n = &self.ui_state.i18n;
        let line = match msg {
            RoomToolMsg::ArrangeRequested => t!(i18n, ids::ROOMS_ARRANGED,
                "room" => answer["room"].as_str().unwrap_or_default().to_string(),
                "moved" => count(answer, "moved"),
                "outside" => count(answer, "outside")),
            RoomToolMsg::AutoPlaceRequested => t!(i18n, ids::ROOMS_AUTO_PLACED,
                "moved" => count(answer, "moved"),
                "rooms" => count(answer, "rooms"),
                "unplaced" => count(answer, "not_placed")),
        };
        crate::diagnostics::log_info(line.into_string());
    }
}
